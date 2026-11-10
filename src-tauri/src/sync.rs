//! Background sync with the Cloudflare Worker (see `worker/src/index.ts`).
//!
//! A round pushes up to `PUSH_BATCH` dirty notes, then pulls pages of server
//! changes until caught up. Conflicts are resolved on the server (newer edit
//! wins, the other version becomes a "Conflict copy"), so the client only
//! has to apply what comes back without clobbering edits made meanwhile.

use std::collections::HashMap;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Instant};

use crate::error::AppResult;
use crate::util::now_ms;
use crate::AppState;

pub const PUSH_BATCH: usize = 100;
const EDIT_DELAY: Duration = Duration::from_secs(3);
const PERIODIC: Duration = Duration::from_secs(5 * 60);
/// Focus or resume syncs only if the last sync is at least this old.
const FOCUS_MIN: Duration = Duration::from_secs(30);
const BACKOFF_START: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_secs(5 * 60);
/// Safety net against a misbehaving server: rounds and pages per sync.
const MAX_STEPS: usize = 500;

// ---- Wire format (snake_case, matches the Worker) ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushedNote {
    pub id: String,
    pub body: String,
    pub pinned: u8,
    pub deleted: u8,
    pub updated_at: i64,
    pub base_rev: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerNote {
    pub id: String,
    pub body: String,
    pub pinned: u8,
    pub deleted: u8,
    pub updated_at: i64,
    pub rev: i64,
}

#[derive(Debug, Serialize)]
pub struct SyncRequest {
    pub since_rev: i64,
    pub changes: Vec<PushedNote>,
}

#[derive(Debug, Deserialize)]
pub struct SyncResponse {
    pub server_rev: i64,
    #[serde(default)]
    pub more: bool,
    #[serde(default)]
    pub reset: bool,
    pub changes: Vec<ServerNote>,
}

// ---- Status shown in the UI ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    Off,
    Idle,
    Syncing,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: SyncState,
    pub last_synced_at: Option<i64>,
    pub message: Option<String>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            state: SyncState::Off,
            last_synced_at: None,
            message: None,
        }
    }
}

#[derive(Debug)]
enum SyncError {
    /// Couldn't reach the server; retried with backoff.
    Offline(String),
    /// The server answered with an error, or local storage failed.
    Failed(String),
}

impl From<crate::error::AppError> for SyncError {
    fn from(e: crate::error::AppError) -> Self {
        SyncError::Failed(e.to_string())
    }
}

/// Where and how to sync, from settings.
#[derive(Clone)]
pub struct SyncConfig {
    pub endpoint: String,
    pub token: String,
}

// ---- Local database side (pure, unit-tested) ----

pub fn last_rev(conn: &Connection) -> AppResult<i64> {
    let rev: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'last_sync_rev'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(rev.and_then(|v| v.parse().ok()).unwrap_or(0))
}

fn set_last_rev(conn: &Connection, rev: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('last_sync_rev', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [rev.to_string()],
    )?;
    Ok(())
}

/// Forget everything about the server, so the next sync re-pushes every note
/// (used when switching servers, or when the server's data was reset).
pub fn reset_state(conn: &Connection) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM notes WHERE deleted = 2", [])?;
    tx.execute("UPDATE notes SET server_rev = 0, dirty = 1", [])?;
    set_last_rev(&tx, 0)?;
    tx.commit()?;
    Ok(())
}

/// Dirty notes to push (oldest edits first) and the pull cursor.
pub fn collect(conn: &Connection) -> AppResult<SyncRequest> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, body, pinned, deleted, updated_at, server_rev FROM notes
         WHERE dirty = 1 ORDER BY updated_at LIMIT ?1",
    )?;
    let changes = stmt
        .query_map([PUSH_BATCH as i64], |r| {
            Ok(PushedNote {
                id: r.get(0)?,
                body: r.get(1)?,
                pinned: r.get(2)?,
                deleted: r.get(3)?,
                updated_at: r.get(4)?,
                base_rev: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SyncRequest {
        since_rev: last_rev(conn)?,
        changes,
    })
}

/// Applies one page of server changes. `pushed` maps each note pushed this
/// round to the `updated_at` that was sent. Returns ids of notes that changed.
pub fn apply(
    conn: &Connection,
    page: &SyncResponse,
    pushed: &HashMap<String, i64>,
) -> AppResult<Vec<String>> {
    let tx = conn.unchecked_transaction()?;
    let mut changed = Vec::new();
    for r in &page.changes {
        let local: Option<(i64, bool, String, u8, u8)> = tx
            .prepare_cached(
                "SELECT updated_at, dirty, body, pinned, deleted FROM notes WHERE id = ?1",
            )?
            .query_row([&r.id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .optional()?;
        let sent = pushed.get(&r.id).copied();
        let take_remote = match &local {
            None => r.deleted != 2,
            // Clean notes simply follow the server.
            Some((_, false, ..)) => true,
            // We pushed exactly this version: the server's answer settles it
            // (ours, or the winner of a conflict whose loser came back as a copy).
            Some((updated_at, true, ..)) => sent == Some(*updated_at),
        };
        // Our own pushes come back as echoes; only real differences count as changes.
        let differs = local.as_ref().is_none_or(|(_, _, body, pinned, deleted)| {
            *body != r.body || *pinned != r.pinned || *deleted != r.deleted
        });
        if take_remote {
            if r.deleted == 2 {
                tx.execute("DELETE FROM notes WHERE id = ?1", [&r.id])?;
            } else {
                tx.prepare_cached(
                    "INSERT INTO notes (id, body, pinned, deleted, updated_at, dirty, server_rev)
                     VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)
                     ON CONFLICT(id) DO UPDATE SET body = excluded.body, pinned = excluded.pinned,
                       deleted = excluded.deleted, updated_at = excluded.updated_at,
                       dirty = 0, server_rev = excluded.server_rev",
                )?
                .execute(params![
                    r.id,
                    r.body,
                    r.pinned,
                    r.deleted,
                    r.updated_at,
                    r.rev
                ])?;
            }
            if differs {
                changed.push(r.id.clone());
            }
        } else if sent == Some(r.updated_at) {
            // The server took an earlier version of ours; local edits made since
            // build on it, so record that the server has seen it.
            tx.execute(
                "UPDATE notes SET server_rev = ?2 WHERE id = ?1",
                params![r.id, r.rev],
            )?;
        }
        // Otherwise: local edits the server hasn't seen. They're pushed next
        // round, and the server keeps both versions if they conflict.
    }
    set_last_rev(&tx, page.server_rev)?;
    tx.execute("DELETE FROM notes WHERE deleted = 2 AND dirty = 0", [])?;
    tx.commit()?;
    Ok(changed)
}

// ---- Network ----

pub fn http_client() -> reqwest::Client {
    let builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("FerriteNotes/", env!("CARGO_PKG_VERSION")));

    // The platform verifier needs JNI initialisation on Android; the bundled
    // Mozilla roots are simpler and cover workers.dev.
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
    );

    builder.build().unwrap_or_default()
}

async fn post(
    http: &reqwest::Client,
    cfg: &SyncConfig,
    req: &SyncRequest,
) -> Result<SyncResponse, SyncError> {
    let res = http
        .post(&cfg.endpoint)
        .bearer_auth(&cfg.token)
        .json(req)
        .send()
        .await
        .map_err(|e| {
            // The friendly message hides the cause (DNS, TLS, timeout…); log it.
            let mut cause = e.to_string();
            let mut source = std::error::Error::source(&e);
            while let Some(s) = source {
                cause.push_str(": ");
                cause.push_str(&s.to_string());
                source = s.source();
            }
            eprintln!("Sync request failed: {cause}");
            if e.is_connect() || e.is_timeout() {
                SyncError::Offline("Can't reach the sync server".into())
            } else {
                SyncError::Failed(format!("Sync request failed: {}", e.without_url()))
            }
        })?;

    let status = res.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SyncError::Failed(
            "The server rejected the sync token".into(),
        ));
    }
    if !status.is_success() {
        #[derive(Deserialize)]
        struct ErrorBody {
            error: String,
        }
        let detail = res
            .json::<ErrorBody>()
            .await
            .map(|b| b.error)
            .unwrap_or_default();
        return Err(SyncError::Failed(format!(
            "Sync server error {status}: {detail}"
        )));
    }
    res.json::<SyncResponse>()
        .await
        .map_err(|_| SyncError::Failed("The sync server sent an unexpected reply".into()))
}

/// Push everything dirty and pull everything new. Returns ids of changed notes.
async fn run(state: &AppState, cfg: &SyncConfig) -> Result<Vec<String>, SyncError> {
    let mut changed = Vec::new();
    let mut steps = 0;
    loop {
        let mut req = collect(&*state.conn()?)?;
        let pushed: HashMap<String, i64> = req
            .changes
            .iter()
            .map(|c| (c.id.clone(), c.updated_at))
            .collect();
        let full_batch = req.changes.len() == PUSH_BATCH;

        let mut reset = false;
        loop {
            steps += 1;
            if steps > MAX_STEPS {
                return Err(SyncError::Failed("Sync did not finish; will retry".into()));
            }
            let page = post(&state.sync.http, cfg, &req).await?;
            if page.reset {
                reset_state(&*state.conn()?)?;
                reset = true;
                break;
            }
            changed.extend(apply(&*state.conn()?, &page, &pushed)?);
            if !page.more {
                break;
            }
            req = SyncRequest {
                since_rev: page.server_rev,
                changes: Vec::new(),
            };
        }
        if !reset && !full_batch {
            return Ok(changed);
        }
    }
}

// ---- Background loop ----

#[derive(Debug)]
pub enum Trigger {
    /// Sync as soon as possible (start, settings changed, "Sync now").
    Now,
    /// The window got focus or the phone app resumed. Only syncs if the last sync is
    /// older than `FOCUS_MIN`: clicking or dragging the title bar refocuses the window,
    /// and that shouldn't hit the server every time.
    Focused,
    /// A note changed; sync once edits pause for a few seconds.
    Edited,
}

pub struct Sync {
    tx: mpsc::UnboundedSender<Trigger>,
    status: std::sync::Mutex<SyncStatus>,
    /// One sync at a time (background loop vs. "Sync now").
    running: tokio::sync::Mutex<()>,
    http: reqwest::Client,
}

impl Sync {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Trigger>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let sync = Self {
            tx,
            status: std::sync::Mutex::new(SyncStatus::default()),
            running: tokio::sync::Mutex::new(()),
            http: http_client(),
        };
        (sync, rx)
    }

    pub fn trigger(&self, t: Trigger) {
        let _ = self.tx.send(t);
    }

    pub fn edited(&self) {
        self.trigger(Trigger::Edited);
    }

    pub fn status(&self) -> SyncStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn publish(app: &AppHandle, state: &AppState, update: impl FnOnce(&mut SyncStatus)) -> SyncStatus {
    let status = match state.sync.status.lock() {
        Ok(mut s) => {
            update(&mut s);
            s.clone()
        }
        Err(_) => SyncStatus::default(),
    };
    let _ = app.emit("sync-status", &status);
    status
}

/// Runs one full sync now and returns the resulting status.
pub async fn sync_once(app: &AppHandle) -> SyncStatus {
    let state = app.state::<AppState>();
    let _running = state.sync.running.lock().await;

    let Some(cfg) = state.sync_config() else {
        return publish(app, &state, |s| *s = SyncStatus::default());
    };
    publish(app, &state, |s| {
        s.state = SyncState::Syncing;
        s.message = None;
    });

    match run(&state, &cfg).await {
        Ok(changed) => {
            if !changed.is_empty() {
                let _ = app.emit("notes-changed", &changed);
            }
            publish(app, &state, |s| {
                s.state = SyncState::Idle;
                s.last_synced_at = Some(now_ms());
                s.message = None;
            })
        }
        Err(SyncError::Offline(msg)) => publish(app, &state, |s| {
            s.state = SyncState::Offline;
            s.message = Some(msg);
        }),
        Err(SyncError::Failed(msg)) => publish(app, &state, |s| {
            s.state = SyncState::Error;
            s.message = Some(msg);
        }),
    }
}

/// Syncs on start, a few seconds after edits, on demand, and every 5 minutes;
/// failures retry with exponential backoff (5 s → 10 s → … → 5 min).
pub async fn run_loop(app: AppHandle, mut rx: mpsc::UnboundedReceiver<Trigger>) {
    let mut due: Option<Instant> = Some(Instant::now());
    let mut last_sync: Option<Instant> = None;
    let mut retry_at: Option<Instant> = None;
    let mut backoff = BACKOFF_START;

    loop {
        let wake = due.unwrap_or_else(|| Instant::now() + PERIODIC);
        tokio::select! {
            trigger = rx.recv() => match trigger {
                None => return,
                Some(Trigger::Now) => due = Some(Instant::now()),
                Some(Trigger::Focused) => {
                    if last_sync.is_none_or(|t| t.elapsed() >= FOCUS_MIN) && retry_at.is_none() {
                        due = Some(Instant::now());
                    }
                }
                Some(Trigger::Edited) => {
                    let at = Instant::now() + EDIT_DELAY;
                    // Debounce: each edit pushes the sync back, but never before a pending retry.
                    due = Some(retry_at.map_or(at, |r| r.max(at)));
                }
            },
            _ = sleep_until(wake) => {
                let status = sync_once(&app).await;
                last_sync = Some(Instant::now());
                if matches!(status.state, SyncState::Offline | SyncState::Error) {
                    let at = Instant::now() + backoff;
                    retry_at = Some(at);
                    due = Some(at);
                    backoff = (backoff * 2).min(BACKOFF_MAX);
                } else {
                    retry_at = None;
                    due = None;
                    backoff = BACKOFF_START;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Simulates devices against an in-memory copy of the Worker's algorithm.

    use super::*;
    use crate::db::open_in_memory;
    use crate::notes;

    #[derive(Default)]
    struct FakeServer {
        notes: Vec<ServerNote>,
        rev: i64,
        page_size: usize,
    }

    impl FakeServer {
        fn new() -> Self {
            Self {
                page_size: 200,
                ..Default::default()
            }
        }

        fn write(&mut self, mut n: ServerNote) {
            self.rev += 1;
            n.rev = self.rev;
            self.notes.retain(|x| x.id != n.id);
            self.notes.push(n);
        }

        fn keep_loser(&mut self, loser: &ServerNote) {
            if loser.deleted == 0 && !loser.body.trim().is_empty() {
                let title = loser
                    .body
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim();
                self.write(ServerNote {
                    id: uuid::Uuid::now_v7().to_string(),
                    body: format!("Conflict copy – {title}\n\n{}", loser.body),
                    pinned: 0,
                    deleted: 0,
                    updated_at: now_ms(),
                    rev: 0,
                });
            }
        }

        fn handle(&mut self, req: &SyncRequest) -> SyncResponse {
            if req.since_rev > self.rev {
                return SyncResponse {
                    server_rev: 0,
                    more: false,
                    reset: true,
                    changes: vec![],
                };
            }
            for c in &req.changes {
                let incoming = ServerNote {
                    id: c.id.clone(),
                    body: c.body.clone(),
                    pinned: c.pinned,
                    deleted: c.deleted,
                    updated_at: c.updated_at,
                    rev: 0,
                };
                match self.notes.iter().find(|n| n.id == c.id).cloned() {
                    None => {
                        if c.deleted != 2 {
                            self.write(incoming)
                        }
                    }
                    Some(cur) if c.updated_at > cur.updated_at => {
                        if cur.rev > c.base_rev && cur.body != c.body {
                            self.keep_loser(&cur);
                        }
                        self.write(incoming);
                    }
                    Some(cur) => {
                        if cur.body != c.body {
                            self.keep_loser(&incoming);
                        }
                        self.write(cur);
                    }
                }
            }
            let mut newer: Vec<_> = self
                .notes
                .iter()
                .filter(|n| n.rev > req.since_rev)
                .cloned()
                .collect();
            newer.sort_by_key(|n| n.rev);
            let more = newer.len() > self.page_size;
            newer.truncate(self.page_size);
            SyncResponse {
                server_rev: newer.last().map_or(req.since_rev, |n| n.rev),
                more,
                reset: false,
                changes: newer,
            }
        }
    }

    /// Same steps as `run`, without the network.
    fn sync(conn: &Connection, server: &mut FakeServer) {
        loop {
            let mut req = collect(conn).unwrap();
            let pushed: HashMap<_, _> = req
                .changes
                .iter()
                .map(|c| (c.id.clone(), c.updated_at))
                .collect();
            let full = req.changes.len() == PUSH_BATCH;
            let mut reset = false;
            loop {
                let page = server.handle(&req);
                if page.reset {
                    reset_state(conn).unwrap();
                    reset = true;
                    break;
                }
                apply(conn, &page, &pushed).unwrap();
                if !page.more {
                    break;
                }
                req = SyncRequest {
                    since_rev: page.server_rev,
                    changes: vec![],
                };
            }
            if !reset && !full {
                return;
            }
        }
    }

    fn bodies(conn: &Connection) -> Vec<String> {
        let mut v: Vec<String> = conn
            .prepare("SELECT body FROM notes WHERE deleted = 0 ORDER BY body")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        v.sort();
        v
    }

    fn dirty_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM notes WHERE dirty = 1", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn new_note(conn: &Connection, body: &str) -> String {
        let n = notes::create(conn).unwrap();
        notes::save(conn, &n.id, body).unwrap();
        n.id
    }

    #[test]
    fn notes_reach_the_other_device() {
        let mut server = FakeServer::new();
        let (a, b) = (open_in_memory(), open_in_memory());
        new_note(&a, "Groceries\n- [ ] milk");
        sync(&a, &mut server);
        sync(&b, &mut server);
        assert_eq!(bodies(&b), vec!["Groceries\n- [ ] milk"]);
        assert_eq!(dirty_count(&a), 0);
        assert_eq!(dirty_count(&b), 0);
    }

    #[test]
    fn offline_edits_on_both_devices_keep_both_versions() {
        let mut server = FakeServer::new();
        let (a, b) = (open_in_memory(), open_in_memory());
        let id = new_note(&a, "Plan\nv1");
        sync(&a, &mut server);
        sync(&b, &mut server);

        notes::save(&a, &id, "Plan\nfrom A").unwrap();
        notes::save(&b, &id, "Plan\nfrom B").unwrap();
        sync(&a, &mut server);
        sync(&b, &mut server);
        sync(&a, &mut server);

        // Both devices converge, and neither edit is lost.
        assert_eq!(bodies(&a), bodies(&b));
        let all = bodies(&a).join("\n---\n");
        assert!(all.contains("from A"), "{all}");
        assert!(all.contains("from B"), "{all}");
        assert!(all.contains("Conflict copy – Plan"), "{all}");
        assert_eq!(bodies(&a).len(), 2);
    }

    #[test]
    fn clock_skew_never_loses_an_edit() {
        let mut server = FakeServer::new();
        let (a, b) = (open_in_memory(), open_in_memory());
        let id = new_note(&a, "Note\nv1");
        sync(&a, &mut server);
        sync(&b, &mut server);

        // A's clock is an hour fast; B edits later in real time but "earlier" by its clock.
        notes::save(&a, &id, "Note\nfrom A").unwrap();
        a.execute(
            "UPDATE notes SET updated_at = updated_at + 3600000 WHERE id = ?1",
            [&id],
        )
        .unwrap();
        notes::save(&b, &id, "Note\nfrom B").unwrap();
        sync(&a, &mut server);
        sync(&b, &mut server);

        let all = bodies(&b).join("\n");
        assert!(all.contains("from A") && all.contains("from B"), "{all}");
    }

    #[test]
    fn retry_after_lost_response_is_harmless() {
        let mut server = FakeServer::new();
        let a = open_in_memory();
        new_note(&a, "hello");
        // The server applies the push, but the reply never arrives.
        let _lost = server.handle(&collect(&a).unwrap());
        sync(&a, &mut server);
        assert_eq!(bodies(&a), vec!["hello"]);
        assert_eq!(server.notes.len(), 1);
        assert_eq!(dirty_count(&a), 0);
    }

    #[test]
    fn edits_made_during_a_sync_stay_dirty() {
        let mut server = FakeServer::new();
        let a = open_in_memory();
        let id = new_note(&a, "v1");
        let req = collect(&a).unwrap();
        let pushed: HashMap<_, _> = req
            .changes
            .iter()
            .map(|c| (c.id.clone(), c.updated_at))
            .collect();
        let page = server.handle(&req);
        notes::save(&a, &id, "v2").unwrap(); // typed while the request was in flight
        apply(&a, &page, &pushed).unwrap();

        assert_eq!(notes::get(&a, &id).unwrap().body, "v2");
        assert_eq!(dirty_count(&a), 1);
        sync(&a, &mut server);
        assert_eq!(server.notes[0].body, "v2");
        assert!(
            server.notes.len() == 1,
            "no conflict copy for our own edits"
        );
    }

    #[test]
    fn trash_and_purge_propagate() {
        let mut server = FakeServer::new();
        let (a, b) = (open_in_memory(), open_in_memory());
        let id = new_note(&a, "secret");
        sync(&a, &mut server);
        sync(&b, &mut server);

        notes::set_deleted(&a, &id, true).unwrap();
        sync(&a, &mut server);
        sync(&b, &mut server);
        assert!(notes::get(&b, &id).unwrap().deleted);

        notes::purge_trash(&a, true).unwrap();
        sync(&a, &mut server);
        sync(&b, &mut server);
        let rows = |c: &Connection| -> i64 {
            c.query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(rows(&a), 0);
        assert_eq!(rows(&b), 0);
        assert_eq!(server.notes[0].body, "");
    }

    #[test]
    fn large_libraries_sync_in_batches_and_pages() {
        let mut server = FakeServer::new();
        server.page_size = 30;
        let (a, b) = (open_in_memory(), open_in_memory());
        for i in 0..250 {
            new_note(&a, &format!("note {i:03}"));
        }
        sync(&a, &mut server);
        sync(&b, &mut server);
        assert_eq!(bodies(&b).len(), 250);
        assert_eq!(dirty_count(&a), 0);
    }

    #[test]
    fn new_server_gets_everything_again() {
        let mut server = FakeServer::new();
        let a = open_in_memory();
        new_note(&a, "one");
        sync(&a, &mut server);
        new_note(&a, "two");
        sync(&a, &mut server);

        let mut fresh = FakeServer::new(); // e.g. the database was recreated
        sync(&a, &mut fresh);
        let mut on_server: Vec<_> = fresh.notes.iter().map(|n| n.body.clone()).collect();
        on_server.sort();
        assert_eq!(on_server, vec!["one", "two"]);
    }
}
