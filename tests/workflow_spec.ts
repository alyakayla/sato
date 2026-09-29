import { describe, expect, it } from 'bun:test';
import { applyStep, citationCounts, initialSteps } from '../src/lib/steps';
import { expandSlash, matchSlash, slashFragment, SLASH_COMMANDS } from '../src/lib/commands';
import { quickFind } from '../src/lib/tree';
import type { TreeNode } from '../src/lib/types';

function n(partial: Partial<TreeNode> & Pick<TreeNode, 'id' | 'nodeKey' | 'kind' | 'label'>): TreeNode {
  return {
    parentId: null,
    depth: 0,
    ordinal: 0,
    status: null,
    detail: null,
    color: null,
    caseId: null,
    documentId: null,
    sizeBytes: null,
    updatedAt: '2026-01-01T00:00:00Z',
    startsAt: null,
    ...partial,
  };
}

describe('agent steps', () => {
  it('starts with every step pending, in order', () => {
    const rows = initialSteps();
    expect(rows.map((r) => r.key)).toEqual(['embed', 'search', 'generate']);
    expect(rows.every((r) => r.state === 'pending')).toBe(true);
  });

  it('marks earlier steps done when a later one starts', () => {
    const rows = applyStep(initialSteps(), { step: 'generate', state: 'running' });
    expect(rows.map((r) => r.state)).toEqual(['done', 'done', 'running']);
  });

  it('keeps retrieval counts once reported', () => {
    let rows = applyStep(initialSteps(), { step: 'search', state: 'done', passages: 8, documents: 3 });
    rows = applyStep(rows, { step: 'generate', state: 'running' });
    expect([rows[1].passages, rows[1].documents]).toEqual([8, 3]);
  });

  it('a skipped step stays skipped when later steps arrive', () => {
    let rows = applyStep(initialSteps(), { step: 'embed', state: 'skipped' });
    rows = applyStep(rows, { step: 'search', state: 'done', passages: 3, documents: 2 });
    rows = applyStep(rows, { step: 'generate', state: 'running' });
    expect(rows.map((r) => r.state)).toEqual(['skipped', 'done', 'running']);
  });

  it('ignores unknown steps', () => {
    const rows = initialSteps();
    expect(applyStep(rows, { step: 'nope' as never, state: 'done' })).toBe(rows);
  });
});

describe('citationCounts', () => {
  it('is null with no citations', () => {
    expect(citationCounts([])).toBeNull();
  });

  it('counts passages and distinct documents', () => {
    expect(citationCounts([{ documentId: 'a' }, { documentId: 'a' }, { documentId: 'b' }])).toEqual({
      passages: 3,
      documents: 2,
    });
  });
});

describe('slash commands', () => {
  it('only triggers at the very start of the composer', () => {
    expect(slashFragment('/sum', 4)).toBe('sum');
    expect(slashFragment('/', 1)).toBe('');
    expect(slashFragment('and/or', 6)).toBeNull();
    expect(slashFragment(' /sum', 5)).toBeNull();
  });

  it('closes once the caret leaves the command token', () => {
    expect(slashFragment('/summarize now', 14)).toBeNull();
  });

  it('matches by prefix', () => {
    expect(matchSlash('d').map((c) => c.name)).toEqual(['deadlines', 'draft']);
    expect(matchSlash('').length).toBe(SLASH_COMMANDS.length);
    expect(matchSlash('zzz')).toEqual([]);
  });

  it('expands the template and places the caret after the first @', () => {
    const out = expandSlash('Compare @{caret} and @ — where?', '/comp', 5);
    expect(out.text.startsWith('Compare @ and @')).toBe(true);
    expect(out.text.slice(0, out.caret)).toBe('Compare @');
  });

  it('keeps whatever followed the caret', () => {
    const out = expandSlash('Draft a {caret}', '/dr letter', 3);
    expect(out.text).toBe('Draft a  letter');
    expect(out.caret).toBe('Draft a '.length);
  });
});

describe('quickFind', () => {
  const TREE: TreeNode[] = [
    n({ id: 'c1', nodeKey: 'case-01', kind: 'case', label: 'Acme v. Smith', caseId: 'c1' }),
    n({ id: 'd1', nodeKey: 'case-01/answer.pdf', kind: 'document', label: 'answer.pdf', parentId: 'c1' }),
    n({ id: 'd2', nodeKey: 'case-01/answer-2.pdf', kind: 'document', label: 'answer-2.pdf', parentId: 'c1' }),
    n({ id: 'p1', nodeKey: 'case-01/jane', kind: 'person', label: 'Jane Answerly', parentId: 'c1' }),
  ];

  it('groups hits by kind in KIND_ORDER', () => {
    const groups = quickFind(TREE, 'answer');
    expect(groups.map((g) => g.kind)).toEqual(['document', 'person']);
  });

  it('caps each group', () => {
    const groups = quickFind(TREE, 'answer', 1);
    expect(groups[0].nodes.length).toBe(1);
  });

  it('returns nothing for an empty query', () => {
    expect(quickFind(TREE, '  ')).toEqual([]);
  });
});
