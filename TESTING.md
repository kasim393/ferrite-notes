# Testing Ferrite Notes

What's built so far: milestones 1–4 (the skeleton, core notes, tasks, search, preview) and
milestones 6–7 (the sync server and the sync client), and from milestone 8 the portable zip, export, and data folder info.

Already checked: `cargo test --lib` (22 Rust tests, including simulated two-device sync and export),
`npm test` in `worker/` (11 Worker tests), `npm run check`, `npm run build`, and clippy.
Not checked yet: a real Cloudflare deploy and syncing between two running apps. The steps for
that are in the Sync section below.

## Run it

```bash
npm install
```

```bash
npm run tauri dev
```

The first run compiles Tauri, which takes a few minutes. Later runs are fast.

## Smoke test checklist

**Startup**
- [ ] The window opens without a white flash. On the first run the list is empty and says "No notes yet".
- [ ] Close the app and reopen it. The window keeps its size and position, and the last note you had open opens again.

**Core notes**
- [ ] `Ctrl+N` creates a note and puts the cursor in the editor. Type `Groceries` and press Enter, then type a few lines.
- [ ] The status goes from "Editing…" to "Saved", and the list shows the title "Groceries" with a snippet.
- [ ] Press `Ctrl+N` twice without typing anything. No empty "Untitled" notes are left behind.
- [ ] Pin a note (`Ctrl+Shift+P` or the Pin button). It moves to the top of the list with a dot.
- [ ] Trash a note with the Trash button. It moves to the Trash tab. Open it there, press Restore, and it comes back.
- [ ] Click "Empty trash" and the trash is cleared.
- [ ] Type in a note, then close the window right away. Reopen the app and the text is still there.

**Tasks (editor)**
- [ ] Type `[]` at the start of a line. It becomes a checkbox (no dash shown). Type `milk` and press Enter: the next line is a new checkbox.
- [ ] `- [ ] milk` typed by hand works the same way.
- [ ] Press Enter on an empty `- [ ] ` line. The prefix is removed.
- [ ] Click a checkbox. It toggles, and checked lines turn grey (no strikethrough). `Ctrl+Z` undoes the toggle.
- [ ] Backspace right after a checkbox removes the whole checkbox in one press.
- [ ] `Ctrl+Enter` toggles the task on the current line. On a plain line it turns the line into a task. On an empty line the cursor ends up after the new checkbox.
- [ ] Select several task lines and press `Ctrl+Enter`: all get checked. Press it again: all get unchecked. Select a mix of checked and unchecked tasks: all get checked.
- [ ] Select several plain-text lines and press `Ctrl+Enter`: each becomes a task (blank lines are skipped).
- [ ] Indented sub-tasks (press Tab before `- [ ]`) also get checkboxes. `1. [ ] numbered` works too.

**Search**
- [ ] `Ctrl+F` focuses search. Type part of a word, like `gro`. Matching notes appear with the match highlighted.
- [ ] Trashed notes don't show up in search. `Esc` clears the search.

**Preview**
- [ ] Paste some markdown (headings, a table, a code block, a link, tasks), then press `Ctrl+P`.
- [ ] It renders. Clicking a checkbox in the preview toggles it, and the change is still there after pressing `Ctrl+P` to go back to the editor.
- [ ] Clicking a link opens your system browser, not the app window.
- [ ] Open a different note: it opens in the editor. Go back to the first note: it is still in preview.
- [ ] Paste `<script>alert(1)</script>` and preview it. Nothing runs.

**Theme and layout**
- [ ] Settings (gear icon in the sidebar footer) → Theme switches between System, Light, and Dark.
- [ ] `Ctrl+B` or the panel icon at the left of the top bar hides and shows the sidebar, including while typing in the editor. The choice is still there after restarting the app. `Ctrl+F` with the sidebar hidden brings it back and focuses search.
- [ ] Shrink the window to under 700px wide. The app shows only the list, then only the note after you open one, with a ‹ back button.

**Portable zip, export and backups (milestone 8)**
- [ ] Run `npm run package:portable`. It writes `release/Ferrite-Notes-0.1.0-portable-windows-x64.zip`.
- [ ] Unzip it somewhere new and run `ferrite-notes.exe`. A separate, empty app opens, and a `data\` folder appears next to it.
- [ ] Rename the `portable` marker to `portable.txt`. The app still opens in portable mode.
- [ ] Settings → "Your notes" shows "Portable mode" and the `data` folder path. "Open data folder" opens Explorer there.
- [ ] Settings → "Export all notes" writes `ferrite-notes-YYYY-MM-DD.zip` and opens Explorer with it selected. In the installed app it goes to Downloads; in portable mode it goes next to the exe.
- [ ] Open the zip: one `.md` file per note, named after its title. Trashed notes are in `Trash\`. Each file opens in Notepad.

## Where data lives
- Installed mode: `%APPDATA%\com.ferritenotes.notes\` (Windows) holds `notes.db`, `settings.json`, and `backups/`.
- A backup is written once a day to `backups/notes-YYYY-MM-DD.db`. The last 7 are kept.

## If something fails
Send me the error from the terminal running `npm run tauri dev`, or the red toast in the app.

## Sync (milestones 6–7)

Restart `npm run tauri dev` first. The Rust side changed, so hot reload isn't enough.

**1. Deploy your server (one time)**
```bash
cd worker
```
```bash
npm install
```
```bash
npx wrangler login
```
```bash
npm run setup
```
At the end it prints a **Sync URL** and a **Sync token**. Keep the token private.

- [ ] Open the Sync URL in a browser. It says "Ferrite Notes sync server is running."

**2. Connect the app**
- [ ] The sidebar footer shows a grey dot and "Sync off". Click it (or the gear icon) to open Settings.
- [ ] Paste the URL and token, then press Save. The footer goes "Syncing…", then shows a green dot and "Synced HH:MM".
- [ ] Type in a note. About 3 seconds after you stop, the footer time updates.

**3. Add a second device on the same PC**
The portable copy has its own data folder, so it acts like a second device.
- [ ] Run `npm run tauri build`. Copy `src-tauri/target/release/ferrite-notes.exe` into an empty folder and create an empty file named `portable` next to it.
- [ ] Run it while the dev app is still open. Both windows open.
- [ ] In the portable app, open Settings and paste the same URL and token. Your notes appear.
- [ ] Edit a note in one app, then click into the other window. Focusing a window syncs it, and the change shows up.
- [ ] Trash a note in one app. It moves to the Trash in the other. Empty the trash: it disappears from both.

**4. Conflicts (nothing should be lost)**
- [ ] Turn off Wi-Fi or unplug the network. Both footers soon show a yellow dot and "Offline".
- [ ] Edit the **same** note differently in each app.
- [ ] Turn the network back on and click into each window.
- [ ] Both apps end up with the newer edit, plus a note titled **"Conflict copy – …"** holding the other edit.

**5. Errors**
- [ ] In Settings, change one character of the token and Save. The footer shows a red dot and "Sync error"; hover over it to see "The server rejected the sync token". Paste the right token back.
- [ ] Press "Turn off sync" in Settings. The footer goes back to "Sync off" and the app keeps working offline.


## Android (milestone 9)

**Build and install on a USB-connected phone**

Windows only lets programs create symlinks in Developer Mode, and `tauri android build`
symlinks the Rust library into the Android project. Turn it on once: Settings → System →
For developers → Developer Mode. Then:

```powershell
npx tauri android dev            # live reload on the phone (phone and PC on the same Wi-Fi)
npx tauri android build --debug --apk --target aarch64
adb install -r src-tauri\gen\android\app\build\outputs\apk\arm64\debug\app-arm64-debug.apk
```

A plain debug APK is about 200 MB (unoptimized Rust with debug info). For daily use,
put the release Rust library inside the debug APK, which comes to about 17 MB. The steps
are in CLAUDE.md under "Daily-use APK".

**Layout and navigation**
- [ ] Nothing is hidden behind the status bar at the top or the gesture bar at the bottom.
- [ ] The list shows first. Tap a note: the note fills the screen with a ‹ button.
- [ ] The back gesture (or ‹) returns to the list. Another back gesture leaves the app.
- [ ] Open Settings (gear). The back gesture closes Settings, not the app.
- [ ] Buttons (toolbar, +, Notes/Trash, gear, sync) are easy to hit with a thumb.
- [ ] Type in a long note: the keyboard never covers the line you are typing on, and the
      toolbar stays visible.
- [ ] Rotate the phone: the layout still fits.

**Notes and saving**
- [ ] Create, edit, pin, trash, restore. Tick checkboxes in the editor and in preview.
- [ ] Type something, then immediately swipe home. Reopen the app: the text is still there.
- [ ] Swipe the app away from recents, reopen: the text is still there.

**Sync**
- [ ] Settings → paste the same sync URL and token as on the PC → Save. Status turns
      "Synced", and the PC's notes appear.
- [ ] Edit a note on the phone, switch to the PC: the change arrives within a few seconds.
- [ ] Edit on the PC, then reopen the phone app from recents: it syncs on resume and the
      change shows.

**Export**
- [ ] Settings → Export all notes opens Android's "Save as" picker with
      `ferrite-notes-YYYY-MM-DD.zip`. Save to Downloads; it says "Saved …". The zip opens
      in Files and contains the `.md` notes.
- [ ] Cancelling the picker shows no error. "Open data folder" isn't shown on the phone.
