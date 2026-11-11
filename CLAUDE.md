# Ferrite Notes – project context

Project context for contributors and AI coding assistants (Claude Code reads this file
automatically). Read it first, then `PLAN.md` (the original plan) and `TESTING.md` (the
manual test checklist). Human contributors: see also `CONTRIBUTING.md`. Last updated
2026-11-11.

Ferrite Notes is a fast, light, offline-first notes app for quick notes, task lists and bug
lists. It has clickable checkboxes, a markdown preview, full-text search, pin/trash, and
optional free sync through your own Cloudflare Worker. Targets: Windows (installer +
portable zip), Linux, macOS, Android, and optionally iOS.

## Status

| # | Milestone (PLAN.md §14) | State |
|---|---|---|
| 1–4 | Skeleton, core notes, tasks, search + preview | ✅ Done, committed |
| 5 | Daily-driver week | ✅ Done: used daily for over a week |
| 6–7 | Sync server (Worker + D1) and sync client | ✅ Done, deployed, two-device test passed |
| 8 | Portable zip, export, backups | ✅ Windows done. Linux AppImage / macOS .app not yet (need CI) |
| — | SQLite size trim | ✅ Done (exe 6.96 → 6.73 MB, zip ~3.2 MB) |
| 9 | Android (+ mobile layout) | 🚧 Works on a real phone (TESTING.md passed); release APK left |
| — | Design / UX pass | ✅ Done (2026-11-11): macOS (default), Gaming, Terminal (CRT), Retro, E-ink; own title bar; Fe icon. On desktop and phone |
| 10 | Hardening: perf targets, CI releases | Perf measured and tuned (2026-11-10). Logs dropped. CI after the design pass |

Commits so far: `f8829a5` (M1–4) → `61ba0fe` (multi-check, icons) → `ee54d78` (sync) →
`c4f2296` (M8) → `298d036` (SQLite trim) → `1f4fb7a` (collapsible sidebar) →
`f27f243` (Android M9) → `dc360fe` (perf) → `f54f303` (design pass) → the Retro bezel fix. The branch is `main`, with no remote yet.

## Layout

```
src/                         Svelte 5 frontend (runes)
  App.svelte                 layout, toolbar, global shortcuts, close/blur save, back gesture
  themes.css                 every theme's variables, scrollbars, checkboxes, Retro/Terminal looks
  assets/retro/              generated worn-metal textures (WebP)
  lib/api.ts                 typed invoke() wrappers + shared types
  lib/store.svelte.ts        all UI state; debounced, serialized saves; sync events
  lib/editor/setup.ts        minimal CodeMirror 6 + its theme (fonts/padding live HERE)
  lib/editor/checkbox.ts     task widgets, [] shortcut, Mod-Enter, Enter continues lists
  lib/editor/markdown.ts     lazily loaded markdown language + highlight style
  lib/components/            Sidebar, NoteList, Editor, Preview, SettingsDialog, Titlebar, Icon
src-tauri/                   Rust core
  src/lib.rs                 AppState, commands, plugin setup, sync loop spawn
  src/db.rs                  open, pragmas, migrations (PRAGMA user_version)
  src/notes.rs               CRUD, list summaries, FTS search, task toggle, purge
  src/sync.rs                sync client, background loop, two-device tests (FakeServer)
  src/markdown.rs            pulldown-cmark + ammonia; checkboxes carry data-line
  src/export.rs              "Export all notes" → zip of .md files
  src/paths.rs               portable vs installed data dir, export dir
  src/settings.rs            settings.json (look, theme, sync URL/token), validation
  src/perf.rs                ignored benchmark: 5,000 notes, search timing
  src/backup.rs              daily VACUUM INTO backups/, keep 7
  migrations/001_init.sql, 002_sync.sql
  .cargo/config.toml         LIBSQLITE3_FLAGS (trimmed SQLite build)
worker/                      Cloudflare Worker sync server (TypeScript, D1)
  src/index.ts               POST /sync, conflict copies, tombstones
  scripts/setup.mjs          one-command deploy (npm run setup)
  test/sync.test.ts          Vitest in the Workers runtime (@cloudflare/vitest-plugin)
scripts/package-portable.mjs builds release/Ferrite-Notes-<ver>-portable-windows-<arch>.zip
scripts/gen-retro-textures.py  makes src/assets/retro/*.webp (Pillow + NumPy)
scripts/gen-icon.py          draws src-tauri/icons/app-icon.png (then `npx tauri icon`)
docs/screenshots/            README images, one per theme (taken from the real app)
src-tauri/gen/android/       the generated Android project (MainActivity.kt has IME insets)
```

## Commands

Run these from the repo root unless noted.

| What | Command |
|---|---|
| Run the app (dev) | `npm run tauri dev` |
| Frontend type-check / build | `npm run check` / `npm run build` |
| Rust tests (23) | `cd src-tauri && cargo test --lib` |
| Search benchmark (5,000 notes) | `cd src-tauri && cargo test --release --lib perf -- --ignored --nocapture` |
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
- **Sync triggers:** app start, window focus or phone resume (only if the last sync is
  30 s or older, `Trigger::Focused`: clicking the custom title bar refocuses the window),
  3 s after the last edit, every 5 min, and "Sync now". Backoff on failure is 5 s → 5 min. Rust emits `sync-status` and
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
  with **no strikethrough** (a deliberate choice). Typing `[]` at a line start makes a task.
  Mod-Enter works across a multi-line selection: all tasks → check all, or uncheck if all
  are checked; otherwise it turns non-task lines into tasks.
- **Preview mode is remembered per note** (localStorage `ferrite:previewNotes`), never
  global.
- **The HTML sanitizer (ammonia) stays.** Raw-HTML rendering is kept, sanitized.
- **Web images in the preview:** the CSP allows `img-src … https:` (not `http:`), and
  images are capped at the note's width. Opening a note with a web image downloads it,
  so that site sees your IP address, the same as a browser would. Some sites
  (Wikimedia) refuse images embedded in other apps, which isn't a bug.
- **Sidebar collapses** with `Ctrl+B` or the toolbar's panel icon, remembered in localStorage
  (`ferrite:sidebarCollapsed`). It doesn't apply under 700 px, where list and note are
  separate screens.
- **Themes ("looks")**: macOS (the default), Gaming, Terminal, Retro
  and E-ink. Settings stores `look` (the theme) and `theme` (the mode: system, light or
  dark). Unknown values fall back to macOS and system.
  - `store.applyAppearance()` puts `data-look` and the resolved `data-mode` on `<html>`,
    and follows the system's light/dark setting. Gaming, Terminal and Retro are dark-only,
    so Settings hides the mode switch for them.
  - `themes.css` holds every look's variables, plus thin rounded scrollbars
    (`::-webkit-scrollbar`) and checkboxes drawn by the theme (`appearance: none`; the
    tick is a data-URI SVG in `--check-icon`).
  - Small touches per look: Gaming has a cyan bar on the open note and a cut-corner +
    button; E-ink outlines the selected note.
  - **Terminal is a vintage CRT.** A dark bezel around a screen with rounded corners,
    scanlines and an edge fade (`.app::after`, `pointer-events: none`), a phosphor glow
    (`text-shadow`), the selected note in inverse video, boxed buttons, a blinking block
    cursor, and `# ` before the title. The screen is square: rounded corners didn't read as
    a CRT, and a real bend would blur the text.
  - **Retro is worn cassette-futurism hardware**, made to look realistic:
    - Textures: scratched, rusty grey paint for the casing (`.window`), chipped mustard
      paint for the panel (the sidebar, which re-scopes `--text`, `--surface` and so on),
      and aged brass for the bezel around the LCD (`.pane`).
    - The textures are seamless WebP tiles in `src/assets/retro/` (about 140 KB). They're
      generated procedurally by `scripts/gen-retro-textures.py` (Pillow + NumPy, no
      photos, no licence), so the build doesn't need Python.
    - Screws are an SVG data URI in `--screw`. Positions in the `background` shorthand
      must be four-value (`left 8px bottom 8px`): `8px bottom 8px` is invalid, and one
      invalid value drops the whole background.
    - The panel is worn most at the edges; a radial wash keeps the text in the middle
      readable.
    - Keys look like the hardware's: a domed face with grime at the edges, set in a black
      well (a `box-shadow` ring). The window keys are printed MIN / MAX / OFF, and OFF
      (close) is orange. Settings is a screwed-on mustard panel with LCD fields and the
      same keys.
    - The bezel has the panel's corner radius (`--radius-lg`, 12 px). The LCD is its own
      rounded layer (`.pane::before`, 8 px radius, inset by `--bezel`), so it stays rounded
      inside. The scanlines (`.pane::after`) use the same inset.
    - A TX/RX lamp in the title bar is green when idle, blinks amber while saving or
      syncing, and goes red on an error.
  - The block cursor needs CodeMirror's `drawSelection()`, since the native caret can't be
    a block. The selection colour is `--selection`.
  - **Glass was removed.** It looked poor: washed-out text, see-through
    Settings, and it depended on what was behind the window. The window is no longer
    transparent.
- **Own title bar** (`decorations: false`, `Titlebar.svelte`, desktop only). It drags the
  window (`data-tauri-drag-region`, double-click maximizes). macOS gets the three dots on
  the left; the other looks get Windows-style buttons on the right. Each look styles it
  with `--titlebar-*`. The accepted cost: no Windows 11 Snap Layouts
  menu on the maximize button, and no Alt+Space window menu.
- **No typing sound.** It was built and then removed (2026-11-11), together with the idea
  of real Mechvibes-style recordings.
  - Fonts are system fonts only, nothing bundled: Segoe UI (macOS), Bahnschrift
    (Gaming), Cascadia Mono / Consolas (Terminal), Georgia / Noto Serif (E-ink).
  - Rejected along the way: warm cream with terracotta (the old look), four calmer themes
    (Graphite, Field, Ferrofluid, Classic), Glass, and the Sci-fi and Blueprint ideas.
  - Opening `localhost:1420` in a normal browser fails ("reading 'invoke'"). That's
    expected: only the app window can reach Rust.
- **List and title:** list previews drop markdown markers and finished tasks, and join
  list items with ", ". Notes with tasks show a done/total badge (`tasks_done`,
  `tasks_total` from `notes::task_counts`, counted over the whole body). The editor's
  first line gets `cm-title-line`, so it looks like a title.
- **Toolbar uses SVG icons** with tooltips and aria-labels (Lucide paths, ISC license).
  The header is a fixed 40 px.
- **Android:**
  - Back gesture: Android's WebView goes back in history when it can, so `App.svelte`
    pushes a history entry per screen you can back out of (note on a narrow screen,
    Settings) and `popstate` closes the top one.
  - Edge to edge: `.app` pads with `env(safe-area-inset-*)` (the WebView reports them).
    `MainActivity.kt` pads the content view by the keyboard (IME) height, because
    edge-to-edge apps aren't resized for it.
  - Sync TLS uses the bundled Mozilla roots (`webpki-root-certs`, Android only), not the
    platform verifier, which needs JNI setup there.
  - Sync triggers on `WindowEvent::Resumed`; the page flushes saves on `visibilitychange`.
  - Export: the system "Save as" picker (plugin-dialog) and plugin-fs to write the
    `content://` URI, both registered **from Rust, Android only**. There are no JS
    bindings or capabilities for them. `get_storage_info` returns `mobile` to hide
    "Open data folder".
  - Touch screens (`pointer: coarse`) get 44 px tap targets; the header is 48 px there.
    Exception: task checkboxes are 18 px with no vertical margin. Bigger boxes looked too
    large and made task lines taller than text lines.
- **Performance** (measured 2026-11-10 on a Windows 11 PC, release build, portable copy):

  | | Empty | 5,000 notes | Target |
  |---|---|---|---|
  | Start → window shown | ~320 ms | ~400 ms | < 300 ms |
  | Search | | ~5 ms | < 20 ms ✅ |
  | Our process (private) | 5.7 MB | 8 MB | |
  | With WebView2's 6 helpers (private) | ~100 MB | ~126 MB | < 80 MB |

  - The start and RAM targets can't be met with WebView2. About 300 ms of the start is
    WebView2 starting before our page loads (our page itself takes about 100 ms), and
    the helpers hold about 95 MB. Our code is well within budget, so these targets were relaxed (2026-11-10).
  - `--disable-gpu` (`additionalBrowserArgs` in `tauri.conf.json`, which must repeat
    Tauri's default `--disable-features=…`) dropped memory from 155 to 102 MB and the
    start from 355 to 323 ms. The GPU process alone was 68 MB. Drawing on the CPU is
    fine for a text app. `--in-process-gpu --renderer-process-limit=1` saved only 8 MB
    more, and is less stable, so it's not used.
  - The note list renders in pages of 150 as you scroll, and `store.list` is
    `$state.raw`. With 5,000 notes, rendering every row cost 800 ms and 360 MB.
  - Android (debug APK, release `.so`): tap to list in about 1.1 s. That's 370 ms
    process, 395 ms WebView start, and 375 ms page. PSS is about 170 MB plus the WebView
    renderer. Phones have no target.
  - How to measure: the UI calls `startup_done` after the first paint, and Rust prints
    `ferrite-startup: shown_unix_ms=… since_run_ms=…` to stderr (logcat on Android).
    Launch a portable copy with stderr redirected, wait for that line, then add up
    `PrivatePageCount` over the process tree (Win32_Process). For a breakdown, set
    `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` and read
    `performance` timings over CDP. Seed a 5,000-note database with
    `FERRITE_SEED_DB=<path>` plus the benchmark command.
  - **No log files.** They aren't worth it for a local app that never sends data
    anywhere. Only the startup marker and `eprintln!` remain.
- **App icon:** iron's periodic-table tile: a big silver "Fe" on brushed dark iron, with
  26 and 55.845. `scripts/gen-icon.py` draws the 1024 px master
  `src-tauri/icons/app-icon.png`, and `npx tauri icon src-tauri/icons/app-icon.png`
  makes every size, including the Android mipmaps.
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
- Svelte 5 runes and plain CSS variables. No UI library. Colours, radii and fonts are
  theme variables in `themes.css`. Components use only `var(--…)`, never hex values.
- CodeMirror styling goes in `EditorView.theme` in `setup.ts`. Plain CSS loses to its
  base theme.

**Git and secrets**
- AI assistants: commit only when asked. Stage with `git add -A -- . ':!.claude'`;
  `.claude/` (local launch config) stays untracked.
- `worker/wrangler.toml` keeps the placeholder `database_id = "00000000-…"` in git.
  `npm run setup` writes your real D1 id into your local copy; never commit it. Tip:
  `git update-index --skip-worktree worker/wrangler.toml` hides that local change.
- Never write a sync token into files, logs, or chat. Settings' `Debug` redacts it.

## Environment notes

- Developed on Windows 11 with PowerShell, Node 24 and Rust 1.98; Android builds use
  Android Studio's SDK and NDK 27.1. Contributors on macOS/Linux use the same commands.
- Testing is mostly by hand: when a step is slow, write clear numbered steps (in
  `TESTING.md` style) and say what was and wasn't verified.
- Gotchas seen so far:
  - Windows hides `.txt`, which is why `portable.txt` is accepted.
  - A new Cloudflare account must register a `workers.dev` subdomain; `setup.mjs`
    handles that interactively.
  - The sync URL is `https://<worker-name>.<account-subdomain>.workers.dev`.
  - Git Bash's `tar` can't write zips, so the packaging script calls
    `%SystemRoot%\System32\tar.exe`.
  - A running `ferrite-notes.exe` in `src-tauri/target/release` blocks
    `npm run package:portable` ("Access is denied"); close the app first.

## What's left

**Now: milestone 9, Android.** The toolchain needs the SDK, NDK 27.1, `JAVA_HOME`,
`NDK_HOME` and the Android Rust targets; `src-tauri/gen/android` is committed.
- Done (see "Android" under Key decisions): back gesture, safe areas, keyboard insets,
  tap targets, sync on resume, TLS roots, export via "Save as".
- To do:
  - The Android section of `TESTING.md` passed on a real phone, sync included. (With
    the phone locked, sync gets a DNS error: Android blocks background network access.)
  - The `.so` is linked for 16 KB pages (`max-page-size=16384` in `.cargo/config.toml`).
    Without it, Android 15+ shows a compatibility popup on every launch of a debug build,
    and Google Play rejects the app.
  - On Windows, turn on Developer Mode, or `tauri android build` fails at the symlink step.
    Until then: build, copy `target/aarch64-linux-android/<profile>/libferrite_notes_lib.so`
    into `gen/android/app/src/main/jniLibs/arm64-v8a/`, then run
    `gradlew assembleArm64Debug -x rustBuildArm64Debug`.
  - **Daily-use APK (~17 MB):** the **release** Rust library inside the **debug** APK, so
    no signing key is needed and it updates in place.
    1. `npx tauri android build --apk --target aarch64` builds the release `.so` (it then fails at the
       symlink step; ignore that).
    2. Copy `src-tauri/target/aarch64-linux-android/release/libferrite_notes_lib.so` into
       `gen/android/app/src/main/jniLibs/arm64-v8a/`.
    3. Delete `app/build/outputs/apk` and `app/build/intermediates/apk`. Debug packaging is
       incremental and otherwise keeps the old 200 MB library's space: the APK grew to 214 MB.
    4. Run `gradlew assembleArm64Debug -x rustBuildArm64Debug` from `gen/android`, then
       `adb install -r app/build/outputs/apk/arm64/debug/app-arm64-debug.apk`.
    - The APK is the 9 MB `.so` plus about 7 MB of unshrunk Kotlin/AndroidX code.
    - Turning on R8 (`isMinifyEnabled`) for debug crashed at startup in Jackson's
      `ObjectMapper.<init>`, for only 4 MB saved. Reverted. The release build has R8 on too, so
      check the release APK for the same crash.
  - Release APK: not done yet. Make a signing key (`keystore.properties` is already
    git-ignored). Switching from the debug key
    needs an uninstall, which deletes the phone's local notes. Sync or export first.
  - Debugging: debug builds expose WebView DevTools. Run `adb forward tcp:9333
    localabstract:webview_devtools_remote_<pid>`, then use chrome://inspect or CDP.

**Then:** the rest of milestone 10:
- GitHub Actions to build the Windows zip/installer, Linux AppImage, and macOS .app on
  release tags.
- iOS is optional (it needs a paid Apple account).

**Later or optional:**
- PLAN.md §15 ideas: tags (#hashtags in the text was the preferred design), a
  quick-capture hotkey, end-to-end encryption, note history.
- Alternative sync backends.
