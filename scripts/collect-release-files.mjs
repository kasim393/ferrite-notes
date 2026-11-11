// CI helper for .github/workflows/release.yml: copies one platform's release files into
// out/ (spaces become dots, matching the names GitHub gives uploaded assets) and writes
// SHA256SUMS-<platform>.txt for them.
//
//   ARTIFACTS='["path", ...]' node scripts/collect-release-files.mjs <platform>
//
// ARTIFACTS is tauri-action's `artifactPaths` output. Directories (the macOS .app) are
// skipped; their archived forms (.dmg) are files. The Windows portable zip in release/
// is added when present.

import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

const platform = process.argv[2] ?? process.platform;
const paths = JSON.parse(process.env.ARTIFACTS || '[]');
if (existsSync('release')) {
  for (const f of readdirSync('release')) if (f.endsWith('.zip')) paths.push(join('release', f));
}

mkdirSync('out', { recursive: true });
const sums = [];
for (const p of paths) {
  if (!existsSync(p) || !statSync(p).isFile()) continue;
  const name = basename(p).replaceAll(' ', '.');
  copyFileSync(p, join('out', name));
  const hash = createHash('sha256').update(readFileSync(p)).digest('hex');
  sums.push(`${hash}  ${name}`);
  console.log(`${hash}  ${name}`);
}
if (sums.length === 0) {
  console.error('No release files found.');
  process.exit(1);
}
writeFileSync(`SHA256SUMS-${platform}.txt`, sums.join('\n') + '\n');
