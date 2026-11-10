//! Performance checks against PLAN.md's targets. Ignored by default; run with
//!   cargo test --release --lib perf -- --ignored --nocapture
//! `FERRITE_SEED_DB=<path>` also writes the 5,000 notes to a database file,
//! for measuring startup with a full notes list.

use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::notes::{self, ListFilter};

const NOTES: usize = 5_000;
const WORDS: &[&str] = &[
    "the", "and", "fix", "bug", "login", "page", "safari", "crash", "release", "meeting",
    "notes", "buy", "milk", "eggs", "invoice", "client", "design", "review", "deploy",
    "server", "cache", "token", "sync", "phone", "android", "window", "button", "color",
    "theme", "search", "query", "index", "report", "budget", "travel", "hotel", "flight",
    "dentist", "birthday", "gift", "recipe", "pasta", "garden", "tomato", "book", "chapter",
    "idea", "startup", "pricing", "customer", "feedback", "slow", "fast", "memory", "disk",
];

/// Small deterministic PRNG (xorshift), so every run uses the same notes.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A plausible note: a title, a few paragraphs and maybe a task list.
/// Sizes vary from a one-liner to a few KB, like real quick notes.
fn note_body(rng: &mut Rng, i: usize) -> String {
    let words = |n: usize, rng: &mut Rng| {
        (0..n).map(|_| WORDS[rng.below(WORDS.len())]).collect::<Vec<_>>().join(" ")
    };
    let mut body = format!("{} {i}\n", words(1 + rng.below(5), rng));
    for _ in 0..rng.below(6) {
        body.push_str(&words(5 + rng.below(60), rng));
        body.push_str("\n\n");
    }
    for _ in 0..rng.below(8) {
        let done = if rng.below(3) == 0 { "x" } else { " " };
        body.push_str(&format!("- [{done}] {}\n", words(2 + rng.below(6), rng)));
    }
    if i.is_multiple_of(500) {
        body.push_str("zanzibar\n"); // a rare word: 10 notes
    }
    body
}

fn seed(conn: &Connection) -> usize {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut bytes = 0;
    conn.execute_batch("BEGIN").unwrap();
    for i in 0..NOTES {
        let note = notes::create(conn).unwrap();
        let body = note_body(&mut rng, i);
        bytes += body.len();
        notes::save(conn, &note.id, &body).unwrap();
        if i.is_multiple_of(50) {
            notes::set_pinned(conn, &note.id, true).unwrap();
        }
    }
    conn.execute_batch("COMMIT").unwrap();
    bytes
}

/// Median and worst of `runs` timings of `f`.
fn time(runs: usize, mut f: impl FnMut()) -> (Duration, Duration) {
    f(); // warm-up: first use prepares the cached statements
    let mut t: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    t.sort();
    (t[runs / 2], t[runs - 1])
}

#[test]
#[ignore = "performance check; run explicitly with --release"]
fn search_5000_notes() {
    let conn = match std::env::var_os("FERRITE_SEED_DB") {
        Some(path) => {
            let _ = std::fs::remove_file(&path);
            crate::db::open(std::path::Path::new(&path)).unwrap()
        }
        None => crate::db::open_in_memory(),
    };
    let start = Instant::now();
    let bytes = seed(&conn);
    println!("seeded {NOTES} notes ({} KB) in {:?}", bytes / 1024, start.elapsed());

    let mut worst = Duration::ZERO;
    for query in ["bug", "zanzibar", "saf", "fix login", "release safari crash", "nothingmatches"] {
        let mut hits = 0;
        let (median, max) = time(50, || hits = notes::search(&conn, query).unwrap().len());
        println!("search {query:<22} {hits:>5} hits  median {median:>10.2?}  worst {max:>10.2?}");
        worst = worst.max(median);
    }
    let (median, max) = time(20, || {
        notes::list(&conn, ListFilter::Notes).unwrap();
    });
    println!("list all notes                     median {median:>10.2?}  worst {max:>10.2?}");

    assert!(worst < Duration::from_millis(20), "a search took {worst:?} (target < 20 ms)");
}
