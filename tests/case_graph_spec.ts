import { describe, expect, it } from 'bun:test';
import { connectionCount, GRAPH, layoutCaseGraph } from '../src/lib/caseGraph';
import type { TreeNode } from '../src/lib/types';

function n(partial: Partial<TreeNode> & Pick<TreeNode, 'id' | 'kind' | 'label'>): TreeNode {
  return {
    nodeKey: partial.id,
    parentId: null,
    depth: 1,
    ordinal: 0,
    status: null,
    detail: null,
    color: null,
    caseId: 'c1',
    documentId: null,
    sizeBytes: null,
    updatedAt: '2026-01-01T00:00:00Z',
    startsAt: null,
    ...partial,
  };
}

const caseNode = n({ id: 'case:c1', kind: 'case', label: 'case-01', detail: 'Henderson v. Ardent Holdings Limited', depth: 0 });
const items = [
  n({ id: 'event:e1', kind: 'event', label: 'Hearing' }),
  n({ id: 'document:d1', kind: 'document', label: 'answer.pdf', documentId: 'd1' }),
  n({ id: 'document:d2', kind: 'document', label: 'a-very-long-document-name-that-overflows.pdf', documentId: 'd2' }),
  n({ id: 'person:p1:c1', kind: 'person', label: 'Miriam Okonkwo' }),
];
const label = (k: string) => `[${k}]`;

describe('layoutCaseGraph', () => {
  const g = layoutCaseGraph(caseNode, [caseNode, ...items], label);

  it('puts the case first, then one group per kind present, in a fixed order', () => {
    expect(g.nodes[0].kind).toBe('case');
    expect(g.nodes.filter((x) => x.kind === 'group').map((x) => x.groupOf)).toEqual(['document', 'person', 'event']);
  });

  it('connects the case to each group and each group to its items', () => {
    const groups = g.nodes.filter((x) => x.kind === 'group');
    expect(g.edges.filter((e) => e.from === caseNode.id).map((e) => e.to)).toEqual(groups.map((x) => x.id));
    expect(g.edges.filter((e) => e.from === 'group:document').map((e) => e.to)).toEqual(['document:d1', 'document:d2']);
    expect(g.edges).toHaveLength(groups.length + items.length);
  });

  it('lays out three columns, left to right', () => {
    const xs = new Set(g.nodes.map((x) => x.x));
    expect(xs.size).toBe(3);
    const group = g.nodes.find((x) => x.id === 'group:document')!;
    const item = g.nodes.find((x) => x.id === 'document:d1')!;
    expect(group.x).toBeGreaterThan(0);
    expect(item.x).toBeGreaterThan(group.x + group.w);
  });

  it('centres each group on its items, and the case on its groups', () => {
    const group = g.nodes.find((x) => x.id === 'group:document')!;
    const kids = g.nodes.filter((x) => x.kind === 'document');
    const mid = (kids[0].y + kids.at(-1)!.y) / 2;
    expect(group.y).toBeCloseTo(mid);
    const groups = g.nodes.filter((x) => x.kind === 'group');
    expect(g.nodes[0].y).toBeCloseTo((groups[0].y + groups.at(-1)!.y) / 2);
  });

  it('never overlaps two items', () => {
    const rows = g.nodes.filter((x) => x.kind !== 'group' && x.kind !== 'case').sort((a, b) => a.y - b.y);
    for (let i = 1; i < rows.length; i++) expect(rows[i].y).toBeGreaterThanOrEqual(rows[i - 1].y + GRAPH.h);
  });

  it('shortens long labels but keeps the full title for the tooltip', () => {
    const long = g.nodes.find((x) => x.id === 'document:d2')!;
    expect(long.label.length).toBeLessThanOrEqual(GRAPH.maxChars);
    expect(long.label.endsWith('…')).toBe(true);
    expect(long.title).toBe('a-very-long-document-name-that-overflows.pdf');
    expect(g.nodes[0].title).toBe('Henderson v. Ardent Holdings Limited');
  });

  it('reports bounds that contain every node', () => {
    for (const x of g.nodes) {
      expect(x.x).toBeGreaterThanOrEqual(g.bounds.x);
      expect(x.y).toBeGreaterThanOrEqual(g.bounds.y);
      expect(x.x + x.w).toBeLessThanOrEqual(g.bounds.x + g.bounds.w);
      expect(x.y + x.h).toBeLessThanOrEqual(g.bounds.y + g.bounds.h);
    }
  });
});

describe('connectionCount', () => {
  it('counts what is filed under the case, not the case itself', () => {
    expect(connectionCount(caseNode, [caseNode, ...items])).toBe(4);
    expect(connectionCount(caseNode, [caseNode])).toBe(0);
  });
});
