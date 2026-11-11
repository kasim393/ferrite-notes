# Security

## Reporting a problem

Please **don't open a public issue** for a security problem. Use GitHub's private
reporting instead: **Security → Report a vulnerability** on
[the repository](https://github.com/kasim393/ferrite-notes/security/advisories/new).

Include what you found, how to reproduce it, and what an attacker could do with it. You'll
get a reply within a week.

## What's in scope

- The app: anything that lets a note, a pasted link or a web image run code, read files
  or reach the Rust commands it shouldn't (the Markdown preview is sanitized with ammonia
  and the page has a strict Content Security Policy).
- The sync client and the Worker in `worker/`: anything that leaks or bypasses the sync
  token, or lets one person read or change another's notes.
- The build and packaging scripts.

## How sync is protected

- Each person runs their own Worker; there is no shared server.
- Requests use HTTPS and a random bearer token, set with `wrangler secret put` by
  `npm run setup`. The app stores the token in its local `settings.json` and never logs it.
- Notes are stored unencrypted in your own Cloudflare D1 database. End-to-end encryption
  is a possible future feature, not a current one.

## Supported versions

Only the latest release gets security fixes.
