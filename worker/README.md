# Ferrite Notes sync server

A tiny Cloudflare Worker with a D1 database. Each person runs their own copy on a free
Cloudflare account. It holds one person's notes and is protected by a secret token.

## Set it up (about 5 minutes)

You need Node.js and a free Cloudflare account (https://dash.cloudflare.com/sign-up).

```bash
cd worker
npm install
npx wrangler login
npm run setup
```

`npx wrangler login` opens your browser so you can allow access. `npm run setup` does the rest:

1. Creates the `ferrite-notes` D1 database and saves its id in `wrangler.toml`.
2. Creates the tables.
3. Deploys the Worker.
4. Sets a new random sync token.
5. Prints the **Sync URL** and **Sync token**.

On a brand-new Cloudflare account, the deploy step first asks you to register a
`workers.dev` subdomain. Answer "yes" and pick any free name. You only do this once per account.

In Ferrite Notes, open **Settings** (the gear icon at the bottom of the sidebar), paste both
values, and press **Save**. Do this on every device.

Running `npm run setup` again is safe. It reuses the database and keeps your notes, but it
makes a **new** token, so you'll need to paste the new token on every device.

## Updating the server later

```bash
npm run deploy
```

This applies any new database migrations, then redeploys the Worker.

## Development

```bash
npm test
```

```bash
npm run typecheck
```

```bash
npm run dev
```

`npm test` runs the Worker tests locally in the Workers runtime with a throwaway D1.
`npm run dev` serves the Worker at `http://localhost:8787`. For local use, put
`SYNC_TOKEN=some-token` in a `.dev.vars` file next to this README.

## How it works

There is one endpoint, `POST /sync`, with `Authorization: Bearer <token>`.

- The client sends the notes it changed and its cursor (`since_rev`). The server sends back
  every note with a newer `rev` (a server-side counter, so device clocks never matter).
- If two devices edit the same note, the newer edit wins. The other version is kept as a new
  note titled **"Conflict copy – …"**, so nothing is lost.
- Deleted notes stay as tombstones. Emptying the trash clears a note's text on the server too.

Personal notes are a few MB at most, which Cloudflare's free Workers and D1 tiers should
cover. Check Cloudflare's pricing page for the current limits.
