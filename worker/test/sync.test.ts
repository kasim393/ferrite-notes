import { env } from 'cloudflare:workers';
import { beforeEach, describe, expect, it } from 'vitest';
import worker, { PAGE_SIZE, type PushedNote, type SyncResponse } from '../src/index';

const TOKEN = 'test-token';

// Every test starts from an empty server.
beforeEach(async () => {
  await env.DB.batch([
    env.DB.prepare('DELETE FROM notes'),
    env.DB.prepare('UPDATE counter SET rev = 0 WHERE id = 1'),
  ]);
});

function call(body: unknown, token: string | null = TOKEN, method = 'POST', path = '/sync') {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = `Bearer ${token}`;
  const request = new Request(`https://sync.example${path}`, {
    method,
    headers,
    body: method === 'POST' ? JSON.stringify(body) : undefined,
  });
  return worker.fetch(request, env);
}

async function sync(since_rev: number, changes: PushedNote[] = []): Promise<SyncResponse> {
  const res = await call({ since_rev, changes });
  expect(res.status).toBe(200);
  return res.json();
}

function note(id: string, body: string, updated_at: number, base_rev = 0, extra: Partial<PushedNote> = {}): PushedNote {
  return { id, body, pinned: 0, deleted: 0, updated_at, base_rev, ...extra };
}

describe('auth and validation', () => {
  it('says hello on GET /', async () => {
    const res = await call(null, null, 'GET', '/');
    expect(res.status).toBe(200);
  });

  it('rejects missing or wrong tokens', async () => {
    expect((await call({ since_rev: 0, changes: [] }, null)).status).toBe(401);
    expect((await call({ since_rev: 0, changes: [] }, 'wrong')).status).toBe(401);
  });

  it('rejects malformed requests', async () => {
    expect((await call({ since_rev: -1, changes: [] })).status).toBe(400);
    expect((await call({ since_rev: 0, changes: [{ id: 'a' }] })).status).toBe(400);
    const tooMany = Array.from({ length: 101 }, (_, i) => note(`n${i}`, 'x', 1));
    expect((await call({ since_rev: 0, changes: tooMany })).status).toBe(400);
  });
});

describe('sync', () => {
  it('round-trips notes between two devices', async () => {
    const a = await sync(0, [note('n1', 'Groceries\n- [ ] milk', 1000)]);
    expect(a.changes.map((n) => n.id)).toEqual(['n1']);
    expect(a.reset).toBe(false);

    const b = await sync(0);
    expect(b.changes).toHaveLength(1);
    expect(b.changes[0]).toMatchObject({ id: 'n1', body: 'Groceries\n- [ ] milk', rev: a.server_rev });

    // Nothing new since the cursor.
    expect((await sync(b.server_rev)).changes).toEqual([]);
  });

  it('is idempotent when a push is retried after a lost response', async () => {
    const change = note('n1', 'hello', 1000);
    await sync(0, [change]);
    const retry = await sync(0, [change]);
    expect(retry.changes).toHaveLength(1);
    expect(retry.changes[0].body).toBe('hello');
  });

  it('keeps the newer edit and saves the other as a conflict copy (server newer)', async () => {
    const first = await sync(0, [note('n1', 'Plan\nv1', 1000)]);
    const base = first.server_rev;
    // Device A edits and syncs first.
    await sync(base, [note('n1', 'Plan\nfrom A', 3000, base)]);
    // Device B edited earlier, offline, from the same base.
    const b = await sync(base, [note('n1', 'Plan\nfrom B', 2000, base)]);

    const main = b.changes.find((n) => n.id === 'n1');
    expect(main?.body).toBe('Plan\nfrom A');
    const copy = b.changes.find((n) => n.id !== 'n1');
    expect(copy?.body).toBe('Conflict copy – Plan\n\nPlan\nfrom B');
  });

  it('keeps the overwritten version when a newer edit never saw it', async () => {
    const first = await sync(0, [note('n1', 'Plan\nv1', 1000)]);
    const base = first.server_rev;
    await sync(base, [note('n1', 'Plan\nfrom A', 2000, base)]);
    const b = await sync(base, [note('n1', 'Plan\nfrom B', 3000, base)]);

    expect(b.changes.find((n) => n.id === 'n1')?.body).toBe('Plan\nfrom B');
    expect(b.changes.some((n) => n.body === 'Conflict copy – Plan\n\nPlan\nfrom A')).toBe(true);
  });

  it('makes no conflict copy when the client had seen the latest version', async () => {
    const first = await sync(0, [note('n1', 'v1', 1000)]);
    const second = await sync(first.server_rev, [note('n1', 'v2', 2000, first.server_rev)]);
    const third = await sync(second.server_rev, [note('n1', 'v3', 3000, second.server_rev)]);
    expect(third.changes).toHaveLength(1);
    expect(third.changes[0].body).toBe('v3');
  });

  it('records purges as tombstones and ignores purges of unknown notes', async () => {
    const first = await sync(0, [note('n1', 'secret', 1000)]);
    const purged = await sync(first.server_rev, [
      note('n1', '', 2000, first.server_rev, { deleted: 2 }),
      note('never-synced', '', 2000, 0, { deleted: 2 }),
    ]);
    expect(purged.changes).toEqual([expect.objectContaining({ id: 'n1', body: '', deleted: 2 })]);
  });

  it('pages large pulls', async () => {
    for (let i = 0; i < 3; i++) {
      const batch = Array.from({ length: 100 }, (_, j) => note(`n${i}-${j}`, 'x', 1000));
      await sync(0, batch);
    }
    const page1 = await sync(0);
    expect(page1.changes).toHaveLength(PAGE_SIZE);
    expect(page1.more).toBe(true);
    const page2 = await sync(page1.server_rev);
    expect(page2.changes).toHaveLength(300 - PAGE_SIZE);
    expect(page2.more).toBe(false);
  });

  it('asks the client to start over when its cursor is ahead of the server', async () => {
    const res = await sync(999);
    expect(res).toMatchObject({ reset: true, server_rev: 0, changes: [] });
  });
});
