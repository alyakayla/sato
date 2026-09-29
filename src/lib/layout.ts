/**
 * Panel layout: where the navigation rail docks, and which side of the
 * workspace the assistant sits on.
 *
 * Two independent choices rather than one column order, because the rail can
 * dock to any edge (a column on the sides, a bar on the top or bottom) while
 * the assistant and workspace always share a row.
 *
 * Pure functions only, so the drag logic can be tested without a DOM. The
 * shell owns the pointer handling and measures; this module decides what a
 * drop means.
 */

export type PanelId = 'rail' | 'chat' | 'workspace';
export type RailDock = 'left' | 'right' | 'top' | 'bottom';
export type Side = 'left' | 'right';

export interface Layout {
  rail: RailDock;
  chat: Side;
}

export const RAIL_DOCKS: RailDock[] = ['left', 'right', 'top', 'bottom'];
export const DEFAULT_LAYOUT: Layout = { rail: 'left', chat: 'left' };

/**
 * Validates a persisted layout. Anything malformed falls back field by field
 * to the default, so a bad value can never leave a panel unreachable.
 */
export function normalizeLayout(value: unknown): Layout {
  const v = (value && typeof value === 'object' ? value : {}) as Partial<Record<keyof Layout, unknown>>;
  return {
    rail: RAIL_DOCKS.includes(v.rail as RailDock) ? (v.rail as RailDock) : DEFAULT_LAYOUT.rail,
    chat: v.chat === 'left' || v.chat === 'right' ? v.chat : DEFAULT_LAYOUT.chat,
  };
}

export function sameLayout(a: Layout, b: Layout): boolean {
  return a.rail === b.rail && a.chat === b.chat;
}

export function isDefaultLayout(l: Layout): boolean {
  return sameLayout(l, DEFAULT_LAYOUT);
}

interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * The window edge nearest the pointer — where a dragged rail docks. Distances
 * are relative to each axis's size, so on a wide window the top and bottom
 * edges are as easy to hit as the sides.
 */
export function nearestEdge(box: Box, x: number, y: number): RailDock {
  const fx = (x - box.left) / Math.max(1, box.width);
  const fy = (y - box.top) / Math.max(1, box.height);
  const d: [RailDock, number][] = [
    ['left', fx],
    ['right', 1 - fx],
    ['top', fy],
    ['bottom', 1 - fy],
  ];
  return d.reduce((best, cur) => (cur[1] < best[1] ? cur : best))[0];
}

/** Which half of a region the pointer is over. */
export function sideOf(box: Pick<Box, 'left' | 'width'>, x: number): Side {
  return x < box.left + box.width / 2 ? 'left' : 'right';
}

export function opposite(side: Side): Side {
  return side === 'left' ? 'right' : 'left';
}

/**
 * The layout a drop would produce. The rail docks to the nearest window edge;
 * the assistant or workspace goes to whichever half of the content area the
 * pointer is over (dropping the workspace on the left puts the assistant on
 * the right).
 */
export function proposeLayout(
  current: Layout,
  id: PanelId,
  shell: Box,
  content: Pick<Box, 'left' | 'width'>,
  x: number,
  y: number,
): Layout {
  if (id === 'rail') return { ...current, rail: nearestEdge(shell, x, y) };
  const side = sideOf(content, x);
  return { ...current, chat: id === 'chat' ? side : opposite(side) };
}

/** Keyboard moves. Arrows dock the rail directly; ← / → place the others. */
export function keyLayout(current: Layout, id: PanelId, key: string): Layout | null {
  const dir = ({ ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'top', ArrowDown: 'bottom' } as const)[
    key as 'ArrowLeft'
  ];
  if (!dir) return null;
  if (id === 'rail') return dir === current.rail ? null : { ...current, rail: dir };
  if (dir !== 'left' && dir !== 'right') return null;
  const chat = id === 'chat' ? dir : opposite(dir);
  return chat === current.chat ? null : { ...current, chat };
}
