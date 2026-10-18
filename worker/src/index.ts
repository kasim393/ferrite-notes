/**
 * Ferrite Notes sync server: one endpoint, POST /sync, for a single user.
 *
 * Each request pushes the client's changed notes and pulls every note whose
 * `rev` (server sequence number) is newer than the client's cursor.
 * Conflicts are last-write-wins per note, but the losing version is kept as a
 * new "Conflict copy" note, so nothing is ever lost.
 */

export interface Env {
  DB: D1Database;
  SYNC_TOKEN?: string;
}

/** Live note, in the trash, or purged (a tombstone with its body cleared). */
type Deleted = 0 | 1 | 2;

export interface PushedNote {
  id: string;
  body: string;
  pinned: 0 | 1;
  deleted: Deleted;
  updated_at: number;
  /** The server `rev` this edit was based on (0 if never synced). */
  base_rev: number;
}

export interface ServerNote {
  id: string;
  body: string;
  pinned: 0 | 1;
  deleted: Deleted;
  updated_at: number;
  rev: number;
}

export interface SyncRequest {
  since_rev: number;
  changes: PushedNote[];
}

export interface SyncResponse {
  /** Cursor to send as `since_rev` next time. */
  server_rev: number;
  /** More changes are waiting: call again with `since_rev = server_rev`. */
  more: boolean;
  /** The client's cursor is ahead of this server (e.g. a new database): start over from 0 and re-push everything. */
  reset: boolean;
  changes: ServerNote[];
}

/** D1 allows 100 bound parameters per query; existing notes are looked up in one `IN (...)`. */
export const MAX_CHANGES = 100;
export const MAX_BODY_CHARS = 1_000_000;
export const PAGE_SIZE = 200;
const MAX_ATTEMPTS = 5;

class BadRequest extends Error {}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === '/' && request.method === 'GET') {
      return new Response('Ferrite Notes sync server is running.\n');
    }
    if (url.pathname !== '/sync') return json({ error: 'Not found' }, 404);
    if (request.method !== 'POST') return json({ error: 'Use POST' }, 405);
    if (!env.SYNC_TOKEN) return json({ error: 'SYNC_TOKEN is not set on the server' }, 500);
    if (!(await authorized(request, env.SYNC_TOKEN))) return json({ error: 'Unauthorized' }, 401);

    let input: SyncRequest;
    try {
      input = parseRequest(await request.json());
    } catch (e) {
      return json({ error: e instanceof BadRequest ? e.message : 'Body must be JSON' }, 400);
    }

    try {
      return json(await sync(env.DB, input));
    } catch (e) {
      console.error('sync failed', e);
      return json({ error: 'Sync failed on the server' }, 500);
    }
  },
} satisfies ExportedHandler<Env>;

function json(body: unknown, status = 200): Response {
  return Response.json(body, { status, headers: { 'Cache-Control': 'no-store' } });
}

/** Constant-time token check (hashing first makes the lengths equal). */
async function authorized(request: Request, token: string): Promise<boolean> {
  const header = request.headers.get('Authorization') ?? '';
  const match = /^Bearer (.+)$/.exec(header);
  if (!match) return false;
  const enc = new TextEncoder();
  const [a, b] = await Promise.all([
    crypto.subtle.digest('SHA-256', enc.encode(match[1])),
    crypto.subtle.digest('SHA-256', enc.encode(token)),
  ]);
  return crypto.subtle.timingSafeEqual(a, b);
}

function isInt(v: unknown, min = 0): v is number {
  return typeof v === 'number' && Number.isSafeInteger(v) && v >= min;
}

export function parseRequest(raw: unknown): SyncRequest {
  if (typeof raw !== 'object' || raw === null) throw new BadRequest('Expected an object');
  const { since_rev, changes } = raw as Record<string, unknown>;
  if (!isInt(since_rev)) throw new BadRequest('since_rev must be a non-negative integer');
  if (!Array.isArray(changes)) throw new BadRequest('changes must be an array');
  if (changes.length > MAX_CHANGES) throw new BadRequest(`At most ${MAX_CHANGES} changes per request`);

  const parsed = changes.map((c: unknown, i): PushedNote => {
    const n = (typeof c === 'object' && c !== null ? c : {}) as Record<string, unknown>;
    const bad = (field: string) => new BadRequest(`changes[${i}].${field} is invalid`);
    if (typeof n.id !== 'string' || n.id.length === 0 || n.id.length > 64) throw bad('id');
    if (typeof n.body !== 'string' || n.body.length > MAX_BODY_CHARS) throw bad('body');
    if (n.pinned !== 0 && n.pinned !== 1) throw bad('pinned');
    if (n.deleted !== 0 && n.deleted !== 1 && n.deleted !== 2) throw bad('deleted');
    if (!isInt(n.updated_at)) throw bad('updated_at');
    if (!isInt(n.base_rev)) throw bad('base_rev');
    return {
      id: n.id,
      body: n.body,
      pinned: n.pinned,
      deleted: n.deleted,
      updated_at: n.updated_at,
      base_rev: n.base_rev,
    };
  });
  return { since_rev, changes: parsed };
}

export async function sync(db: D1Database, req: SyncRequest): Promise<SyncResponse> {
  const counter = await currentRev(db);
  if (req.since_rev > counter) {
    return { server_rev: 0, more: false, reset: true, changes: [] };
  }
  for (let attempt = 1; ; attempt++) {
    try {
      await push(db, req.changes);
      break;
    } catch (e) {
      // Another request moved the counter between our read and our write; redo with fresh data.
      if (attempt < MAX_ATTEMPTS && String(e).includes('CHECK constraint failed')) continue;
      throw e;
    }
  }
  return pull(db, req.since_rev);
}

async function currentRev(db: D1Database): Promise<number> {
  return (await db.prepare('SELECT rev FROM counter WHERE id = 1').first<number>('rev')) ?? 0;
}

function titleOf(body: string): string {
  const line = body.split('\n').find((l) => l.trim() !== '') ?? '';
  const title = line.replace(/^#+/, '').trim();
  return title.length > 80 ? `${title.slice(0, 80)}…` : title || 'Untitled';
}

async function push(db: D1Database, changes: PushedNote[]): Promise<void> {
  if (changes.length === 0) return;

  const counter = await currentRev(db);
  const ids = [...new Set(changes.map((c) => c.id))];
  const { results } = await db
    .prepare(
      `SELECT id, body, pinned, deleted, updated_at, rev FROM notes
       WHERE id IN (${ids.map(() => '?').join(', ')})`,
    )
    .bind(...ids)
    .all<ServerNote>();
  const existing = new Map(results.map((n) => [n.id, n]));

  let rev = counter;
  const upsert = db.prepare(
    `INSERT INTO notes (id, body, pinned, deleted, updated_at, rev) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
     ON CONFLICT(id) DO UPDATE SET body = excluded.body, pinned = excluded.pinned,
       deleted = excluded.deleted, updated_at = excluded.updated_at, rev = excluded.rev`,
  );
  const statements: D1PreparedStatement[] = [
    // Optimistic lock: fails the whole batch (CHECK id = 1) if the counter moved since we read it.
    db
      .prepare('INSERT INTO counter (id, rev) SELECT 2, 0 WHERE (SELECT rev FROM counter WHERE id = 1) <> ?1')
      .bind(counter),
  ];

  const write = (n: Omit<ServerNote, 'rev'>) => {
    rev++;
    statements.push(upsert.bind(n.id, n.body, n.pinned, n.deleted, n.updated_at, rev));
    return { ...n, rev };
  };
  /** Keep a version that lost a conflict as its own note. Trashed or empty versions aren't worth keeping. */
  const keepLoser = (loser: Omit<ServerNote, 'rev'>) => {
    if (loser.deleted !== 0 || loser.body.trim() === '') return;
    write({
      id: crypto.randomUUID(),
      body: `Conflict copy – ${titleOf(loser.body)}\n\n${loser.body}`,
      pinned: 0,
      deleted: 0,
      updated_at: Date.now(),
    });
  };

  for (const c of changes) {
    const { base_rev, ...incoming } = c;
    const current = existing.get(c.id);
    if (!current) {
      // Purging a note the server never saw: nothing to record.
      if (incoming.deleted !== 2) existing.set(c.id, write(incoming));
      continue;
    }
    const differs = current.body !== incoming.body;
    if (incoming.updated_at > current.updated_at) {
      // Newer edit wins. If it overwrites a version the client never saw, keep that one too.
      if (current.rev > base_rev && differs) keepLoser(current);
      existing.set(c.id, write(incoming));
    } else {
      // The server's copy is newer. Re-stamp it so the client receives it in this response.
      if (differs) keepLoser(incoming);
      existing.set(c.id, write(current));
    }
  }

  statements.push(db.prepare('UPDATE counter SET rev = ?1 WHERE id = 1').bind(rev));
  await db.batch(statements);
}

async function pull(db: D1Database, since: number): Promise<SyncResponse> {
  const { results } = await db
    .prepare(
      `SELECT id, body, pinned, deleted, updated_at, rev FROM notes
       WHERE rev > ?1 ORDER BY rev LIMIT ?2`,
    )
    .bind(since, PAGE_SIZE + 1)
    .all<ServerNote>();
  const more = results.length > PAGE_SIZE;
  const changes = results.slice(0, PAGE_SIZE);
  return { server_rev: changes.at(-1)?.rev ?? since, more, reset: false, changes };
}
