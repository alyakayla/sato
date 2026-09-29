import { DEFAULT_LAYOUT, normalizeLayout, sameLayout, type Layout, type PanelId } from './layout';
import { Store } from './stores.svelte';
import { withTransition } from './viewTransition';

/**
 * The panel layout and the drag that changes it.
 *
 * Dragging uses pointer events, not HTML5 drag-and-drop: Tauri's file-drop
 * handler intercepts native drag events in the Windows webview, and pointer
 * events also give us one code path for mouse, pen and touch.
 */

const LAYOUT_KEY = 'counsel.layout';

function load(): Layout {
  try {
    return normalizeLayout(JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? 'null'));
  } catch {
    return { ...DEFAULT_LAYOUT };
  }
}

export const panelLayout = new Store<Layout>(load());

export function setLayout(next: Layout): void {
  if (sameLayout(next, panelLayout.value)) return;
  localStorage.setItem(LAYOUT_KEY, JSON.stringify(next));
  // Panels glide to their new places rather than jumping.
  withTransition(() => panelLayout.set(next), 'layout');
}

export function resetLayout(): void {
  setLayout({ ...DEFAULT_LAYOUT });
}

/** Pixels the pointer must travel before a press becomes a drag. */
const THRESHOLD = 4;

interface DragState {
  id: PanelId | null;
  active: boolean;
  x: number;
  y: number;
  /** The layout a drop here would produce, or null when it would change nothing. */
  target: Layout | null;
}

export const drag = $state<DragState>({ id: null, active: false, x: 0, y: 0, target: null });

/** Registered by the shell, which is the only place that can measure. */
let propose: ((id: PanelId, x: number, y: number) => Layout) | null = null;

export function registerDropResolver(fn: (id: PanelId, x: number, y: number) => Layout): () => void {
  propose = fn;
  return () => {
    if (propose === fn) propose = null;
  };
}

/** Starts a potential drag from a grip. A press that never moves is a click. */
export function beginPanelDrag(id: PanelId, e: PointerEvent): void {
  if (e.button !== 0) return;
  const startX = e.clientX;
  const startY = e.clientY;
  drag.id = id;

  const move = (ev: PointerEvent) => {
    if (!drag.active && Math.hypot(ev.clientX - startX, ev.clientY - startY) < THRESHOLD) return;
    drag.active = true;
    drag.x = ev.clientX;
    drag.y = ev.clientY;
    const next = propose?.(id, ev.clientX, ev.clientY) ?? null;
    drag.target = next && !sameLayout(next, panelLayout.value) ? next : null;
  };

  const end = (commit: boolean) => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    window.removeEventListener('keydown', key, true);
    if (commit && drag.active && drag.target) setLayout(drag.target);
    drag.id = null;
    drag.active = false;
    drag.target = null;
  };
  const up = () => end(true);
  // Esc cancels mid-drag; captured so it does not also close a dialog.
  const key = (ev: KeyboardEvent) => {
    if (ev.key === 'Escape' && drag.active) {
      ev.preventDefault();
      ev.stopPropagation();
      end(false);
    }
  };

  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
  window.addEventListener('keydown', key, true);
}
