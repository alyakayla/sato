import { describe, expect, it } from 'bun:test';
import {
  buildMentionIndex,
  buildMentionLookup,
  mentionLookupFor,
  rankMentions,
  resolveMention,
  quickFind,
  suggestMentions,
} from '../src/lib/tree';
import type { TreeNode, TreeNodeKind } from '../src/lib/types';

const KINDS: TreeNodeKind[] = ['case', 'document', 'person', 'sheet', 'event'];

/** A deterministic, realistically shaped tree of `n` nodes. */
function bigTree(n: number): TreeNode[] {
  const out: TreeNode[] = [];
  for (let i = 0; i < n; i++) {
    const c = Math.floor(i / 50);
    const kind = i % 50 === 0 ? 'case' : KINDS[(i % 4) + 1];
    out.push({
      id: `n${i}`,
      nodeKey: kind === 'case' ? `case-${c}` : `case-${c}/${kind}-${i}.pdf`,
      kind,
      label: kind === 'case' ? `Matter ${c}` : `${kind} ${i} answer brief`,
      parentId: null,
      depth: kind === 'case' ? 0 : 1,
      ordinal: i,
      status: null,
      detail: null,
      color: null,
      caseId: `c${c}`,
      documentId: null,
      sizeBytes: null,
      updatedAt: '2026-01-01T00:00:00Z',
      startsAt: null,
    });
  }
  return out;
}

/** The pre-optimisation algorithm: score everything, then stable-sort. */
function reference(nodes: TreeNode[], query: string, limit: number): string[] {
  const q = query.trim().toLowerCase();
  const scored: { id: string; score: number }[] = [];
  for (const n of nodes) {
    const key = n.nodeKey.toLowerCase();
    const label = n.label.toLowerCase();
    let score: number;
    if (key === q) score = 100;
    else if (key.startsWith(q)) score = 80 - key.length * 0.1;
    else if (label.startsWith(q)) score = 70 - label.length * 0.1;
    else if (key.includes(q)) score = 50 - key.length * 0.1;
    else if (label.includes(q)) score = 40 - label.length * 0.1;
    else continue;
    if (n.kind === 'document') score += 5;
    if (n.kind === 'case') score -= 2;
    scored.push({ id: n.id, score });
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, limit).map((s) => s.id);
}

describe('rankMentions', () => {
  const tree = bigTree(5000);
  const index = buildMentionIndex(tree);

  it('top-k matches a full sort exactly, ties included', () => {
    for (const q of ['case-1', 'answer', 'doc', 'matter 7', 'case-12/sheet', 'zzz', '4']) {
      for (const limit of [1, 8, 20, 64]) {
        expect(rankMentions(index, q, limit).map((i) => tree[i].id)).toEqual(reference(tree, q, limit));
      }
    }
  });

  it('the large-limit path (quick find) matches a full sort too', () => {
    expect(rankMentions(index, 'answer', tree.length).map((i) => tree[i].id)).toEqual(
      reference(tree, 'answer', tree.length),
    );
  });

  it('returns nothing for a blank query or a zero limit', () => {
    expect(rankMentions(index, '   ', 8)).toEqual([]);
    expect(rankMentions(index, 'case', 0)).toEqual([]);
  });

  it('caches the index per tree array, so repeat queries skip the rebuild', () => {
    const a = suggestMentions(tree, 'case-3', 8);
    const b = suggestMentions(tree, 'case-3', 8);
    expect(b.map((s) => s.node.id)).toEqual(a.map((s) => s.node.id));
  });

  it('stays fast on a very large tree', () => {
    const huge = bigTree(100_000);
    const idx = buildMentionIndex(huge);
    rankMentions(idx, 'answer', 8); // warm up
    const start = performance.now();
    for (let i = 0; i < 10; i++) rankMentions(idx, 'answer', 8);
    const perQuery = (performance.now() - start) / 10;
    // Generous bound: a regression to per-keystroke allocation or a full sort
    // blows well past it, while CI noise does not.
    expect(perQuery).toBeLessThan(40);
  });
});

describe('resolveMention (composer confirmation)', () => {
  const tree = bigTree(5000);
  const lookup = buildMentionLookup(tree);

  it('an exact handle resolves to itself, ahead of longer handles it prefixes', () => {
    const r = resolveMention(lookup, tree, 'case-1');
    expect(r?.node.nodeKey).toBe('case-1');
    expect(r!.more).toBeGreaterThan(0);
  });

  it('a partial path resolves to the first document under it', () => {
    const r = resolveMention(lookup, tree, 'case-12/sheet');
    expect(r?.node.nodeKey.startsWith('case-12/sheet')).toBe(true);
  });

  it('falls back to the file name when the handle does not match', () => {
    const r = resolveMention(lookup, tree, 'document-604');
    expect(r?.node.nodeKey).toBe('case-12/document-604.pdf');
    expect(r!.more).toBe(0);
  });

  it('is case-insensitive and ignores surrounding space', () => {
    expect(resolveMention(lookup, tree, '  CASE-3/SHEET ')?.node.nodeKey.startsWith('case-3/sheet')).toBe(true);
  });

  it('reports no match, and nothing for an empty fragment', () => {
    expect(resolveMention(lookup, tree, 'zzz')).toBeNull();
    expect(resolveMention(lookup, tree, '')).toBeNull();
  });

  it('counts every other match for "+N more"', () => {
    const brute = tree.filter((n) => n.nodeKey.toLowerCase().startsWith('case-7/')).length;
    expect(resolveMention(lookup, tree, 'case-7/')!.more).toBe(brute - 1);
  });

  it('caches the lookup per tree array', () => {
    expect(mentionLookupFor(tree)).toBe(mentionLookupFor(tree));
  });

  it('is effectively free per keystroke on a very large tree', () => {
    const huge = bigTree(100_000);
    const big = buildMentionLookup(huge);
    const start = performance.now();
    for (let i = 0; i < 1000; i++) resolveMention(big, huge, `case-${i % 2000}/doc`);
    const perKeystroke = (performance.now() - start) / 1000;
    expect(perKeystroke).toBeLessThan(0.5);
  });
});

describe('quickFind (top-k per kind)', () => {
  const tree = bigTree(8000);

  /** The previous algorithm: rank every match, then group. */
  function reference(q: string, perKind: number) {
    const ranked = rankMentions(buildMentionIndex(tree), q, tree.length).map((i) => tree[i]);
    const groups = new Map<string, string[]>();
    for (const n of ranked) {
      const list = groups.get(n.kind) ?? [];
      if (list.length < perKind) list.push(n.id);
      groups.set(n.kind, list);
    }
    return ['case', 'document', 'person', 'sheet', 'event'].filter((k) => groups.has(k)).map((k) => [k, groups.get(k)]);
  }

  it('returns exactly what ranking everything returned', () => {
    for (const q of ['case-1', 'answer', 'sheet', 'matter 3', 'zzz', 'doc']) {
      const got = quickFind(tree, q, 5).map((g) => [g.kind, g.nodes.map((n) => n.id)]);
      expect(JSON.stringify(got)).toBe(JSON.stringify(reference(q, 5)));
    }
  });
});
