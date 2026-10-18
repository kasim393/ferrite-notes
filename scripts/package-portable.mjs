// Builds the Windows portable zip: the app, the `portable` marker, and a README.
//   npm run package:portable  →  release/Ferrite-Notes-<version>-portable-windows-<arch>.zip
// Linux (AppImage) and macOS (.app) builds have to run on those systems.

import { execSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

if (process.platform !== 'win32') {
  console.error('This script makes the Windows portable zip; run it on Windows.');
  process.exit(1);
}

const { version } = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const name = `Ferrite-Notes-${version}-portable-windows-${process.arch}`;
const releaseDir = join(root, 'release');
const stage = join(releaseDir, name);
const zip = join(releaseDir, `${name}.zip`);

console.log('▸ Building the release app (no installers)');
execSync('npx tauri build --no-bundle', { cwd: root, stdio: 'inherit' });

console.log(`▸ Staging ${name}`);
rmSync(stage, { recursive: true, force: true });
rmSync(zip, { force: true });
mkdirSync(stage, { recursive: true });
copyFileSync(join(root, 'src-tauri', 'target', 'release', 'ferrite-notes.exe'), join(stage, 'ferrite-notes.exe'));
writeFileSync(join(stage, 'portable'), '');
writeFileSync(
  join(stage, 'README.txt'),
  `Ferrite Notes ${version} (portable)
================================

Unzip this folder anywhere (a USB stick, Documents, Desktop) and run
ferrite-notes.exe. Nothing is installed and nothing is written to the PC:
your notes are saved in the "data" folder next to the app.

The empty file named "portable" is what turns on portable mode. Keep it
next to ferrite-notes.exe.

Needs Microsoft Edge WebView2, which comes with Windows 10 and 11.

Back up your notes
------------------
- Easiest: Settings (gear icon) > Export all notes. This writes a zip of
  plain .md text files next to the app. Copy it anywhere: a drive, Google
  Drive, Dropbox, email.
- Full copy: close the app, then copy or zip the whole "data" folder.
- The app also keeps daily backups of the last 7 days in data\\backups.

Read your notes without the app
-------------------------------
Exported .md files open in any text editor (Notepad, VS Code, Obsidian).
data\\notes.db is a standard SQLite database; DB Browser for SQLite can
open it (the notes are in the "notes" table, "body" column).

Upgrading
---------
Replace ferrite-notes.exe with the new version. Keep the "data" folder.
`.replace(/\n/g, '\r\n'),
);

console.log('▸ Zipping');
// Windows 10+ ships bsdtar, which writes zip files with -a. Call it by full path:
// Git Bash puts GNU tar (no zip support) first on PATH.
const tar = join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
execSync(`"${tar}" -a -c -f "${zip}" -C "${releaseDir}" "${name}"`, { stdio: 'inherit' });
rmSync(stage, { recursive: true, force: true });

console.log(`\n✔ ${zip}`);
