# Testing milestones 1–4

What's built so far: the skeleton, core notes, tasks, search, and preview (PLAN.md §14, milestones 1–4).
Sync (milestones 6–7) is not started yet. That follows the plan's order: milestone 5 is a week of
daily use first.

Already checked: `cargo test --lib` (12 Rust unit tests pass), `npm run check` (0 errors),
`npm run build`.

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
- [ ] "Theme: …" in the sidebar footer cycles through system, light, and dark.
- [ ] Shrink the window to under 700px wide. The app shows only the list, then only the note after you open one, with a ‹ back button.

**Portable mode (optional)**
- [ ] Run `npm run tauri build`. Copy the exe from `src-tauri/target/release/` into an empty folder and create an empty file named `portable` next to it.
- [ ] Run the exe. A `data/` folder with `notes.db`, `backups/`, and `webview/` appears next to it.

## Where data lives
- Installed mode: `%APPDATA%\com.ferritenotes.notes\` (Windows) holds `notes.db`, `settings.json`, and `backups/`.
- A backup is written once a day to `backups/notes-YYYY-MM-DD.db`. The last 7 are kept.

## If something fails
Send me the error from the terminal running `npm run tauri dev`, or the red toast in the app.
