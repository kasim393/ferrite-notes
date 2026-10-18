import { listen } from '@tauri-apps/api/event';
import {
  api,
  errorMessage,
  type ListFilter,
  type Note,
  type NoteSummary,
  type Settings,
  type SyncStatus,
  type Theme,
} from './api';

const SAVE_DELAY_MS = 400;
const LAST_NOTE_KEY = 'ferrite:lastNote';
const PREVIEW_KEY = 'ferrite:previewNotes';

export type SaveStatus = 'saved' | 'unsaved' | 'saving' | 'error';

function remember(id: string | null) {
  try {
    if (id) localStorage.setItem(LAST_NOTE_KEY, id);
    else localStorage.removeItem(LAST_NOTE_KEY);
  } catch {
    // Storage can be unavailable; remembering the last note is only a convenience.
  }
}

function recalled(): string | null {
  try {
    return localStorage.getItem(LAST_NOTE_KEY);
  } catch {
    return null;
  }
}

/** Ids of notes the user left in preview mode. */
function loadPreviewIds(): string[] {
  try {
    const ids: unknown = JSON.parse(localStorage.getItem(PREVIEW_KEY) ?? '[]');
    return Array.isArray(ids) ? ids.filter((id) => typeof id === 'string') : [];
  } catch {
    return [];
  }
}

function savePreviewIds(ids: string[]) {
  try {
    localStorage.setItem(PREVIEW_KEY, JSON.stringify(ids));
  } catch {
    // Only a convenience, like the last-opened note.
  }
}

class Store {
  filter = $state<ListFilter>('notes');
  query = $state('');
  list = $state<NoteSummary[]>([]);
  current = $state<Note | null>(null);
  /** Bumped when the body changes from outside the editor (open, preview toggle). */
  docVersion = $state(0);
  /** Preview mode is remembered per note, so it never carries over to another note. */
  previewIds = $state<string[]>(loadPreviewIds());
  saveStatus = $state<SaveStatus>('saved');
  error = $state<string | null>(null);
  theme = $state<Theme>('system');
  syncStatus = $state<SyncStatus>({ state: 'off', lastSyncedAt: null, message: null });
  settingsOpen = $state(false);

  get preview(): boolean {
    return this.current != null && this.previewIds.includes(this.current.id);
  }

  /** Set by the editor so shortcuts can focus it. */
  focusEditor: () => void = () => {};
  focusSearch: () => void = () => {};

  #pending: { id: string; body: string } | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** Saves run one at a time, in order, so an older body never overwrites a newer one. */
  #saveChain: Promise<void> = Promise.resolve();
  #listRequest = 0;

  async init() {
    try {
      const settings = await api.getSettings();
      this.setTheme(settings.theme, false);
      // Rust pushes sync progress and remote changes as events; no polling here.
      await listen<SyncStatus>('sync-status', (e) => (this.syncStatus = e.payload));
      await listen<string[]>('notes-changed', (e) => void this.onRemoteChanges(e.payload));
      this.syncStatus = await api.getSyncStatus();
    } catch (e) {
      this.fail(e);
    }
    await this.refresh();
    const last = recalled();
    const target = this.list.find((n) => n.id === last) ?? this.list[0];
    if (target) await this.open(target.id);
  }

  /** Notes changed on another device and were just pulled in. */
  async onRemoteChanges(ids: string[]) {
    await this.refresh();
    const note = this.current;
    // Never swap the text out from under unsaved typing; it is merged on the next sync.
    if (!note || !ids.includes(note.id) || this.saveStatus !== 'saved') return;
    try {
      const fresh = await api.getNote(note.id);
      if (fresh.body !== note.body) {
        this.current = fresh;
        this.docVersion++;
      } else {
        note.pinned = fresh.pinned;
        note.deleted = fresh.deleted;
        note.updatedAt = fresh.updatedAt;
      }
    } catch {
      // Purged on another device.
      this.current = null;
    }
  }

  async syncNow() {
    try {
      this.syncStatus = await api.syncNow();
    } catch (e) {
      this.fail(e);
    }
  }

  /** Saves sync settings; returns false (and shows the error) if they were rejected. */
  async saveSync(syncUrl: string, syncToken: string): Promise<boolean> {
    try {
      const current = await api.getSettings();
      const next: Settings = { ...current, syncUrl, syncToken };
      await api.setSettings(next);
      this.syncStatus = await api.getSyncStatus();
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    }
  }

  fail(e: unknown) {
    this.error = errorMessage(e);
  }

  async refresh() {
    const request = ++this.#listRequest;
    try {
      const q = this.query.trim();
      const list =
        q && this.filter === 'notes' ? await api.search(q) : await api.listNotes(this.filter);
      // Drop stale responses when the user types faster than results arrive.
      if (request === this.#listRequest) this.list = list;
    } catch (e) {
      this.fail(e);
    }
  }

  /** Called by the editor on every change; saves are debounced. */
  edit(body: string) {
    const note = this.current;
    if (!note) return;
    note.body = body;
    this.#pending = { id: note.id, body };
    this.saveStatus = 'unsaved';
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.flush(), SAVE_DELAY_MS);
  }

  /** Saves any pending edit now. Safe to call any time. */
  flush(): Promise<void> {
    clearTimeout(this.#timer);
    this.#saveChain = this.#saveChain.then(() => this.#savePending());
    return this.#saveChain;
  }

  async #savePending() {
    const pending = this.#pending;
    if (!pending) return;
    this.#pending = null;
    this.saveStatus = 'saving';
    try {
      const updatedAt = await api.saveNote(pending.id, pending.body);
      if (this.current?.id === pending.id) this.current.updatedAt = updatedAt;
      this.saveStatus = this.#pending ? 'unsaved' : 'saved';
      await this.refresh();
    } catch (e) {
      // Keep the edit so the next flush retries it.
      this.#pending ??= pending;
      this.saveStatus = 'error';
      this.fail(e);
    }
  }

  /** Leave the current note: save it, and drop it if it was never written in. */
  async #leaveCurrent() {
    await this.flush();
    const note = this.current;
    if (note && !note.deleted && note.body.trim() === '') {
      try {
        await api.discardIfEmpty(note.id);
      } catch (e) {
        this.fail(e);
      }
    }
  }

  async open(id: string) {
    if (this.current?.id === id) return;
    await this.#leaveCurrent();
    try {
      this.current = await api.getNote(id);
      this.docVersion++;
      remember(id);
    } catch (e) {
      this.fail(e);
      this.current = null;
    }
    await this.refresh();
  }

  /** Back to the list (used on narrow screens). */
  async close() {
    await this.#leaveCurrent();
    this.current = null;
    await this.refresh();
  }

  async create() {
    await this.#leaveCurrent();
    try {
      if (this.filter !== 'notes') this.filter = 'notes';
      this.query = '';
      this.current = await api.createNote();
      this.docVersion++;
      remember(this.current.id);
      await this.refresh();
      queueMicrotask(() => this.focusEditor());
    } catch (e) {
      this.fail(e);
    }
  }

  async togglePin() {
    const note = this.current;
    if (!note || note.deleted) return;
    try {
      await api.setPinned(note.id, !note.pinned);
      note.pinned = !note.pinned;
      await this.refresh();
    } catch (e) {
      this.fail(e);
    }
  }

  async trash() {
    const note = this.current;
    if (!note || note.deleted) return;
    await this.flush();
    try {
      const index = this.list.findIndex((n) => n.id === note.id);
      await api.trashNote(note.id);
      this.current = null;
      remember(null);
      await this.refresh();
      const next = this.list[Math.min(Math.max(index, 0), this.list.length - 1)];
      if (next) await this.open(next.id);
    } catch (e) {
      this.fail(e);
    }
  }

  async restore() {
    const note = this.current;
    if (!note || !note.deleted) return;
    try {
      await api.restoreNote(note.id);
      this.filter = 'notes';
      this.current = null;
      await this.refresh();
      await this.open(note.id);
    } catch (e) {
      this.fail(e);
    }
  }

  async emptyTrash() {
    try {
      await api.purgeTrash();
      if (this.current?.deleted) this.current = null;
      await this.refresh();
    } catch (e) {
      this.fail(e);
    }
  }

  async setFilter(filter: ListFilter) {
    if (this.filter === filter) return;
    await this.#leaveCurrent();
    this.filter = filter;
    this.current = null;
    await this.refresh();
  }

  async toggleTask(lineNo: number) {
    const note = this.current;
    if (!note) return;
    await this.flush();
    try {
      this.current = await api.toggleTask(note.id, lineNo);
      this.docVersion++;
      await this.refresh();
    } catch (e) {
      this.fail(e);
    }
  }

  async togglePreview() {
    if (!this.current) return;
    await this.flush();
    const id = this.current.id;
    const on = !this.preview;
    this.previewIds = on ? [...this.previewIds, id] : this.previewIds.filter((p) => p !== id);
    savePreviewIds(this.previewIds);
    if (!on) {
      this.docVersion++;
      queueMicrotask(() => this.focusEditor());
    }
  }

  setTheme(theme: Theme, persist = true) {
    this.theme = theme;
    if (theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
    if (persist) {
      api
        .getSettings()
        .then((s) => api.setSettings({ ...s, theme }))
        .catch((e) => this.fail(e));
    }
  }
}

export const store = new Store();
