import { describe, expect, it } from 'bun:test';
import { targetOf } from '../src/lib/artifacts';
import type { TreeNode } from '../src/lib/types';

function n(partial: Partial<TreeNode> & Pick<TreeNode, 'id' | 'kind'>): TreeNode {
  return {
    nodeKey: 'case-01/x',
    label: 'Label',
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

describe('targetOf (context-menu targets from tree nodes)', () => {
  it('a case uses its case id, and its title as the label', () => {
    const t = targetOf(n({ id: 'case:c1', kind: 'case', caseId: 'c1', label: 'case-01', detail: 'Henderson v. Ardent' }));
    expect(t).toEqual({ kind: 'case', id: 'c1', label: 'Henderson v. Ardent', nodeKey: 'case-01/x' });
  });

  it('a document uses its document id', () => {
    const t = targetOf(n({ id: 'document:d1', kind: 'document', documentId: 'd1', label: 'answer.pdf' }));
    expect(t).toMatchObject({ kind: 'document', id: 'd1', label: 'answer.pdf' });
  });

  it('a sheet takes its id from the node id', () => {
    expect(targetOf(n({ id: 'sheet:s9', kind: 'sheet' }))).toMatchObject({ kind: 'sheet', id: 's9' });
  });

  it('a person splits person and case out of `person:<id>:<case>`', () => {
    const t = targetOf(n({ id: 'person:p1:c2', kind: 'person', caseId: 'c2', label: 'Miriam Okonkwo' }));
    expect(t).toMatchObject({ kind: 'person', id: 'p1', caseId: 'c2', label: 'Miriam Okonkwo' });
  });

  it('an event carries its case and start, for jumping the calendar to it', () => {
    const t = targetOf(
      n({ id: 'event:e1', kind: 'event', caseId: 'c1', startsAt: '2026-10-03T09:00:00Z', label: 'Hearing' }),
    );
    expect(t).toMatchObject({ kind: 'event', id: 'e1', caseId: 'c1', startsAt: '2026-10-03T09:00:00Z' });
  });

  it('returns null when a node lacks the id it needs', () => {
    expect(targetOf(n({ id: 'case:c1', kind: 'case', caseId: null }))).toBeNull();
    expect(targetOf(n({ id: 'document:d1', kind: 'document', documentId: null }))).toBeNull();
  });
});
