mod backup;
mod db;
mod error;
mod export;
mod markdown;
mod notes;
mod paths;
#[cfg(test)]
mod perf;
mod settings;
mod sync;
mod util;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State, WindowEvent};

use error::{AppError, AppResult};
use notes::{ListFilter, Note, NoteSummary};
use settings::Settings;
use sync::{Sync, SyncConfig, SyncStatus, Trigger};

const TRASH_MAX_AGE_MS: i64 = 30 * util::DAY_MS;

/// When `run()` began, for the startup-time marker.
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub struct AppState {
    db: Mutex<Connection>,
    settings: Mutex<Settings>,
    sync: Sync,
    data_dir: PathBuf,
}

impl AppState {
    fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| AppError::Other("Database is unavailable".into()))
    }

    fn settings(&self) -> AppResult<MutexGuard<'_, Settings>> {
        self.settings
            .lock()
            .map_err(|_| AppError::Other("Settings are unavailable".into()))
    }

    fn sync_config(&self) -> Option<SyncConfig> {
        self.settings().ok()?.sync_config()
    }

    fn sync_on(&self) -> bool {
        self.sync_config().is_some()
    }
}

fn init_state(app: &AppHandle, sync: Sync) -> AppResult<AppState> {
    let data_dir = paths::data_dir(app)?;
    let settings = settings::load(&data_dir.join("settings.json"));
    let sync_on = settings.sync_config().is_some();

    let db_path = data_dir.join("notes.db");
    let conn = db::open(&db_path)?;
    notes::purge_old_trash(&conn, TRASH_MAX_AGE_MS, sync_on)?;
    notes::clean_up_purged(&conn, sync_on)?;

    let backup_db = db_path.clone();
    let backup_dir = data_dir.clone();
    std::thread::spawn(move || {
        if let Err(e) = backup::run_daily(&backup_db, &backup_dir) {
            eprintln!("Daily backup failed: {e}");
        }
    });

    Ok(AppState {
        db: Mutex::new(conn),
        settings: Mutex::new(settings),
        sync,
        data_dir,
    })
}

#[tauri::command]
async fn list_notes(state: State<'_, AppState>, filter: ListFilter) -> AppResult<Vec<NoteSummary>> {
    notes::list(&*state.conn()?, filter)
}

#[tauri::command]
async fn get_note(state: State<'_, AppState>, id: String) -> AppResult<Note> {
    notes::get(&*state.conn()?, &id)
}

#[tauri::command]
async fn save_note(state: State<'_, AppState>, id: String, body: String) -> AppResult<i64> {
    let updated_at = notes::save(&*state.conn()?, &id, &body)?;
    state.sync.edited();
    Ok(updated_at)
}

#[tauri::command]
async fn create_note(state: State<'_, AppState>) -> AppResult<Note> {
    notes::create(&*state.conn()?)
}

#[tauri::command]
async fn discard_if_empty(state: State<'_, AppState>, id: String) -> AppResult<bool> {
    let sync_on = state.sync_on();
    let discarded = notes::discard_if_empty(&*state.conn()?, &id, sync_on)?;
    if discarded {
        state.sync.edited();
    }
    Ok(discarded)
}

#[tauri::command]
async fn toggle_task(state: State<'_, AppState>, id: String, line_no: usize) -> AppResult<Note> {
    let note = notes::toggle_task(&*state.conn()?, &id, line_no)?;
    state.sync.edited();
    Ok(note)
}

#[tauri::command]
async fn set_pinned(state: State<'_, AppState>, id: String, pinned: bool) -> AppResult<()> {
    notes::set_pinned(&*state.conn()?, &id, pinned)?;
    state.sync.edited();
    Ok(())
}

#[tauri::command]
async fn trash_note(state: State<'_, AppState>, id: String) -> AppResult<()> {
    notes::set_deleted(&*state.conn()?, &id, true)?;
    state.sync.edited();
    Ok(())
}

#[tauri::command]
async fn restore_note(state: State<'_, AppState>, id: String) -> AppResult<()> {
    notes::set_deleted(&*state.conn()?, &id, false)?;
    state.sync.edited();
    Ok(())
}

#[tauri::command]
async fn purge_trash(state: State<'_, AppState>) -> AppResult<()> {
    let sync_on = state.sync_on();
    notes::purge_trash(&*state.conn()?, sync_on)?;
    state.sync.edited();
    Ok(())
}

#[tauri::command]
async fn search(state: State<'_, AppState>, query: String) -> AppResult<Vec<NoteSummary>> {
    notes::search(&*state.conn()?, &query)
}

#[tauri::command]
async fn render_markdown(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let body = notes::get(&*state.conn()?, &id)?.body;
    Ok(markdown::render(&body))
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    Ok(state.settings()?.clone())
}

/// Saves settings and returns them cleaned up (trimmed, URL normalized).
#[tauri::command]
async fn set_settings(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    let settings = settings.normalized()?;
    let previous = state.settings()?.clone();
    settings::save(&state.data_dir.join("settings.json"), &settings)?;

    let url_changed = previous.sync_url != settings.sync_url;
    let sync_on = settings.sync_config().is_some();
    *state.settings()? = settings.clone();
    {
        let conn = state.conn()?;
        if url_changed && sync_on {
            // A different server knows nothing about our notes: send them all.
            sync::reset_state(&conn)?;
        }
        if !sync_on {
            notes::clean_up_purged(&conn, false)?;
        }
    }
    if url_changed || previous.sync_token != settings.sync_token {
        state.sync.trigger(Trigger::Now);
    }
    Ok(settings)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    portable: bool,
    /// Phones have no file manager to "reveal" a folder in.
    mobile: bool,
    data_dir: String,
    db_path: String,
}

#[tauri::command]
async fn get_storage_info(state: State<'_, AppState>) -> AppResult<StorageInfo> {
    Ok(StorageInfo {
        portable: paths::portable_dir().is_some(),
        mobile: cfg!(mobile),
        data_dir: state.data_dir.to_string_lossy().into_owned(),
        db_path: state.data_dir.join("notes.db").to_string_lossy().into_owned(),
    })
}

/// Exports every note as `.md` files in a zip. Returns where it went, or
/// `None` if the user cancelled the Android "Save as" picker.
#[cfg(not(target_os = "android"))]
#[tauri::command]
async fn export_notes(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<String>> {
    let dir = paths::export_dir(&app, &state.data_dir);
    let path = export::export_zip(&*state.conn()?, &dir)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Android apps can't write to Downloads directly, so the user picks a place
/// (Downloads, Drive, …) in the system picker and we write to what it returns.
#[cfg(target_os = "android")]
#[tauri::command]
async fn export_notes(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<String>> {
    use std::io::Write;
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_fs::{FsExt, OpenOptions};

    let bytes = export::export_bytes(&*state.conn()?)?;
    let name = format!("{}.zip", export::export_stem());
    let Some(target) = app
        .dialog()
        .file()
        .add_filter("Zip", &["zip"])
        .set_file_name(&name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut file = app.fs().open(target, opts)?;
    file.write_all(&bytes)?;
    file.flush()?;
    Ok(Some(name))
}

/// Called once by the UI right after the window is shown with the note list.
/// Prints a marker that measurement scripts read from stderr / logcat:
/// `ferrite-startup: shown_unix_ms=<ms> since_run_ms=<ms>`.
#[tauri::command]
async fn startup_done() -> AppResult<()> {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let since_run = STARTED.get().map_or(0, |t| t.elapsed().as_millis());
        eprintln!("ferrite-startup: shown_unix_ms={} since_run_ms={since_run}", util::now_ms());
    });
    Ok(())
}

#[tauri::command]
async fn get_sync_status(state: State<'_, AppState>) -> AppResult<SyncStatus> {
    Ok(state.sync.status())
}

#[tauri::command]
async fn sync_now(app: AppHandle) -> AppResult<SyncStatus> {
    Ok(sync::sync_once(&app).await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    STARTED.get_or_init(std::time::Instant::now);
    #[cfg(desktop)]
    let portable = paths::portable_dir();
    // TLS for sync uses rustls with the small `ring` crypto backend.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Keep the webview's own data beside the app too, so portable mode
    // writes nothing to the host machine.
    #[cfg(windows)]
    if let Some(dir) = &portable {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir.join("webview"));
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // Installed mode only: a portable copy has its own data folder, so it may run
    // beside the installed app (handy as a second "device" for testing sync).
    #[cfg(desktop)]
    if portable.is_none() {
        use tauri_plugin_window_state::StateFlags;
        // Must be the first plugin registered.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::new()
                // The UI shows the window itself after first render. The frame is
                // always off (we draw our own title bar), so never restore an old one.
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE & !StateFlags::DECORATIONS)
                .build(),
        );
    }

    #[cfg(target_os = "android")]
    {
        builder = builder
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init());
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (sync, triggers) = Sync::new();
            app.manage(init_state(app.handle(), sync)?);
            tauri::async_runtime::spawn(sync::run_loop(app.handle().clone(), triggers));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Phones have no focus event worth using; they resume instead.
            #[cfg(mobile)]
            let came_back = matches!(event, WindowEvent::Focused(true) | WindowEvent::Resumed);
            #[cfg(desktop)]
            let came_back = matches!(event, WindowEvent::Focused(true));
            if came_back {
                if let Some(state) = window.try_state::<AppState>() {
                    state.sync.trigger(Trigger::Focused);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_notes,
            get_note,
            save_note,
            create_note,
            discard_if_empty,
            toggle_task,
            set_pinned,
            trash_note,
            restore_note,
            purge_trash,
            search,
            render_markdown,
            get_settings,
            set_settings,
            get_sync_status,
            sync_now,
            get_storage_info,
            export_notes,
            startup_done,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ferrite Notes");
}
