import type { TreeNode, TreeNodeKind } from './types';

/**
 * Layout for a case's connection graph: the case on the left, one group per
 * kind of thing filed under it (documents, people, sheets, events), and each
 * item to the right of its group — a tidy tree read left to right.
 *
 * Pure, so it can be tested; the component only pans, zooms and draws.
 */

export type GraphNodeKind = 'case' | 'group' | TreeNodeKind;

export interface GraphNode {
  id: string;
  kind: GraphNodeKind;
  /** The kind of item under a group node. */
  groupOf?: TreeNodeKind;
  label: string;
  /** Full label, for the tooltip, when `label` was shortened. */
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** The tree node to open, for the case and its items. */
  node?: TreeNode;
  /** Items under a group. */
  count?: number;
}

export interface GraphEdge {
  from: string;
  to: string;
  /** SVG path: a horizontal S-curve from the parent's right edge to the child's left. */
  d: string;
}

export interface CaseGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  /** Bounding box of everything drawn. */
  bounds: { x: number; y: number; w: number; h: number };
}

const GROUP_ORDER: TreeNodeKind[] = ['document', 'person', 'sheet', 'event'];

export const GRAPH = {
  caseW: 170,
  groupW: 130,
  itemW: 190,
  h: 28,
  rowGap: 8,
  groupGap: 20,
  colGap: 56,
  maxChars: 24,
} as const;

function shorten(s: string, max: number): string {
  return s.length > max ? `${s.slice(0, max - 1)}…` : s;
}

function curve(a: GraphNode, b: GraphNode): string {
  const x1 = a.x + a.w;
  const y1 = a.y + a.h / 2;
  const x2 = b.x;
  const y2 = b.y + b.h / 2;
  const mx = (x1 + x2) / 2;
  return `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`;
}

/**
 * `nodes` is every tree node filed under the case (the case node itself
 * included); `groupLabel` names a group in the user's language.
 */
export function layoutCaseGraph(
  caseNode: TreeNode,
  nodes: TreeNode[],
  groupLabel: (kind: TreeNodeKind) => string,
): CaseGraph {
  const items = nodes.filter((n) => n.id !== caseNode.id && n.kind !== 'case');
  const byKind = new Map<TreeNodeKind, TreeNode[]>();
  for (const n of items) {
    const list = byKind.get(n.kind) ?? [];
    list.push(n);
    byKind.set(n.kind, list);
  }

  const { caseW, groupW, itemW, h, rowGap, groupGap, colGap, maxChars } = GRAPH;
  const groupX = caseW + colGap;
  const itemX = groupX + groupW + colGap;

  const out: GraphNode[] = [];
  const edges: GraphEdge[] = [];
  const groups: GraphNode[] = [];
  let y = 0;

  for (const kind of GROUP_ORDER) {
    const list = byKind.get(kind);
    if (!list?.length) continue;
    const top = y;
    const itemNodes = list.map((n, i) => {
      const label = n.kind === 'case' ? (n.detail ?? n.label) : n.label;
      return {
        id: n.id,
        kind: n.kind,
        label: shorten(label, maxChars),
        title: label,
        x: itemX,
        y: top + i * (h + rowGap),
        w: itemW,
        h,
        node: n,
      } satisfies GraphNode;
    });
    const bottom = top + list.length * (h + rowGap) - rowGap;
    const name = groupLabel(kind);
    const group: GraphNode = {
      id: `group:${kind}`,
      kind: 'group',
      groupOf: kind,
      label: name,
      title: name,
      x: groupX,
      // Centred on its items.
      y: (top + bottom) / 2 - h / 2,
      w: groupW,
      h,
      count: list.length,
    };
    groups.push(group);
    out.push(group, ...itemNodes);
    for (const item of itemNodes) edges.push({ from: group.id, to: item.id, d: curve(group, item) });
    y = bottom + rowGap + groupGap;
  }

  const caseLabel = caseNode.detail ?? caseNode.label;
  const lastGroup = groups.at(-1);
  const caseY = groups.length && lastGroup ? (groups[0].y + lastGroup.y) / 2 : 0;
  const root: GraphNode = {
    id: caseNode.id,
    kind: 'case',
    label: shorten(caseLabel, maxChars - 4),
    title: caseLabel,
    x: 0,
    y: caseY,
    w: caseW,
    h,
    node: caseNode,
  };
  // Case → group edges first, in group order, so they draw under the rest.
  edges.unshift(...groups.map((g) => ({ from: root.id, to: g.id, d: curve(root, g) })));
  out.unshift(root);

  const minY = Math.min(...out.map((n) => n.y));
  const maxY = Math.max(...out.map((n) => n.y + n.h));
  const maxX = Math.max(...out.map((n) => n.x + n.w));
  return { nodes: out, edges, bounds: { x: 0, y: minY, w: maxX, h: maxY - minY } };
}

/** How many things are connected to the case — zero means "nothing to draw". */
export function connectionCount(caseNode: TreeNode, nodes: TreeNode[]): number {
  return nodes.filter((n) => n.id !== caseNode.id && n.kind !== 'case').length;
}
