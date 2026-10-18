mod backup;
mod db;
mod error;
mod markdown;
mod notes;
mod paths;
mod settings;
mod util;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use error::{AppError, AppResult};
use notes::{ListFilter, Note, NoteSummary};
use settings::Settings;

const TRASH_MAX_AGE_MS: i64 = 30 * util::DAY_MS;

pub struct AppState {
    db: Mutex<Connection>,
    settings: Mutex<Settings>,
    data_dir: PathBuf,
}

impl AppState {
    fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.db.lock().map_err(|_| AppError::Other("Database is unavailable".into()))
    }

    fn settings(&self) -> AppResult<MutexGuard<'_, Settings>> {
        self.settings.lock().map_err(|_| AppError::Other("Settings are unavailable".into()))
    }
}

fn init_state(app: &AppHandle) -> AppResult<AppState> {
    let data_dir = paths::data_dir(app)?;
    let db_path = data_dir.join("notes.db");
    let conn = db::open(&db_path)?;
    notes::purge_old_trash(&conn, TRASH_MAX_AGE_MS)?;

    let backup_db = db_path.clone();
    let backup_dir = data_dir.clone();
    std::thread::spawn(move || {
        if let Err(e) = backup::run_daily(&backup_db, &backup_dir) {
            eprintln!("Daily backup failed: {e}");
        }
    });

    Ok(AppState {
        db: Mutex::new(conn),
        settings: Mutex::new(settings::load(&data_dir.join("settings.json"))),
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
    notes::save(&*state.conn()?, &id, &body)
}

#[tauri::command]
async fn create_note(state: State<'_, AppState>) -> AppResult<Note> {
    notes::create(&*state.conn()?)
}

#[tauri::command]
async fn discard_if_empty(state: State<'_, AppState>, id: String) -> AppResult<bool> {
    notes::discard_if_empty(&*state.conn()?, &id)
}

#[tauri::command]
async fn toggle_task(state: State<'_, AppState>, id: String, line_no: usize) -> AppResult<Note> {
    notes::toggle_task(&*state.conn()?, &id, line_no)
}

#[tauri::command]
async fn set_pinned(state: State<'_, AppState>, id: String, pinned: bool) -> AppResult<()> {
    notes::set_pinned(&*state.conn()?, &id, pinned)
}

#[tauri::command]
async fn trash_note(state: State<'_, AppState>, id: String) -> AppResult<()> {
    notes::set_deleted(&*state.conn()?, &id, true)
}

#[tauri::command]
async fn restore_note(state: State<'_, AppState>, id: String) -> AppResult<()> {
    notes::set_deleted(&*state.conn()?, &id, false)
}

#[tauri::command]
async fn purge_trash(state: State<'_, AppState>) -> AppResult<()> {
    notes::purge_trash(&*state.conn()?)
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

#[tauri::command]
async fn set_settings(state: State<'_, AppState>, settings: Settings) -> AppResult<()> {
    settings::save(&state.data_dir.join("settings.json"), &settings)?;
    *state.settings()? = settings;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let portable = paths::portable_dir();

    // Keep the webview's own data beside the app too, so portable mode
    // writes nothing to the host machine.
    #[cfg(windows)]
    if let Some(dir) = &portable {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir.join("webview"));
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        // Must be the first plugin registered.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
        if portable.is_none() {
            use tauri_plugin_window_state::StateFlags;
            builder = builder.plugin(
                tauri_plugin_window_state::Builder::new()
                    // The UI shows the window itself after first render.
                    .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                    .build(),
            );
        }
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = init_state(app.handle())?;
            app.manage(state);
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ferrite Notes");
}
