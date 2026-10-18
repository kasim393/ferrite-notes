import { invoke } from '@tauri-apps/api/core';

export type ListFilter = 'notes' | 'trash';
export type Theme = 'system' | 'light' | 'dark';

export interface Note {
  id: string;
  body: string;
  pinned: boolean;
  deleted: boolean;
  updatedAt: number;
}

export interface NoteSummary {
  id: string;
  title: string;
  /** Plain text; search matches are wrapped in \u0002 … \u0003. */
  snippet: string;
  pinned: boolean;
  deleted: boolean;
  updatedAt: number;
}

export interface Settings {
  theme: Theme;
  syncUrl: string | null;
  syncToken: string | null;
}

export type SyncState = 'off' | 'idle' | 'syncing' | 'offline' | 'error';

export interface SyncStatus {
  state: SyncState;
  lastSyncedAt: number | null;
  message: string | null;
}

export const api = {
  listNotes: (filter: ListFilter) => invoke<NoteSummary[]>('list_notes', { filter }),
  getNote: (id: string) => invoke<Note>('get_note', { id }),
  saveNote: (id: string, body: string) => invoke<number>('save_note', { id, body }),
  createNote: () => invoke<Note>('create_note'),
  discardIfEmpty: (id: string) => invoke<boolean>('discard_if_empty', { id }),
  toggleTask: (id: string, lineNo: number) => invoke<Note>('toggle_task', { id, lineNo }),
  setPinned: (id: string, pinned: boolean) => invoke<void>('set_pinned', { id, pinned }),
  trashNote: (id: string) => invoke<void>('trash_note', { id }),
  restoreNote: (id: string) => invoke<void>('restore_note', { id }),
  purgeTrash: () => invoke<void>('purge_trash'),
  search: (query: string) => invoke<NoteSummary[]>('search', { query }),
  renderMarkdown: (id: string) => invoke<string>('render_markdown', { id }),
  getSettings: () => invoke<Settings>('get_settings'),
  /** Returns the settings as saved (trimmed, URL normalized). */
  setSettings: (settings: Settings) => invoke<Settings>('set_settings', { settings }),
  getSyncStatus: () => invoke<SyncStatus>('get_sync_status'),
  syncNow: () => invoke<SyncStatus>('sync_now'),
};

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
