import { describe, expect, it } from 'bun:test';
import {
  ancestorsOf,
  countByKind,
  filterTree,
  mentionedDocumentIds,
  mentionFragment,
  parseMentions,
  segmentMessage,
  suggestMentions,
  visibleRows,
} from '../src/lib/tree';
import type { TreeNode } from '../src/lib/types';

/**
 * Builds a tree node with sensible defaults, so each test only states the
 * fields it actually cares about.
 */
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

/**
 * The fixture the mention syntax was designed around: `case-01` holds a
 * document called `document-abc.pdf`, exactly as in the product brief.
 */
const TREE: TreeNode[] = [
  n({
    id: 'case:1',
    nodeKey: 'case-01',
    kind: 'case',
    label: 'Henderson v. Ardent',
    detail: 'case-01',
    caseId: 'c1',
    ordinal: 0,
  }),
  n({
    id: 'document:d1',
    nodeKey: 'case-01/document-abc.pdf',
    kind: 'document',
    label: 'document-abc.pdf',
    parentId: 'case:1',
    depth: 1,
    caseId: 'c1',
    documentId: 'doc-1',
    status: 'Ready',
    ordinal: 1,
  }),
  n({
    id: 'person:p1:1',
    nodeKey: 'case-01/person-1',
    kind: 'person',
    label: 'Dana Whitfield',
    parentId: 'case:1',
    depth: 1,
    caseId: 'c1',
    ordinal: 2,
  }),
  n({
    id: 'case:2',
    nodeKey: 'case-02',
    kind: 'case',
    label: 'In re Okonkwo',
    detail: 'case-02',
    caseId: 'c2',
    ordinal: 3,
  }),
  n({
    id: 'event:e1',
    nodeKey: 'case-02/hearing',
    kind: 'event',
    label: 'Hearing',
    parentId: 'case:2',
    depth: 1,
    caseId: 'c2',
    status: 'Hearing',
    ordinal: 4,
    startsAt: '2030-04-01T09:00:00Z',
  }),
  // An upload that was never filed against a case hangs off the root.
  n({
    id: 'document:d2',
    nodeKey: 'orphan-notes.pdf',
    kind: 'document',
    label: 'orphan-notes.pdf',
    documentId: 'doc-2',
    ordinal: 5,
  }),
];

describe('parseMentions', () => {
  it('finds a mention in prose', () => {
    const m = parseMentions('see @case-01/document-abc.pdf for the answer');
    expect(m).toHaveLength(1);
    expect(m[0].raw).toBe('@case-01/document-abc.pdf');
    expect(m[0].key).toBe('case-01/document-abc.pdf');
  });

  it('records positions so the text can be split', () => {
    const text = 'hi @case-01 x';
    const [m] = parseMentions(text);
    expect(text.slice(m.start, m.end)).toBe('@case-01');
  });

  it('finds several mentions', () => {
    const m = parseMentions('@case-01/a.pdf and @case-02/b.pdf');
    expect(m.map((x) => x.key)).toEqual(['case-01/a.pdf', 'case-02/b.pdf']);
  });

  it('lowercases the key so matching is case-insensitive', () => {
    expect(parseMentions('@CASE-01/Doc.PDF')[0].key).toBe('case-01/doc.pdf');
  });

  it('stops at whitespace so a file name with spaces does not swallow the rest', () => {
    // `@case-01` then a space, then ordinary words.
    expect(parseMentions('@case-01 and then').map((m) => m.key)).toEqual(['case-01']);
  });

  it('ignores a bare email-style handle preceded by a word character', () => {
    // `me@work` is not a mention: the @ is preceded by a letter.
    expect(parseMentions('email me@work tomorrow')).toHaveLength(0);
  });

  it('returns nothing for plain text', () => {
    expect(parseMentions('no handles here')).toHaveLength(0);
  });
});

describe('mentionFragment', () => {
  const at = (text: string) => mentionFragment(text, text.length);

  it('detects a fragment being typed at the caret', () => {
    expect(at('look at @case-01/do')).toEqual({ start: 8, query: 'case-01/do' });
  });

  it('detects a bare @ with no query yet', () => {
    expect(at('@')).toEqual({ start: 0, query: '' });
  });

  it('returns null outside a mention', () => {
    expect(at('no handles')).toBeNull();
  });

  it('closes the fragment as soon as a space is typed', () => {
    // The handle ends at whitespace, so the popup must not follow the caret
    // into the rest of the sentence.
    expect(at('@case-01 and more')).toBeNull();
    expect(at('@case-01 ')).toBeNull();
  });

  it('returns null inside an email address', () => {
    expect(at('email me@work')).toBeNull();
  });

  it('only inspects text before the caret', () => {
    // Caret sits before the mention, so the popup must not be open.
    expect(mentionFragment('@case-01 ok', 0)).toBeNull();
  });
});

describe('segmentMessage', () => {
  it('splits text around a resolvable mention', () => {
    const segs = segmentMessage('look at @case-01/document-abc.pdf now', TREE);
    expect(segs.map((s) => s.text)).toEqual([
      'look at ',
      '@case-01/document-abc.pdf',
      ' now',
    ]);
    expect(segs[1].node?.documentId).toBe('doc-1');
  });

  it('leaves an unresolvable mention as text with no node', () => {
    const segs = segmentMessage('@case-99/missing.pdf', TREE);
    expect(segs).toHaveLength(1);
    expect(segs[0].mention).not.toBeNull();
    expect(segs[0].node).toBeNull();
  });

  it('round-trips the original text exactly', () => {
    const text = 'compare @case-01/document-abc.pdf with @case-02/hearing?';
    expect(segmentMessage(text, TREE).map((s) => s.text).join('')).toBe(text);
  });

  it('handles a mention at the very start and end', () => {
    const segs = segmentMessage('@case-01 x @case-02', TREE);
    expect(segs.map((s) => s.text)).toEqual(['@case-01', ' x ', '@case-02']);
  });
});

describe('mentionedDocumentIds', () => {
  it('collects the documents named in a message', () => {
    expect(
      mentionedDocumentIds('what does @case-01/document-abc.pdf say?', TREE),
    ).toEqual(['doc-1']);
  });

  it('ignores a case mention, which is a scope not a document', () => {
    expect(mentionedDocumentIds('summarise @case-01', TREE)).toEqual([]);
  });

  it('deduplicates a document mentioned twice', () => {
    const ids = mentionedDocumentIds(
      '@case-01/document-abc.pdf and again @case-01/document-abc.pdf',
      TREE,
    );
    expect(ids).toEqual(['doc-1']);
  });

  it('resolves an unfiled document by its bare file name', () => {
    expect(mentionedDocumentIds('check @orphan-notes.pdf', TREE)).toEqual(['doc-2']);
  });

  it('returns an empty list when nothing is referenced', () => {
    expect(mentionedDocumentIds('general question', TREE)).toEqual([]);
  });
});

describe('visibleRows', () => {
  it('lists every node when nothing is collapsed', () => {
    expect(visibleRows(TREE, new Set()).map((x) => x.id)).toEqual([
      'case:1',
      'document:d1',
      'person:p1:1',
      'case:2',
      'event:e1',
      'document:d2',
    ]);
  });

  it('hides descendants of a collapsed case', () => {
    const ids = visibleRows(TREE, new Set(['case:1'])).map((x) => x.id);
    expect(ids).toEqual(['case:1', 'case:2', 'event:e1', 'document:d2']);
  });

  it('orders siblings by ordinal, not by arrival', () => {
    const shuffled = [
      TREE[0],
      { ...TREE[1], ordinal: 9 },
      { ...TREE[2], ordinal: 2 },
    ];
    expect(visibleRows(shuffled, new Set()).map((x) => x.id)).toEqual([
      'case:1',
      'person:p1:1',
      'document:d1',
    ]);
  });

  it('returns an empty list for no nodes', () => {
    expect(visibleRows([], new Set())).toEqual([]);
  });
});

describe('ancestorsOf', () => {
  it('walks up nearest-first', () => {
    expect(ancestorsOf(TREE, 'event:e1')).toEqual(['case:2']);
  });

  it('returns nothing for a root node', () => {
    expect(ancestorsOf(TREE, 'case:1')).toEqual([]);
  });
});

describe('filterTree', () => {
  it('returns the whole tree for an empty query', () => {
    expect(filterTree(TREE, '   ')).toHaveLength(TREE.length);
  });

  it('matches on the label', () => {
    // Only the case itself matches; its children are not pulled in, so the
    // filter reads as "what I searched for" rather than "everything nearby".
    expect(filterTree(TREE, 'henderson').map((x) => x.id)).toEqual(['case:1']);
  });

  it('matches on the mention key so a reference finds its case', () => {
    expect(filterTree(TREE, 'case-02').map((x) => x.id)).toEqual(['case:2', 'event:e1']);
  });

  it('keeps ancestors of a match so the result still reads as a tree', () => {
    // Matching the person alone still surfaces the case it belongs to.
    expect(filterTree(TREE, 'whitfield').map((x) => x.id)).toEqual(['case:1', 'person:p1:1']);
  });

  it('matches on status', () => {
    expect(filterTree(TREE, 'ready').map((x) => x.id)).toEqual(['case:1', 'document:d1']);
  });

  it('is case-insensitive', () => {
    expect(filterTree(TREE, 'HENDERSON').map((x) => x.id)).toEqual(['case:1']);
  });

  it('returns nothing when there is no match', () => {
    expect(filterTree(TREE, 'zzzz')).toEqual([]);
  });
});

describe('suggestMentions', () => {
  it('offers nothing for an empty query', () => {
    expect(suggestMentions(TREE, '')).toEqual([]);
  });

  it('prefix-matches the mention key', () => {
    expect(suggestMentions(TREE, 'case-01/').map((s) => s.node.id)).toEqual([
      'document:d1',
      'person:p1:1',
    ]);
  });

  it('ranks an exact handle match above its children', () => {
    // Typing the complete reference `case-01` most likely means the case.
    expect(suggestMentions(TREE, 'case-01')[0].node.kind).toBe('case');
  });

  it('surfaces the child document once the query extends past the parent key', () => {
    // The one extra character is what disambiguates, and the document is the
    // more useful target from there.
    expect(suggestMentions(TREE, 'case-01/')[0].node.kind).toBe('document');
  });

  it('inserts an @-prefixed token with a trailing space', () => {
    expect(suggestMentions(TREE, 'orphan')[0].insert).toBe('@orphan-notes.pdf ');
  });

  it('matches on the human label too', () => {
    expect(suggestMentions(TREE, 'whitfield').map((s) => s.node.id)).toEqual(['person:p1:1']);
  });

  it('is case-insensitive', () => {
    expect(suggestMentions(TREE, 'CASE-01/').map((s) => s.node.id)).toEqual([
      'document:d1',
      'person:p1:1',
    ]);
  });

  it('caps the number of suggestions', () => {
    const many = Array.from({ length: 50 }, (_, i) =>
      n({
        id: `document:x${i}`,
        nodeKey: `case-01/file-${i}.pdf`,
        kind: 'document',
        label: `file-${i}.pdf`,
      }),
    );
    expect(suggestMentions(many, 'case-01/', 8)).toHaveLength(8);
  });
});

describe('countByKind', () => {
  it('counts each kind', () => {
    expect(countByKind(TREE)).toEqual({ case: 2, document: 2, person: 1, sheet: 0, event: 1 });
  });

  it('returns zeroes for an empty tree', () => {
    expect(countByKind([])).toEqual({ case: 0, document: 0, person: 0, sheet: 0, event: 0 });
  });
});
