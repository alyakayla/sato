import { clearPop, popArtifact, unpopArtifact } from './pop';
import { notify } from './stores.svelte';
import { t } from './i18n/index.svelte';

/**
 * Deletes that can be rolled back.
 *
 * Nothing is destroyed at first: the item pops and is hidden everywhere it
 * appears, and a toast offers "→ Rollback". The real delete runs only when
 * the toast's time is up. Rolling back cancels it and brings the item back.
 * No backend restore exists or is needed — there is nothing to restore, since
 * nothing was deleted yet.
 *
 * One delete is pending at a time: starting another confirms the previous one
 * immediately. Ctrl+Z rolls back the pending delete.
 *
 * Hiding works on the `data-artifact` keys every view already renders (see
 * pop.ts), through one generated stylesheet — so every list, tree and chip
 * showing the item hides it at once, with no per-view code.
 */

/** How long the rollback is on offer. */
export const HOLD_MS = 7000;

interface Pending {
  key: string;
  commit: () => Promise<boolean>;
  after?: () => Promise<void> | void;
  timer: ReturnType<typeof setTimeout>;
}

let pending: Pending | null = null;
const hidden = new Set<string>();
let sheet: HTMLStyleElement | null = null;

function escapeAttr(s: string): string {
  return s.replace(/["\\]/g, '\\$&');
}

function render(): void {
  sheet ??= Object.assign(document.createElement('style'), { id: 'sato-pending-deletes' });
  if (!sheet.isConnected) document.head.appendChild(sheet);
  sheet.textContent = [...hidden]
    .map((k) => `[data-artifact="${escapeAttr(k)}"],[data-artifact^="${escapeAttr(k)}:"]{display:none!important}`)
    .join('\n');
}

function hide(key: string): void {
  hidden.add(key);
  render();
}

function show(key: string): void {
  hidden.delete(key);
  render();
}

/** Runs the held delete now. Resolves once it has committed (or failed). */
async function finish(p: Pending): Promise<void> {
  if (pending !== p) return;
  pending = null;
  clearTimeout(p.timer);
  const ok = await p.commit();
  if (ok) await p.after?.();
  // Deleted: the refresh has removed it, so un-hiding reveals nothing.
  // Failed: it comes back (the failure itself was already reported).
  show(p.key);
  if (ok) clearPop(p.key);
  else unpopArtifact(p.key);
}

/** Confirms the pending delete immediately, if there is one. */
export async function flushPendingDelete(): Promise<void> {
  if (pending) await finish(pending);
}

/** Cancels the pending delete and brings the item back. */
export function rollbackPendingDelete(): void {
  const p = pending;
  if (!p) return;
  pending = null;
  clearTimeout(p.timer);
  show(p.key);
  unpopArtifact(p.key);
  notify('ok', t('undo.restored'));
}

export interface UndoableDelete {
  /** `data-artifact` key of what is being deleted. */
  key: string;
  /** Toast text, e.g. "Document deleted". */
  message: string;
  /** Performs the delete; true when it succeeded. */
  commit: () => Promise<boolean>;
  /** After a successful delete: refresh whatever showed the item. */
  after?: () => Promise<void> | void;
  /** Right away, once it is hidden — e.g. leave the item's own page. */
  before?: () => void;
}

export async function deleteWithUndo(d: UndoableDelete): Promise<void> {
  await flushPendingDelete();
  await popArtifact(d.key);
  hide(d.key);
  d.before?.();
  const p: Pending = { key: d.key, commit: d.commit, after: d.after, timer: 0 as unknown as ReturnType<typeof setTimeout> };
  p.timer = setTimeout(() => void finish(p), HOLD_MS);
  pending = p;
  notify('ok', d.message, {
    action: { label: t('undo.rollback'), run: rollbackPendingDelete },
    ms: HOLD_MS,
  });
}

if (typeof window !== 'undefined') {
  // Ctrl+Z rolls back — unless the user is editing text, where it undoes typing.
  window.addEventListener('keydown', (e) => {
    if (!pending || !(e.ctrlKey || e.metaKey) || e.shiftKey || e.key.toLowerCase() !== 'z') return;
    const el = document.activeElement as HTMLElement | null;
    if (el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))) return;
    e.preventDefault();
    rollbackPendingDelete();
  });
  // Closing the window inside the rollback window: try to finish the delete.
  window.addEventListener('pagehide', () => {
    if (pending) void pending.commit();
  });
}
