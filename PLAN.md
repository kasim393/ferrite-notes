# Notes App — Tauri Build Plan

A lightweight, fast, cross-platform notes app for quick notes, task lists, and bug lists,
with clickable checkboxes, markdown rendering for pasted content, and free cloud sync.
Targets Windows, macOS, Linux (installer + portable), Android, and iOS.

---

## 1. Goals

- Instant startup (< 300 ms to usable window on desktop)
- Low memory (target < 80 MB idle on desktop)
- Small app size (target < 10 MB installer)
- Never lose a note: crash-safe local storage, safe sync
- Works fully offline; sync is an add-on, not a requirement
- Portable mode: run from a USB stick or folder, data stored beside the app
- $0 running cost

## 2. Non-goals (keep it light)

- No rich-text editor, no WYSIWYG
- No collaboration or note sharing
- No attachments/images (v1)
- No plugins/themes system

---

## 3. Tech Stack

| Layer       | Choice                                         | Why                                              |
| ----------- | ---------------------------------------------- | ------------------------------------------------ |
| Shell       | Tauri 2                                        | Small binary, OS webview, desktop + mobile       |
| Backend     | Rust                                           | Storage, search, markdown, sync all run natively |
| Frontend    | Svelte 5 + Vite + TypeScript                   | Compiles to tiny JS, no runtime framework        |
| Editor      | CodeMirror 6 (minimal setup, not `basicSetup`) | Light, fast, supports inline checkbox widgets    |
| Database    | SQLite via `rusqlite` (`bundled` feature)      | Reliable, fast, single file                      |
| Search      | SQLite FTS5                                    | Instant full-text search                         |
| Markdown    | `pulldown-cmark` + `ammonia` (Rust)            | Fast rendering + HTML sanitizing, keeps JS small |
| Sync server | Cloudflare Workers + D1                        | Free tier, no cold-sleep, simple API             |
| HTTP client | `reqwest` (rustls)                             | No OpenSSL dependency, smaller/portable          |
| Styling     | Plain CSS with CSS variables                   | No UI library; light/dark themes via variables   |

### Tauri plugins (official only, minimal)

- `tauri-plugin-single-instance` — prevent two copies corrupting the DB (desktop)
- `tauri-plugin-window-state` — remember window size/position (desktop)
- `tauri-plugin-clipboard-manager` — reliable paste handling (optional)

Avoid adding plugins that aren't needed; each one adds size and permissions.

---

## 4. Architecture

```
┌────────────────────── Tauri App ──────────────────────┐
│  Svelte UI (webview)                                  │
│   - Sidebar: search, note list, pinned, trash         │
│   - Editor: CodeMirror 6 + checkbox widgets           │
│   - Preview: rendered HTML from Rust                  │
│            │  invoke() commands / events              │
│  Rust core ▼                                          │
│   - db.rs      SQLite, migrations, FTS5               │
│   - notes.rs   CRUD, list summaries                   │
│   - markdown.rs render + sanitize                     │
│   - sync.rs    background sync loop                   │
│   - paths.rs   portable vs installed data location    │
└──────────────────────────┬────────────────────────────┘
                           │ HTTPS + bearer token
                ┌──────────▼──────────┐
                │ Cloudflare Worker   │
                │  POST /sync         │
                │  D1 (SQLite)        │
                └─────────────────────┘
```

Principle: **the UI is a thin view**. All data logic lives in Rust so it's fast, testable,
and shared across desktop and mobile.

---

## 5. Project Structure

```
notes/
├─ src/                      # Svelte frontend
│  ├─ App.svelte
│  ├─ lib/
│  │  ├─ api.ts              # typed wrappers around invoke()
│  │  ├─ store.svelte.ts     # app state (Svelte 5 runes)
│  │  ├─ editor/
│  │  │  ├─ setup.ts         # minimal CodeMirror extensions
│  │  │  └─ checkbox.ts      # "- [ ]" widget decorations
│  │  └─ components/
│  │     ├─ Sidebar.svelte
│  │     ├─ NoteList.svelte
│  │     ├─ Editor.svelte
│  │     └─ Preview.svelte
│  └─ styles.css
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs / lib.rs
│  │  ├─ db.rs
│  │  ├─ notes.rs
│  │  ├─ markdown.rs
│  │  ├─ sync.rs
│  │  ├─ paths.rs
│  │  └─ error.rs
│  ├─ migrations/
│  │  └─ 001_init.sql
│  ├─ capabilities/default.json
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ worker/                   # Cloudflare sync server
│  ├─ src/index.ts
│  ├─ schema.sql
│  └─ wrangler.toml
└─ PLAN.md
```

---

## 6. Data Model

### Local (SQLite)

```sql
PRAGMA journal_mode = WAL;        -- crash-safe, fast concurrent reads
PRAGMA synchronous = NORMAL;      -- safe with WAL, much faster than FULL
PRAGMA foreign_keys = ON;

CREATE TABLE notes (
  id          TEXT PRIMARY KEY,           -- UUID v7 (time-sortable)
  body        TEXT NOT NULL DEFAULT '',
  pinned      INTEGER NOT NULL DEFAULT 0,
  deleted     INTEGER NOT NULL DEFAULT 0, -- soft delete (needed for sync)
  updated_at  INTEGER NOT NULL,           -- ms epoch, set on every edit
  dirty       INTEGER NOT NULL DEFAULT 1  -- 1 = not yet pushed to server
);
CREATE INDEX idx_notes_list ON notes(deleted, pinned DESC, updated_at DESC);

CREATE VIRTUAL TABLE notes_fts USING fts5(
  body, content='notes', content_rowid='rowid', tokenize='unicode61'
);
-- + triggers to keep notes_fts in sync on insert/update/delete

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
-- stores: last_sync_rev, device_id, schema_version
```

### Note format

Notes are plain text. First line = title. Tasks are markdown task lines:

```
Bugs - billing page
- [x] Fix tax rounding
- [ ] Date picker breaks on Safari
  - [ ] Nested sub-task works too
```

### Migrations

- Use `PRAGMA user_version` to track schema version.
- Run migrations inside a transaction at startup; refuse to start (with a clear error)
  if the DB is from a newer app version.

---

## 7. Rust Commands (UI ↔ Core API)

| Command                                                 | Returns                   | Notes                                                                  |
| ------------------------------------------------------- | ------------------------- | ---------------------------------------------------------------------- |
| `list_notes(filter)`                                    | `Vec<NoteSummary>`        | id, title, 80-char snippet, pinned, updated_at — **never full bodies** |
| `get_note(id)`                                          | `Note`                    | full body                                                              |
| `save_note(id, body)`                                   | `updated_at`              | called debounced from UI                                               |
| `create_note()`                                         | `Note`                    |                                                                        |
| `toggle_task(id, line_no)`                              | `Note`                    | flips `[ ]`/`[x]` on one line, atomic                                  |
| `set_pinned(id, bool)`                                  | —                         |                                                                        |
| `trash_note(id)` / `restore_note(id)` / `purge_trash()` | —                         |                                                                        |
| `search(query)`                                         | `Vec<NoteSummary>`        | FTS5 with `snippet()` highlighting                                     |
| `render_markdown(id)`                                   | `String` (sanitized HTML) |                                                                        |
| `sync_now()`                                            | `SyncStatus`              |                                                                        |
| `get_settings()` / `set_settings()`                     |                           | sync URL, token, theme                                                 |

Rules:

- All commands return `Result<T, AppError>`; no `unwrap()`/`expect()` in command paths.
- DB access through one connection guarded by a `Mutex` (SQLite is fastest single-writer);
  heavy work (sync, purge) runs on `tauri::async_runtime::spawn_blocking`.
- Use prepared statement caching (`conn.prepare_cached`).

---

## 8. Frontend

### Layout

- **Desktop:** sidebar (search + list) | editor/preview pane
- **Mobile:** list screen → tap → note screen (same components, CSS media query)

### Editor + checkboxes

- CodeMirror 6 with only: history, keymap, line wrapping, markdown language (lazy-loaded),
  and a custom checkbox extension.
- Checkbox extension: a `ViewPlugin` that finds `- [ ]` / `- [x]` in **visible ranges only**
  and replaces the brackets with a clickable `<input type=checkbox>` widget.
  Clicking dispatches a text change (`[ ]` ↔ `[x]`), so undo works naturally.
- Checked items get a strikethrough + muted color via line decoration.
- Keyboard: `Ctrl/Cmd+Enter` toggles the task on the current line;
  `Enter` on a task line auto-inserts `- [ ] ` on the next line; `Enter` on an empty
  task line removes the prefix (like most todo apps).

### Preview (pasted markdown)

- Toggle button (`Ctrl/Cmd+P`) switches to preview.
- HTML rendered in Rust, already sanitized; frontend just sets it into a container.
- Checkboxes in preview are clickable and call `toggle_task(id, line_no)`.

### Saving

- Debounce saves 400 ms after last keystroke, plus save immediately on note switch,
  window blur, and app close (`onCloseRequested`).
- Show a subtle "saved" / "syncing" / "offline" indicator.

### Shortcuts

- `Ctrl/Cmd+N` new note · `Ctrl/Cmd+F` focus search · `Ctrl/Cmd+Shift+P` pin
- `Ctrl/Cmd+Backspace` trash · `Esc` clear search

---

## 9. Sync Design

### Server (Cloudflare Worker + D1)

```sql
CREATE TABLE notes (
  id          TEXT PRIMARY KEY,
  body        TEXT NOT NULL,
  pinned      INTEGER NOT NULL,
  deleted     INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,   -- client edit time (conflict resolution)
  rev         INTEGER NOT NULL    -- server sequence number (sync cursor)
);
CREATE INDEX idx_rev ON notes(rev);
CREATE TABLE counter (id INTEGER PRIMARY KEY CHECK (id = 1), rev INTEGER NOT NULL);
```

Single endpoint: `POST /sync`

```json
// request
{ "since_rev": 120, "changes": [ { "id": "...", "body": "...", "pinned": 0,
  "deleted": 0, "updated_at": 1760000000000 } ] }
// response
{ "server_rev": 135, "changes": [ ...notes with rev > since_rev... ] }
```

Server logic (one D1 batch / transaction):

1. For each incoming change: upsert **only if** `incoming.updated_at > existing.updated_at`
   (or note doesn't exist). Assign a new `rev` from the counter.
2. Return all notes with `rev > since_rev`.

Why `rev` instead of timestamps for the cursor: device clocks can be wrong; a server
counter guarantees no change is ever skipped.

Auth: single-user bearer token. Set with `wrangler secret put SYNC_TOKEN`.
Reject requests without it. HTTPS only (Cloudflare default).

### Client sync loop (Rust)

1. Collect notes where `dirty = 1`.
2. POST with `since_rev = meta.last_sync_rev`.
3. In one transaction: apply returned changes (keep newer `updated_at`), clear `dirty`
   on pushed notes **only if they weren't edited again during the request**
   (compare `updated_at`), save `server_rev`.
4. Triggers: app start, window focus, 3 s after last edit, every 5 min while open.
5. Exponential backoff on failure (5 s → 10 s → … max 5 min). Never block the UI.

### Conflict policy

Last-write-wins per note. Edge case: same note edited on two devices while both offline.
Mitigation (v1.1): if a pulled note conflicts with a local dirty note, keep the newer one
and save the other as a new note titled `Conflict copy – <title>`. Nothing is ever lost.

### Cost

Personal notes are a few MB total. Workers + D1 free tier covers this comfortably.
Check current free-tier limits on Cloudflare's pricing page before deploying.

---

## 10. Portable Mode

### Data location (`paths.rs`)

```
if  <exe_dir>/portable   (empty marker file) exists
    → data dir = <exe_dir>/data/
else
    → data dir = OS app-data dir (app.path().app_data_dir())
```

Data dir contains: `notes.db`, `settings.json`, `backups/`.
Also redirect the webview data folder into `<exe_dir>/data/webview/` in portable mode
so nothing is written to the host machine.

### Per platform

| Platform | Portable form                               | Notes                                                                                                                                                                     |
| -------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows  | `.zip` with `notes.exe` + `portable` marker | Needs WebView2 (preinstalled on Win 10/11). For machines without it, offer a second "full portable" zip using `webviewInstallMode: fixedRuntime` (much larger, ~150 MB+). |
| Linux    | AppImage                                    | Portable by design; add marker file support next to the AppImage.                                                                                                         |
| macOS    | `.app` in a zip                             | Mostly portable already; marker file inside the same folder as the `.app`.                                                                                                |

Portable builds disable auto-update (user replaces the exe manually).

---

## 11. Mobile (Android + iOS)

- Same Rust core and Svelte UI; `tauri android init` / `tauri ios init`.
- Single-instance and window-state plugins are desktop-only: gate with `#[cfg(desktop)]`.
- Responsive layout: list view and note view as separate screens on narrow widths.
- Larger tap targets for checkboxes (min 44×44 px).
- Sync on app resume (mobile lifecycle event) instead of window focus.
- **Android:** free to build and sideload the APK.
- **iOS:** free Apple account installs expire after 7 days; a paid Apple Developer
  account is needed for permanent install/TestFlight. Consider Android-first, or use
  iOS only if you already have an account.

---

## 12. Performance Checklist

### Rust / binary

```toml
[profile.release]
codegen-units = 1
lto = true
opt-level = "s"      # size-optimized; try 3 and compare, both are fast
panic = "abort"
strip = true
```

- `rusqlite` with `bundled` (no system SQLite mismatch), `reqwest` with `rustls-tls`
  and `default-features = false`.
- Only enable needed Tauri features; audit with `cargo tree` and `cargo bloat`.

### Frontend

- Svelte 5, no UI component library, no CSS framework.
- Vite: `build.target = 'es2022'`, minify on, no sourcemaps in release.
- Lazy-load CodeMirror markdown language and preview code.
- Note list sends summaries only; virtualize the list only if > 1,000 notes.
- No polling timers in JS; Rust emits events (`sync-status`, `notes-changed`).

### Startup

- Open DB and run migrations before window shows; show window with
  `visible: false` → `show()` after first render (no white flash).
- Load note list first, open last-used note second.

### Targets to measure (release build)

| Metric                   | Target                                   |
| ------------------------ | ---------------------------------------- |
| Cold start to usable     | < 300 ms desktop                         |
| Idle RAM                 | < 80 MB                                  |
| Search 5,000 notes       | < 20 ms                                  |
| Installer / portable zip | < 10 MB (excluding fixed WebView2 build) |

---

## 13. Stability Checklist

- **No data loss:** WAL mode, transactions for multi-step writes, save on close.
- **Backups:** daily automatic `VACUUM INTO backups/notes-YYYY-MM-DD.db`, keep last 7.
- **Trash:** deleted notes stay in trash 30 days before purge.
- **Errors:** typed `AppError` (thiserror), surfaced to UI as friendly messages;
  log to `data/logs/` with rotation (`tracing` + `tracing-appender`).
- **Single instance** on desktop to prevent concurrent DB writers.
- **Security:** minimal Tauri capabilities (only your commands + needed plugins),
  strict CSP, sanitize all rendered HTML, token never logged.
- **Tests:**
  - Rust unit tests: task toggling, title parsing, migrations, sync merge logic
  - Sync tests: simulated two-device offline edits, clock skew, interrupted requests
  - Worker tests with `wrangler dev` + Vitest
  - Manual smoke test checklist before each release
- **Export:** "Export all as .md files (zip)" — your data is never locked in.

---

## 14. Milestones

| #   | Milestone            | Deliverable                                                       |
| --- | -------------------- | ----------------------------------------------------------------- |
| 1   | Skeleton             | Tauri + Svelte app opens, SQLite created, migrations run          |
| 2   | Core notes           | Create/edit/delete, list, pin, trash, debounced save              |
| 3   | Tasks                | Checkbox widgets in editor, toggle shortcuts, auto-continue lists |
| 4   | Search + preview     | FTS5 search with highlights, Rust markdown preview                |
| 5   | Daily-driver test    | Use it yourself for a week; fix papercuts                         |
| 6   | Sync server          | Worker + D1 deployed, token auth                                  |
| 7   | Sync client          | Background sync, status indicator, backoff, conflict copies       |
| 8   | Portable + packaging | Portable zips/AppImage, installers, backups, export               |
| 9   | Mobile               | Android build, responsive layout; iOS optional                    |
| 10  | Hardening            | Tests, logging, perf measurements against targets                 |

---

## 15. Future Ideas (only if needed)

- Tags (`#tag` parsing, no separate UI needed at first)
- Global hotkey for quick-capture window (desktop)
- End-to-end encryption of note bodies before sync
- Note history via the server keeping old revisions
