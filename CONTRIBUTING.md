# Contributing to Ferrite Notes

Thanks for helping. Ferrite Notes aims to stay **fast, small and simple**, so please open
an issue to talk about a new feature before writing a lot of code.

## Get set up

Follow [Build from source](README.md#build-from-source) in the README, then:

| What | Command |
|---|---|
| Run the app with live reload | `npm run tauri dev` |
| Type-check the frontend | `npm run check` |
| Rust tests | `cd src-tauri && cargo test --lib` |
| Rust lint | `cd src-tauri && cargo clippy --all-targets` |
| Search benchmark (5,000 notes) | `cd src-tauri && cargo test --release --lib perf -- --ignored --nocapture` |
| Worker tests | `cd worker && npm test` |

Run Rust commands from `src-tauri/`, so `.cargo/config.toml` (the trimmed SQLite build)
applies.

[CLAUDE.md](CLAUDE.md) is the map of the code: the layout, the conventions and why things
are the way they are. Please skim it first.

## Before you open a pull request

- `npm run check`, `cargo test --lib` and `cargo clippy --all-targets` pass with no
  warnings.
- New pure logic has a test. Database code takes a `&Connection`, so it can be tested
  with `db::open_in_memory()`.
- UI changes are checked in **every theme** (macOS light and dark, Gaming, Terminal, Retro,
  E-ink light and dark) and at a narrow width (under 700 px, the phone layout). A
  screenshot in the pull request helps.
- Walk through the relevant part of [TESTING.md](TESTING.md), and add a line there for new
  behaviour.
- Keep the app small: think twice about new dependencies, and say how much they add.

## Conventions

- **Rust:** commands are `async fn … -> AppResult<T>`, with no `unwrap`/`expect` in command
  paths. Never hold the database lock across an `.await`. A schema change is a new
  `migrations/00N_*.sql`.
- **Frontend:** Svelte 5 runes and plain CSS. Colours, radii and fonts come from the theme
  variables in `src/themes.css`; components use `var(--…)`, never hex values. CodeMirror
  styling goes in `src/lib/editor/setup.ts`.
- **Secrets:** never commit a sync token or your D1 database id. `worker/wrangler.toml`
  keeps the placeholder id in git.
- Write clear commit messages: a short summary line, then what changed and why.

## Reporting bugs

Open an issue with your OS and app version, what you did, what you expected and what
happened. Screenshots help. For security problems, see [SECURITY.md](SECURITY.md) instead.

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
