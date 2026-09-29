import { flushSync } from 'svelte';

/**
 * Applies a UI change as a view transition: the browser snapshots the page,
 * the change renders once, and the old and new states are animated between
 * as images — so a transition costs a render, not a frame-by-frame layout.
 *
 * Named regions (see "View transitions" in app.css) animate on their own:
 * the workspace stage cross-fades between views, and the rail, assistant and
 * workspace glide to new positions when the layout changes.
 *
 * Falls back to an instant change where the API is missing or the user
 * prefers reduced motion. Note the update runs a frame later than a direct
 * call, once the old state has been captured.
 */
/**
 * How long the browser may take to prepare a view switch's animation (capture
 * the old state, render the new one) before switches go instant. Only very
 * heavy pages reach it; there, an instant switch feels faster than a smooth
 * one that starts late.
 */
const VIEW_BUDGET_MS = 120;
let viewTransitionsTooSlow = false;

export function withTransition(update: () => void, kind: 'view' | 'layout' = 'view'): void {
  const doc = document as Document & {
    startViewTransition?: (cb: () => void) => { finished: Promise<void>; ready: Promise<void> };
  };
  if (
    !doc.startViewTransition ||
    matchMedia('(prefers-reduced-motion: reduce)').matches ||
    (kind === 'view' && viewTransitionsTooSlow)
  ) {
    update();
    return;
  }
  const started = performance.now();
  // Snapshots cost in proportion to what is captured. A view switch captures
  // only the stage; the panels are named (and captured) only while the layout
  // itself changes. `data-vt` switches those names on for this transition.
  const root = document.documentElement;
  root.dataset.vt = kind;
  const vt = doc.startViewTransition(() => {
    update();
    // Svelte batches DOM updates to a microtask; the browser takes the new
    // snapshot when this callback returns, so render now.
    flushSync();
  });
  if (kind === 'view') {
    void vt.ready.then(
      () => {
        if (performance.now() - started > VIEW_BUDGET_MS) viewTransitionsTooSlow = true;
      },
      () => {},
    );
  }
  void vt.finished.finally(() => {
    if (root.dataset.vt === kind) delete root.dataset.vt;
  });
}
