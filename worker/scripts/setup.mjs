// One-command setup for the Ferrite Notes sync server:
// creates the D1 database, applies the schema, deploys the Worker, sets a fresh
// random sync token, and prints the URL + token to paste into the app.
//
// Prerequisite: a (free) Cloudflare account and `npx wrangler login`.

import { execSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

const DB_NAME = 'ferrite-notes';
const PLACEHOLDER_ID = '00000000-0000-0000-0000-000000000000';
const CONFIG = new URL('../wrangler.toml', import.meta.url);

/** Runs a command, shows its errors live, and returns its output. */
function run(command, options = {}) {
  return execSync(command, { encoding: 'utf8', stdio: ['inherit', 'pipe', 'inherit'], ...options });
}

function step(message) {
  console.log(`\n▸ ${message}`);
}

function fail(message) {
  console.error(`\n✖ ${message}`);
  process.exit(1);
}

/** Runs a step's commands; on failure, shows what wrangler printed instead of a stack trace. */
function attempt(what, fn) {
  try {
    return fn();
  } catch (e) {
    if (e?.stdout) console.error(e.stdout);
    fail(`${what} failed. See the wrangler output above, fix it, then run \`npm run setup\` again.`);
  }
}

/**
 * Deploys and returns wrangler's output. A brand-new Cloudflare account has no
 * workers.dev subdomain yet; wrangler can only ask for one interactively, so in
 * that case it runs once with the terminal attached, then again to read the URL.
 */
function deploy() {
  try {
    return run('npx wrangler deploy');
  } catch (e) {
    if (!/workers\.dev subdomain/i.test(String(e?.stdout ?? ''))) throw e;
    console.log(`
  Your Cloudflare account needs a workers.dev subdomain (one time only).
  Wrangler will now ask you to register one: answer "yes", then pick a name.
  Your sync URL will be https://ferrite-notes-sync.<that-name>.workers.dev
`);
    run('npx wrangler deploy', { stdio: 'inherit' });
    return run('npx wrangler deploy');
  }
}

step('Checking Cloudflare login');
let whoami = '';
try {
  whoami = run('npx wrangler whoami');
} catch {
  // Reported below.
}
if (!/associated with the email|You are logged in/i.test(whoami)) {
  fail('Not logged in to Cloudflare. Run `npx wrangler login`, then run `npm run setup` again.');
}

step(`Creating the "${DB_NAME}" D1 database`);
let config = readFileSync(CONFIG, 'utf8');
if (config.includes(PLACEHOLDER_ID)) {
  let id = null;
  try {
    const out = run(`npx wrangler d1 create ${DB_NAME}`);
    id = /"?database_id"?\s*[=:]\s*"([0-9a-f-]{36})"/i.exec(out)?.[1] ?? null;
  } catch {
    // Probably exists already; look it up below.
  }
  if (!id) {
    const list = JSON.parse(run('npx wrangler d1 list --json'));
    id = list.find((db) => db.name === DB_NAME)?.uuid ?? null;
  }
  if (!id) fail('Could not create or find the D1 database. See the wrangler output above.');
  config = config.replace(PLACEHOLDER_ID, id);
  writeFileSync(CONFIG, config);
  console.log(`  database_id ${id} saved to wrangler.toml`);
} else {
  console.log('  Already set in wrangler.toml');
}

step('Applying the database schema');
attempt('Applying the schema', () =>
  run(`npx wrangler d1 migrations apply ${DB_NAME} --remote`, { stdio: 'inherit' }),
);

step('Deploying the Worker');
const deployed = attempt('Deploying', deploy);
const url = /https:\/\/[\w.-]+\.workers\.dev/.exec(deployed)?.[0];
if (!url) {
  console.log(deployed);
  fail('Deployed, but could not find the Worker URL in the output above.');
}

step('Setting a new sync token');
const token = randomBytes(32).toString('base64url');
attempt('Setting the token', () =>
  run('npx wrangler secret put SYNC_TOKEN', { input: token, stdio: ['pipe', 'pipe', 'inherit'] }),
);

console.log(`
✔ Sync server is ready.

  Sync URL:   ${url}
  Sync token: ${token}

In Ferrite Notes, open Settings (gear icon) and paste both on every device.
Keep the token private: anyone with it can read and change your notes.
Running setup again creates a NEW token; update your devices after.
`);
