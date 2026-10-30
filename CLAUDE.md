# Ferrite Notes – project context

This is the hand-off file for a new chat. Read it first, then `PLAN.md` (the original
plan) and `TESTING.md` (the manual test checklist). Last updated 2026-10-29.

Ferrite Notes is a fast, light, offline-first notes app for quick notes, task lists and bug
lists. It has clickable checkboxes, a markdown preview, full-text search, pin/trash, and
optional free sync through the user's own Cloudflare Worker. Targets: Windows (installer +
portable zip), Linux, macOS, Android, and optionally iOS.

## Status

| # | Milestone (PLAN.md §14) | State |
|---|---|---|
| 1–4 | Skeleton, core notes, tasks, search + preview | ✅ Done, committed |
| 5 | Daily-driver week | ⏳ In progress: the user is using the app and collecting papercuts |
| 6–7 | Sync server (Worker + D1) and sync client | ✅ Done, deployed, two-device test passed |
| 8 | Portable zip, export, backups | ✅ Windows done. Linux AppImage / macOS .app not yet (need CI) |
| — | SQLite size trim | ✅ Done (exe 6.96 → 6.73 MB, zip ~3.2 MB) |
| 9 | Android (+ mobile layout) | ⏭ Next. Prerequisites listed below |
| — | Design / UX pass | After milestone 9, so desktop and phone are polished together |
| 10 | Hardening: logging, perf targets, CI releases | After the design pass |

Commits so far: `f8829a5` (M1–4) → `61ba0fe` (multi-check, icons) → `ee54d78` (sync) →
`c4f2296` (M8) → `298d036` (SQLite trim). The branch is `main`, with no remote yet.

## Layout

```
src/                         Svelte 5 frontend (runes)
  App.svelte                 layout, toolbar, global shortcuts, close/blur save
  lib/api.ts                 typed invoke() wrappers + shared types
  lib/store.svelte.ts        all UI state; debounced, serialized saves; sync events
  lib/editor/setup.ts        minimal CodeMirror 6 + its theme (fonts/padding live HERE)
  lib/editor/checkbox.ts     task widgets, [] shortcut, Mod-Enter, Enter continues lists
  lib/editor/markdown.ts     lazily loaded markdown language + highlight style
  lib/components/            Sidebar, NoteList, Editor, Preview, SettingsDialog, Icon
src-tauri/                   Rust core
  src/lib.rs                 AppState, commands, plugin setup, sync loop spawn
  src/db.rs                  open, pragmas, migrations (PRAGMA user_version)
  src/notes.rs               CRUD, list summaries, FTS search, task toggle, purge
  src/sync.rs                sync client, background loop, two-device tests (FakeServer)
  src/markdown.rs            pulldown-cmark + ammonia; checkboxes carry data-line
  src/export.rs              "Export all notes" → zip of .md files
  src/paths.rs               portable vs installed data dir, export dir
  src/settings.rs            settings.json (theme, sync URL/token), validation
  src/backup.rs              daily VACUUM INTO backups/, keep 7
  migrations/001_init.sql, 002_sync.sql
  .cargo/config.toml         LIBSQLITE3_FLAGS (trimmed SQLite build)
worker/                      Cloudflare Worker sync server (TypeScript, D1)
  src/index.ts               POST /sync, conflict copies, tombstones
  scripts/setup.mjs          one-command deploy (npm run setup)
  test/sync.test.ts          Vitest in the Workers runtime (@cloudflare/vitest-plugin)
scripts/package-portable.mjs builds release/Ferrite-Notes-<ver>-portable-windows-<arch>.zip
```

## Commands

Run these from the repo root unless noted.

| What | Command |
|---|---|
| Run the app (dev) | `npm run tauri dev` |
| Frontend type-check / build | `npm run check` / `npm run build` |
| Rust tests (23) | `cd src-tauri && cargo test --lib` |
| Rust lint | `cd src-tauri && cargo clippy --all-targets` |
| Portable zip | `npm run package:portable` → `release/` |
| Worker tests (11) / types | `cd worker && npm test` / `npm run typecheck` |
| Deploy the Worker the first time | `cd worker && npx wrangler login && npm run setup` |
| Redeploy the Worker (with migrations) | `cd worker && npm run deploy` |
| Size per crate | `cd src-tauri && CARGO_TARGET_DIR=target/bloat CARGO_PROFILE_RELEASE_STRIP=false cargo bloat --release --crates -n 25` |

Run the Rust commands from `src-tauri/`, so `.cargo/config.toml` (the SQLite flags) applies.

## Key decisions (and why)

**Data and storage**
- **Notes table has `rid INTEGER PRIMARY KEY` plus `id TEXT UNIQUE` (UUID v7).** FTS5
  external-content rowids must stay stable, and `VACUUM` can renumber implicit rowids.
- **`deleted` uses 0 = live, 1 = trash, 2 = purged.** A purged note is a tombstone with its
  body cleared, kept only until the purge reaches the server. With sync off it is
  hard-deleted right away. Empty never-typed notes are discarded on leave
  (`discard_if_empty`).
- **`updated_at` never goes backwards:** `MAX(now, updated_at + 1)`. Sync's
  last-write-wins relies on this.
- **Trash auto-purges after 30 days**, at startup. Daily backups keep the last 7.

**Sync**
- **Sync protocol:** a single `POST /sync` carries `{since_rev, changes[{…, base_rev}]}`.
  The server answers with `{server_rev, more, reset, changes[{…, rev}]}`.
  - The cursor is a server counter `rev`, not timestamps, so wrong device clocks don't matter.
  - Conflicts are resolved **on the server**. The newer `updated_at` wins. The losing
    version becomes a new note "Conflict copy – <title>" when it was unseen (`rev > base_rev`)
    and has a different body. Nothing is ever lost.
  - The server always re-stamps the rev of every pushed note, so the client receives an
    echo and can clear `dirty`. A push retried after a lost response is harmless.
  - The client takes the server version for clean notes, and for dirty notes whose pushed
    `updated_at` still matches the local one. Otherwise it keeps local edits.
  - `reset: true` (cursor ahead of the server, e.g. a new database) and changing the sync
    URL both re-push everything.
  - The server batch uses an optimistic lock: a CHECK-constraint trick on the `counter`
    table, retried up to 5 times.
- **Sync triggers:** app start, window focus, 3 s after the last edit, every 5 min, and
  "Sync now". Backoff on failure is 5 s → 5 min. Rust emits `sync-status` and
  `notes-changed` events; there is no JS polling.
- **TLS:** reqwest + rustls with the `ring` provider (`rustls-no-provider` + installing the
  provider in `run()`). This avoids aws-lc-rs and OpenSSL.
- **Each user deploys their own Worker** (free Cloudflare account). There is no shared
  server: the app is single-user, has no hosted costs, and holds no one else's notes.
  Other sync backends are deferred until the whole plan is done. Ideas: a "sync folder"
  with per-device change logs (Dropbox/Drive/Syncthing), Google Drive OAuth later.

**Portable mode**
- An empty `portable` (or `portable.txt`) file next to the exe makes the app store
  everything in `./data`, including the WebView2 data. In portable mode the
  single-instance and window-state plugins are skipped. That is also how to test two
  "devices" on one PC.

**Editor and UI**
- **Editor:** a minimal CodeMirror setup. Task bullets `- ` are hidden behind the checkbox
  widget (an atomic range, so Backspace removes the whole box). Checked tasks are muted
  with **no strikethrough** (user preference). Typing `[]` at a line start makes a task.
  Mod-Enter works across a multi-line selection: all tasks → check all, or uncheck if all
  are checked; otherwise it turns non-task lines into tasks.
- **Preview mode is remembered per note** (localStorage `ferrite:previewNotes`), never
  global.
- **The HTML sanitizer (ammonia) stays.** The user decided to keep raw-HTML rendering.
- **Toolbar uses SVG icons** with tooltips and aria-labels (Lucide paths, ISC license).
  The header is a fixed 40 px.
- **Size:** we use `opt-level = "s"`, LTO, `panic = "abort"`, and strip. No UPX (antivirus
  false positives) and no `opt-level = "z"` (slower). The remaining ~6.7 MB is the floor:
  std + Tauri + SQLite + TLS. Of the icons, only `icon.ico` (9 KB) goes into the Windows
  exe, so optimizing the icons isn't worth it.

## Conventions

**Rust**
- All commands are `async fn … -> AppResult<T>`. No `unwrap`/`expect` in command paths.
  Errors serialize to a friendly string.
- One `Mutex<Connection>`. Never hold its guard across `.await`. Use `&*state.conn()?`;
  the explicit deref is needed for the `?` operator to infer the type.
- Use `prepare_cached`. Keep pure DB logic in plain functions that take
  `&Connection`, so they can be unit-tested with `db::open_in_memory()`.
- Every new schema change is a new `migrations/00N_*.sql` added to `MIGRATIONS` in
  `db.rs`.

**Frontend**
- Svelte 5 runes and plain CSS variables (`styles.css`, with light/dark plus a
  `data-theme` override). No UI library.
- CodeMirror styling goes in `EditorView.theme` in `setup.ts`. Plain CSS loses to its
  base theme.

**Git and secrets**
- Commit only when the user asks. End commit messages with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Stage with
  `git add -A -- . ':!.claude'`, since `.claude/` (launch.json) stays untracked.
- `worker/wrangler.toml` is **`git update-index --skip-worktree`**. The repo has the
  placeholder `database_id = "00000000-…"`, while the local file has the real D1 id. Keep
  it that way, so the real id never gets committed before open-sourcing. Run
  `git update-index --no-skip-worktree worker/wrangler.toml` only to intentionally change
  the committed version, then restore the placeholder.
- Never write the sync token into files, logs, or chat. Settings' `Debug` redacts it.

## Working with this user

- The user tests by hand. When a step is slow, give them clear numbered steps or commands
  (in `TESTING.md` style) instead of waiting, and say what was and wasn't verified.
- They decide scope. For example: they kept the sanitizer, chose only the SQLite trim, and
  want to finish the whole PLAN.md before exploring other sync backends.
- Windows 11, PowerShell. Node 24, Rust 1.98, Android Studio already installed.
- Gotchas seen so far:
  - Windows hides `.txt`, which is why `portable.txt` is accepted.
  - A new Cloudflare account must register a `workers.dev` subdomain; `setup.mjs` now
    handles that interactively.
  - The sync URL is `https://<worker-name>.<account-subdomain>.workers.dev`. The user was
    renaming the account subdomain to fix a repeated name; check Settings for the current
    URL.
  - Git Bash's `tar` can't write zips, so the packaging script calls
    `%SystemRoot%\System32\tar.exe`.

## What's left

**Now: milestone 5, a week of daily use.** The user keeps a "Papercuts" task note in the
app. Start the next session by asking for that list.

**Next: milestone 9, Android.**
- User prerequisites:
  - In Android Studio's SDK Manager: one SDK platform (API 34/35), Build-Tools,
    Command-line Tools, Platform-Tools, and the NDK.
  - Set `JAVA_HOME` (Android Studio `jbr`), `ANDROID_HOME`, and `NDK_HOME`.
  - Run `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`.
  - Turn on USB debugging or set up an emulator.
  - Check everything with `npx tauri info`.
- Work:
  - Run `tauri android init` to create `src-tauri/gen/android`.
  - Phone layout: list and note as separate screens (a 700 px breakpoint already exists),
    the Android back gesture going back to the list, safe areas, the keyboard not covering
    the editor, and 44 px tap targets.
  - Sync on app resume, since phones have no focus event.
  - **rustls-platform-verifier needs Android-specific init.** Either do that, or use
    webpki-roots on Android.
  - Export: `revealItemInDir` doesn't exist on Android, so use a share sheet or a save
    dialog (plugin-dialog).
  - Make a release APK signing key.
- Expected APK size: about 5–8 MB for arm64 only.

**Then:** a design pass (desktop and mobile together), followed by milestone 10:
- `tracing` logs in `data/logs` with rotation.
- Measure the plan's targets: cold start under 300 ms, idle RAM under 80 MB, search of
  5,000 notes under 20 ms.
- GitHub Actions to build the Windows zip/installer, Linux AppImage, and macOS .app on
  release tags.
- iOS is optional (it needs a paid Apple account).

**Later or optional:**
- PLAN.md §15 ideas: tags, a quick-capture hotkey, end-to-end encryption, note history.
- Alternative sync backends.
- Root `README.md` polish before open-sourcing.
