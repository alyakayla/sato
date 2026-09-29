/**
 * Delete confirmation, as an in-app dialog rather than the browser's native
 * `confirm()` — which blocks the page, cannot be styled or translated, and
 * offers no way to stop asking.
 *
 * `confirmDelete` resolves true to go ahead. Once the user ticks "Don't ask
 * me again" and confirms, later deletes skip the dialog; Settings can turn
 * the question back on.
 */

const SKIP_KEY = 'counsel.skipDeleteConfirm';

function readSkip(): boolean {
  try {
    return localStorage.getItem(SKIP_KEY) === '1';
  } catch {
    return false;
  }
}

let skip = $state(readSkip());

export function askBeforeDeleting(): boolean {
  return !skip;
}

export function setAskBeforeDeleting(ask: boolean): void {
  skip = !ask;
  try {
    localStorage.setItem(SKIP_KEY, skip ? '1' : '0');
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

export interface DeleteRequest {
  /** Dialog heading, e.g. "Delete this document?". */
  title: string;
  /** What will happen, naming the item. */
  message: string;
}

interface DialogState extends DeleteRequest {
  open: boolean;
  dontAskAgain: boolean;
}

export const confirmDialog = $state<DialogState>({
  open: false,
  title: '',
  message: '',
  dontAskAgain: false,
});

let settle: ((ok: boolean) => void) | null = null;

export function confirmDelete(req: DeleteRequest): Promise<boolean> {
  if (skip) return Promise.resolve(true);
  // A second request while one is open cancels the first.
  settle?.(false);
  confirmDialog.title = req.title;
  confirmDialog.message = req.message;
  confirmDialog.dontAskAgain = false;
  confirmDialog.open = true;
  return new Promise((resolve) => {
    settle = resolve;
  });
}

/** Called by the dialog. The "don't ask" choice only sticks on a confirm. */
export function resolveDelete(ok: boolean): void {
  if (ok && confirmDialog.dontAskAgain) setAskBeforeDeleting(false);
  confirmDialog.open = false;
  const done = settle;
  settle = null;
  done?.(ok);
}
