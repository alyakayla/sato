import type { Case, Category, IndexProgress, Person, TreeNode } from './types';
import { api } from './api';
import { t } from './i18n/index.svelte';
import { withTransition } from './viewTransition';

/**
 * A tiny hand-rolled store. Svelte's built-in `writable` would work, but these
 * values are read by many components and we want explicit invalidation and a
 * single place to add error handling.
 */
export class Store<T> {
  // Raw state: values are always replaced wholesale via `set`, so there is no
  // need for deep proxies — but reading `.value` in a component must be
  // tracked, or views never re-render when the store changes.
  #value = $state.raw() as T;
  #subs = new Set<(v: T) => void>();

  constructor(initial: T) {
    this.#value = initial;
  }

  get value(): T {
    return this.#value;
  }

  set(v: T): void {
    this.#value = v;
    for (const fn of this.#subs) fn(v);
  }

  update(fn: (v: T) => T): void {
    this.set(fn(this.#value));
  }

  subscribe(fn: (v: T) => void): () => void {
    this.#subs.add(fn);
    return () => this.#subs.delete(fn);
  }
}

// ---------------------------------------------------------------------------
// App-wide state
// ---------------------------------------------------------------------------

export type ViewName =
  | 'dashboard'
  | 'cases'
  | 'documents'
  | 'people'
  | 'sheets'
  | 'chat'
  | 'settings';

/**
 * What the right-hand workspace is showing. A single pane keeps navigation
 * unambiguous: opening a document from chat replaces the workspace, and the
 * back stack returns to whatever was there before.
 */
export type PaneView =
  | { kind: 'grid' }
  | { kind: 'case'; caseId: string }
  | { kind: 'document'; documentId: string }
  | { kind: 'sheet'; sheetId: string }
  | { kind: 'people'; caseId: string | null }
  | { kind: 'table'; caseId: string | null }
  | { kind: 'search'; caseId: string | null }
  | { kind: 'calendar'; caseId: string | null }
  | { kind: 'settings' };

export const view = new Store<ViewName>('dashboard');

/** The case a panel should scope to. Null means "all cases". */
export const scopedCaseId = new Store<string | null>(null);

/** The case the chat is currently scoped to. */
export const chatCaseId = new Store<string | null>(null);

export const categories = new Store<Category[]>([]);
export const cases = new Store<Case[]>([]);
export const people = new Store<Person[]>([]);
export const activeCaseId = new Store<string | null>(null);

export const indexing = new Store<Record<string, IndexProgress>>({});
export const toast = new Store<{ kind: 'ok' | 'danger' | 'warn'; text: string } | null>(null);

// --- Workspace -------------------------------------------------------------

export const pane = new Store<PaneView>({ kind: 'grid' });

/** Nodes with children the tree view has collapsed, by node id. */
export const collapsed = new Store<Set<string>>(new Set());

/** The case tree, refreshed whenever anything under it changes. */
export const treeNodes = new Store<TreeNode[]>([]);

/**
 * A mention another view wants dropped into the chat composer — set by the
 * document viewer's "Ask about this" button. The composer consumes and clears
 * it, so the user can add their question before sending.
 */
export const pendingMention = new Store<string | null>(null);

export function askAbout(nodeKey: string): void {
  pendingMention.set(nodeKey);
}

/** Chat panel width in pixels, persisted across restarts. */
const CHAT_W_KEY = 'counsel.chatWidth';
const chatWidth = Number(localStorage.getItem(CHAT_W_KEY));
export const chatPanelWidth = new Store<number>(
  Number.isFinite(chatWidth) && chatWidth >= 300 ? chatWidth : 400,
);

export function setChatPanelWidth(px: number): void {
  const clamped = Math.round(Math.max(300, Math.min(720, px)));
  chatPanelWidth.set(clamped);
  localStorage.setItem(CHAT_W_KEY, String(clamped));
}

/** Whether the assistant column is hidden, persisted across restarts. */
const CHAT_HIDDEN_KEY = 'counsel.chatCollapsed';
export const chatCollapsed = new Store<boolean>(localStorage.getItem(CHAT_HIDDEN_KEY) === '1');

export function toggleChat(): void {
  const next = !chatCollapsed.value;
  localStorage.setItem(CHAT_HIDDEN_KEY, next ? '1' : '0');
  withTransition(() => chatCollapsed.set(next), 'layout');
}

/** Whether quick find (Ctrl+K) is open. Owned here so the rail can open it. */
export const paletteOpen = new Store<boolean>(false);

/**
 * A create flow another surface wants started — set by quick find. QuickAdd
 * owns the dialogs, so it consumes and clears this rather than duplicating them.
 */
export const createRequest = new Store<'case' | 'sheet' | 'import' | null>(null);

/**
 * Bumped whenever an artifact is changed from outside the view that shows it
 * (a context-menu delete, say). Views that keep their own copy of rows —
 * tables, the calendar, rosters — read it in their load effect and refetch.
 */
export const artifactsVersion = new Store<number>(0);

export function artifactsChanged(): void {
  artifactsVersion.set(artifactsVersion.value + 1);
}

/**
 * An editor another surface wants opened — set by an artifact's context menu
 * ("Edit details…"). The view that owns the editor consumes it with
 * `takeEditRequest` once its data has loaded, then opens the editor itself.
 */
export type EditTarget =
  | { kind: 'case'; id: string }
  | { kind: 'sheet'; id: string }
  | { kind: 'person'; id: string }
  | { kind: 'event'; id: string; startsAt: string | null };

export const editRequest = new Store<EditTarget | null>(null);

/** Clears and returns true when the pending request is for this item. */
export function takeEditRequest(kind: EditTarget['kind'], id: string): boolean {
  const req = editRequest.value;
  if (!req || req.kind !== kind || req.id !== id) return false;
  editRequest.set(null);
  return true;
}

/**
 * Back/forward history for the workspace. Kept as a stack plus an index so
 * both toolbar buttons and the keyboard shortcut share one implementation.
 */
const history = new Store<PaneView[]>([{ kind: 'grid' }]);
const historyIndex = new Store<number>(0);

export const canGoBack = new Store<boolean>(false);
export const canGoForward = new Store<boolean>(false);

function syncHistoryFlags(): void {
  const i = historyIndex.value;
  canGoBack.set(i > 0);
  canGoForward.set(i < history.value.length - 1);
}

export const paneHistory = history;
export const paneHistoryIndex = historyIndex;

export function goBack(): void {
  const i = historyIndex.value;
  if (i <= 0) return;
  withTransition(() => {
    historyIndex.set(i - 1);
    pane.set(history.value[i - 1]);
    syncHistoryFlags();
  });
}

export function goForward(): void {
  const i = historyIndex.value;
  if (i >= history.value.length - 1) return;
  withTransition(() => {
    historyIndex.set(i + 1);
    pane.set(history.value[i + 1]);
    syncHistoryFlags();
  });
}

/**
 * Opens a view, pushing it onto the history stack. Navigating from the same
 * place again (a reload of the current pane) does not stack duplicates.
 */
export function openPane(next: PaneView): void {
  const current = pane.value;
  if (JSON.stringify(current) === JSON.stringify(next)) return;
  withTransition(() => {
    const trimmed = history.value.slice(0, historyIndex.value + 1);
    trimmed.push(next);
    // Bound the stack: this is a navigation aid, not an audit log.
    const bounded = trimmed.slice(-50);
    history.set(bounded);
    historyIndex.set(bounded.length - 1);
    pane.set(next);
    syncHistoryFlags();
  });
}

/** Replaces the current entry in place, used when a pane is retargeted. */
export function replacePane(next: PaneView): void {
  const stack = [...history.value];
  stack[historyIndex.value] = next;
  history.set(stack);
  pane.set(next);
}

/** Opens a tree node in the view that suits its kind. */
export function openNode(node: TreeNode): void {
  if (node.kind === 'document' && node.documentId) {
    openPane({ kind: 'document', documentId: node.documentId });
  } else if (node.kind === 'sheet') {
    // Sheet nodes are stored prefixed so ids cannot collide with other kinds.
    openPane({ kind: 'sheet', sheetId: node.id.slice('sheet:'.length) });
  } else if (node.kind === 'case' && node.caseId) {
    openPane({ kind: 'case', caseId: node.caseId });
  } else if (node.kind === 'person' && node.caseId) {
    openPane({ kind: 'people', caseId: node.caseId });
  } else if (node.kind === 'event' && node.caseId) {
    openPane({ kind: 'calendar', caseId: node.caseId });
  }
}

export function toggleCollapsed(id: string): void {
  collapsed.update((set) => {
    const next = new Set(set);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });
}

export function expandTo(nodeId: string, nodes: TreeNode[]): void {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const toExpand: string[] = [];
  let cur = byId.get(nodeId)?.parentId ?? null;
  while (cur) {
    toExpand.push(cur);
    cur = byId.get(cur)?.parentId ?? null;
  }
  if (toExpand.length === 0) return;
  collapsed.update((set) => {
    const next = new Set(set);
    for (const id of toExpand) next.delete(id);
    return next;
  });
}

// ---------------------------------------------------------------------------
// Toasts, errors, and refresh helpers
// ---------------------------------------------------------------------------

let toastTimer: ReturnType<typeof setTimeout> | undefined;

export function notify(kind: 'ok' | 'danger' | 'warn', text: string): void {
  toast.set({ kind, text });
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.set(null), kind === 'danger' ? 6000 : 3200);
}

/** Wraps an async call, surfacing failures as a toast instead of a rejection. */
export async function guard<T>(label: string, fn: () => Promise<T>): Promise<T | null> {
  try {
    return await fn();
  } catch (e) {
    notify('danger', `${label}: ${errorText(e)}`);
    return null;
  }
}

export function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) return String((e as Error).message);
  return String(e);
}

export async function refreshCategories(): Promise<void> {
  const data = await api.listCategories();
  categories.set(data);
}

export async function refreshCases(): Promise<void> {
  const data = await api.listCases();
  cases.set(data);
}

export async function refreshPeople(): Promise<void> {
  const data = await api.listPeople();
  people.set(data);
}

/** Reloads the case tree, which every workspace view navigates from. */
export async function refreshTree(): Promise<void> {
  const data = await guard(t('err.loadTree'), () => api.caseTree());
  if (data) treeNodes.set(data.nodes);
}

/** Reloads everything a structural change could have invalidated. */
export async function refreshAll(): Promise<void> {
  await Promise.all([refreshCases(), refreshCategories(), refreshTree()]);
}

/** Case label used across the header, chat scope, and document filters. */
export function caseLabel(list: Case[], id: string | null): string {
  if (!id) return t('common.allCases');
  const found = list.find((c) => c.id === id);
  return found ? `${found.reference} · ${found.title}` : t('common.allCases');
}

/** Human label for the current workspace pane, used in the header. */
export function paneTitle(p: PaneView, nodes: TreeNode[], caseList: Case[]): string {
  switch (p.kind) {
    case 'grid':
      return t('view.tree');
    case 'settings':
      return t('view.settings');
    case 'case': {
      const c = caseList.find((x) => x.id === p.caseId);
      return c ? c.title : t('kind.case');
    }
    case 'document': {
      const n = nodes.find((x) => x.documentId === p.documentId);
      return n?.label ?? t('kind.document');
    }
    case 'sheet': {
      const n = nodes.find((x) => x.kind === 'sheet' && x.id === `sheet:${p.sheetId}`);
      return n?.label ?? t('view.spreadsheet');
    }
    case 'people':
      return p.caseId ? caseLabel(caseList, p.caseId) : t('view.people');
    case 'search':
      return p.caseId ? t('view.scoped', { case: caseLabel(caseList, p.caseId), view: t('view.search') }) : t('view.search');
    case 'table':
      return p.caseId ? t('view.scoped', { case: caseLabel(caseList, p.caseId), view: t('view.table') }) : t('view.table');
    case 'calendar':
      return p.caseId ? t('view.scoped', { case: caseLabel(caseList, p.caseId), view: t('view.calendar') }) : t('view.calendar');
  }
}
