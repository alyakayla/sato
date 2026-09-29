import type { TreeNode, TreeNodeKind } from './types';

/**
 * The case tree and the `@` mention syntax that drives it.
 *
 * A mention is written `@case-ref/file-name` and resolves against the
 * `nodeKey` of a tree node. Mentions are stored verbatim in the message body so
 * the original text stays readable and searchable, and are re-resolved on
 * render against the live tree.
 */

/** Matches `@` followed by a handle. Deliberately conservative: stops at
 *  whitespace and punctuation so ordinary prose is safe. */
const MENTION_RE = /@([A-Za-z0-9][A-Za-z0-9._\-/]*)/g;

/**
 * A handle only counts when the `@` is not itself part of a word.
 *
 * Without this, `me@work.com` parses as a mention of `work.com` and the
 * composer pops a menu in the middle of an email address.
 */
function isMentionStart(text: string, index: number): boolean {
  return index === 0 || !/[\w.@-]/.test(text[index - 1]!);
}

export interface Mention {
  /** The full token including the `@`. */
  raw: string;
  key: string;
  start: number;
  end: number;
}

/** Extracts every `@handle` token from a message. */
export function parseMentions(text: string): Mention[] {
  const out: Mention[] = [];
  for (const m of text.matchAll(MENTION_RE)) {
    if (!isMentionStart(text, m.index)) continue;
    out.push({ raw: m[0], key: m[1].toLowerCase(), start: m.index, end: m.index + m[0].length });
  }
  return out;
}

/**
 * The `@` fragment the caret is currently inside, or null when it is not in a
 * mention. Shared with the composer so the popup and the parser agree on what
 * counts as a handle.
 */
export function mentionFragment(
  text: string,
  caret: number,
): { start: number; query: string } | null {
  const upto = text.slice(0, caret);
  const m = /@([A-Za-z0-9._\-/]*)$/.exec(upto);
  if (!m) return null;
  const start = upto.length - m[0].length;
  if (!isMentionStart(text, start)) return null;
  return { start, query: m[1] };
}

export interface Segment {
  text: string;
  mention: Mention | null;
  node: TreeNode | null;
}

/**
 * Splits message text into plain and mention segments so a bubble can render
 * mentions as clickable chips. An unresolvable token stays plain text: the
 * document may have been deleted since the message was sent.
 */
const keyMapCache = new WeakMap<TreeNode[], Map<string, TreeNode>>();

/**
 * Handle → node, built once per tree array and shared. Every chat message
 * resolves its mentions through this; building it per message made each
 * render cost messages × tree size, which froze the chat on large trees.
 */
export function nodeKeyMap(nodes: TreeNode[]): Map<string, TreeNode> {
  let map = keyMapCache.get(nodes);
  if (!map) {
    map = new Map(nodes.map((n) => [n.nodeKey.toLowerCase(), n]));
    keyMapCache.set(nodes, map);
  }
  return map;
}

export function segmentMessage(text: string, nodes: TreeNode[]): Segment[] {
  const byKey = nodeKeyMap(nodes);
  const out: Segment[] = [];
  let cursor = 0;
  for (const mention of parseMentions(text)) {
    if (mention.start > cursor) {
      out.push({ text: text.slice(cursor, mention.start), mention: null, node: null });
    }
    out.push({
      text: mention.raw,
      mention,
      node: byKey.get(mention.key) ?? null,
    });
    cursor = mention.end;
  }
  if (cursor < text.length) {
    out.push({ text: text.slice(cursor), mention: null, node: null });
  }
  return out;
}

/** Document ids named by mentions in a message, for scoped retrieval. */
export function mentionedDocumentIds(text: string, nodes: TreeNode[]): string[] {
  const byKey = nodeKeyMap(nodes);
  const ids = new Set<string>();
  for (const m of parseMentions(text)) {
    const node = byKey.get(m.key);
    if (node?.documentId) ids.add(node.documentId);
  }
  return [...ids];
}

// ---------------------------------------------------------------------------
// Tree shaping
// ---------------------------------------------------------------------------

/**
 * Flattens the flat node list into visible rows, honouring which parents are
 * collapsed. Children keep the order the backend assigned, so a case always
 * shows documents, people, sheets and events in a predictable order.
 */
export function visibleRows(nodes: TreeNode[], collapsed: Set<string>): TreeNode[] {
  const children = new Map<string | null, TreeNode[]>();
  for (const n of nodes) {
    const list = children.get(n.parentId) ?? [];
    list.push(n);
    children.set(n.parentId, list);
  }
  for (const list of children.values()) {
    list.sort((a, b) => a.ordinal - b.ordinal);
  }

  const out: TreeNode[] = [];
  const walk = (parent: string | null) => {
    for (const node of children.get(parent) ?? []) {
      out.push(node);
      if (!collapsed.has(node.id)) walk(node.id);
    }
  };
  walk(null);
  return out;
}

/** Every ancestor id of a node, nearest first. */
export function ancestorsOf(nodes: TreeNode[], id: string): string[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const out: string[] = [];
  let cur = byId.get(id)?.parentId ?? null;
  while (cur) {
    out.push(cur);
    cur = byId.get(cur)?.parentId ?? null;
  }
  return out;
}

/**
 * Filters the tree by a free-text query, keeping any node that matches along
 * with all of its ancestors so the result still reads as a tree rather than a
 * flat list of hits.
 */
export function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
  const q = query.trim().toLowerCase();
  if (!q) return nodes;
  const keep = new Set<string>();
  const byId = new Map(nodes.map((n) => [n.id, n]));

  for (const n of nodes) {
    const hay = `${n.label} ${n.nodeKey} ${n.status ?? ''} ${n.detail ?? ''}`.toLowerCase();
    if (hay.includes(q)) {
      for (const a of ancestorsOf(nodes, n.id)) keep.add(a);
      keep.add(n.id);
    }
  }

  // Ancestors were added to `keep` above, so this preserves tree structure
  // rather than flattening results into a bare hit list.
  return nodes.filter((n) => keep.has(n.id));
}

export interface MentionSuggestion {
  node: TreeNode;
  /** Text inserted into the composer, including the leading `@`. */
  insert: string;
}

/**
 * One searchable entry per node, lowercased once. Built when the tree changes,
 * not on every keystroke: typing then costs a scan of plain strings with no
 * allocation per node.
 */
export interface MentionEntry {
  /** Index into the node array the entry was built from. */
  i: number;
  key: string;
  label: string;
  kind: TreeNodeKind;
}

/** The subset of a node the ranking needs, so it can cross to a worker cheaply. */
export type MentionSource = Pick<TreeNode, 'nodeKey' | 'label' | 'kind'>;

export function buildMentionIndex(nodes: MentionSource[]): MentionEntry[] {
  return nodes.map((n, i) => ({ i, key: n.nodeKey.toLowerCase(), label: n.label.toLowerCase(), kind: n.kind }));
}

function scoreEntry(e: MentionEntry, q: string): number | null {
  let score: number;
  if (e.key === q) score = 100;
  else if (e.key.startsWith(q)) score = 80 - e.key.length * 0.1;
  else if (e.label.startsWith(q)) score = 70 - e.label.length * 0.1;
  else if (e.key.includes(q)) score = 50 - e.key.length * 0.1;
  else if (e.label.includes(q)) score = 40 - e.label.length * 0.1;
  else return null;
  // Documents are the most common thing to reference, so nudge them ahead of
  // their parent case when both match equally.
  if (e.kind === 'document') score += 5;
  if (e.kind === 'case') score -= 2;
  return score;
}

/**
 * Ranks an index against a query, returning node indices best-first. Keeps
 * only the top `limit` as it goes, so an autocomplete over a huge tree never
 * sorts every match. Ties keep tree order.
 */
export function rankMentions(index: MentionEntry[], query: string, limit: number): number[] {
  const q = query.trim().toLowerCase();
  if (!q || limit <= 0) return [];

  // A large limit (quick find wants every match) would make the insertion
  // below quadratic, so collect and sort once instead. Array#sort is stable,
  // so ties still keep tree order.
  if (limit > 64) {
    const all: { i: number; score: number }[] = [];
    for (const e of index) {
      const score = scoreEntry(e, q);
      if (score !== null) all.push({ i: e.i, score });
    }
    all.sort((a, b) => b.score - a.score);
    return all.slice(0, limit).map((t) => t.i);
  }

  const top: { i: number; score: number }[] = [];
  for (const e of index) {
    const score = scoreEntry(e, q);
    if (score === null) continue;
    if (top.length === limit && score <= top[top.length - 1].score) continue;
    // Insert after any equal score, so earlier nodes win ties.
    let at = top.length;
    while (at > 0 && top[at - 1].score < score) at--;
    top.splice(at, 0, { i: e.i, score });
    if (top.length > limit) top.pop();
  }
  return top.map((t) => t.i);
}

const indexCache = new WeakMap<TreeNode[], MentionEntry[]>();

/** The index for a node array, built once per array (the tree store replaces it on change). */
export function mentionIndexFor(nodes: TreeNode[]): MentionEntry[] {
  let index = indexCache.get(nodes);
  if (!index) {
    index = buildMentionIndex(nodes);
    indexCache.set(nodes, index);
  }
  return index;
}

/**
 * Ranks nodes for the composer's `@` autocomplete. Prefers deeper, more
 * specific handles so typing `@case-01/d` surfaces the document rather than the
 * case it belongs to.
 */
export function suggestMentions(
  nodes: TreeNode[],
  query: string,
  limit = 8,
): MentionSuggestion[] {
  return rankMentions(mentionIndexFor(nodes), query, limit).map((i) => ({
    node: nodes[i],
    insert: `@${nodes[i].nodeKey} `,
  }));
}

/**
 * Instant `@` resolution for the composer. No ranking and no scan: two sorted
 * arrays — full handles, and file names — searched by binary search, so each
 * keystroke is O(log n) however large the tree is. The real document search
 * happens only when the message is sent.
 */
export interface MentionLookup {
  byKey: { k: string; i: number }[];
  byName: { k: string; i: number }[];
}

export function buildMentionLookup(nodes: Pick<TreeNode, 'nodeKey'>[]): MentionLookup {
  const byKey: { k: string; i: number }[] = [];
  const byName: { k: string; i: number }[] = [];
  nodes.forEach((n, i) => {
    const k = n.nodeKey.toLowerCase();
    byKey.push({ k, i });
    const slash = k.lastIndexOf('/');
    if (slash >= 0) byName.push({ k: k.slice(slash + 1), i });
  });
  // Tree order breaks ties, so equal handles resolve the same way every time.
  const cmp = (a: { k: string; i: number }, b: { k: string; i: number }) =>
    a.k < b.k ? -1 : a.k > b.k ? 1 : a.i - b.i;
  byKey.sort(cmp);
  byName.sort(cmp);
  return { byKey, byName };
}

/** First index in `arr` whose key is >= `k`. */
function lowerBound(arr: { k: string }[], k: string): number {
  let lo = 0;
  let hi = arr.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (arr[mid].k < k) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The slice of `arr` whose keys start with `prefix`. */
function prefixRange(arr: { k: string }[], prefix: string): [number, number] {
  // U+FFFF sorts after every character a handle can contain.
  return [lowerBound(arr, prefix), lowerBound(arr, prefix + '￿')];
}

export interface ResolvedMention {
  node: TreeNode;
  /** Other nodes that also match, for "+N more". */
  more: number;
}

/**
 * The node an `@` fragment points at: an exact or prefix match on the full
 * handle (`case-01/ans…`), or failing that on the file name alone (`ans…`).
 * The first match in sorted order wins, so an exact handle always beats a
 * longer one that merely starts with it.
 */
export function resolveMention(lookup: MentionLookup, nodes: TreeNode[], query: string): ResolvedMention | null {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  for (const arr of [lookup.byKey, lookup.byName]) {
    const [from, to] = prefixRange(arr, q);
    if (to > from) return { node: nodes[arr[from].i], more: to - from - 1 };
  }
  return null;
}

const lookupCache = new WeakMap<TreeNode[], MentionLookup>();

/** The lookup for a node array, built once per array (the tree store replaces it on change). */
export function mentionLookupFor(nodes: TreeNode[]): MentionLookup {
  let lookup = lookupCache.get(nodes);
  if (!lookup) {
    lookup = buildMentionLookup(nodes);
    lookupCache.set(nodes, lookup);
  }
  return lookup;
}

export interface QuickFindGroup {
  kind: TreeNodeKind;
  nodes: TreeNode[];
}

/**
 * Quick-find results: ranked like the `@` autocomplete, then grouped by kind in
 * `KIND_ORDER` so the list has a stable shape as the query changes. Each group
 * is capped so one prolific kind cannot push the others off screen.
 */
export function quickFind(nodes: TreeNode[], query: string, perKind = 5): QuickFindGroup[] {
  const q = query.trim().toLowerCase();
  if (!q || perKind <= 0) return [];
  // Keep only the best `perKind` of each kind while scanning: ranking and
  // sorting every match cost ~100 ms per keystroke on a 50k-node tree, for a
  // list that shows at most five rows per kind.
  const top = new Map<TreeNodeKind, { i: number; score: number }[]>();
  for (const e of mentionIndexFor(nodes)) {
    const score = scoreEntry(e, q);
    if (score === null) continue;
    let list = top.get(e.kind);
    if (!list) top.set(e.kind, (list = []));
    if (list.length === perKind && score <= list[list.length - 1].score) continue;
    let at = list.length;
    while (at > 0 && list[at - 1].score < score) at--;
    list.splice(at, 0, { i: e.i, score });
    if (list.length > perKind) list.pop();
  }
  return KIND_ORDER.filter((k) => top.has(k)).map((kind) => ({ kind, nodes: top.get(kind)!.map((t) => nodes[t.i]) }));
}

/** O(1) lookups into one tree array. */
export interface NodeIndex {
  /** By node id (`case:…`, `document:…`, `sheet:…`, `person:…:…`, `event:…`). */
  byId: Map<string, TreeNode>;
  byDocumentId: Map<string, TreeNode>;
  /** The case node for a case id. */
  byCaseId: Map<string, TreeNode>;
}

const nodeIndexCache = new WeakMap<TreeNode[], NodeIndex>();

/**
 * Built once per tree array and shared. Views that map rows back to tree
 * nodes (tables, menus) used `nodes.find` per row — rows × tree size, a
 * four-second freeze on a 50k-node tree.
 */
export function nodeIndex(nodes: TreeNode[]): NodeIndex {
  let idx = nodeIndexCache.get(nodes);
  if (!idx) {
    idx = { byId: new Map(), byDocumentId: new Map(), byCaseId: new Map() };
    for (const n of nodes) {
      idx.byId.set(n.id, n);
      if (n.documentId) idx.byDocumentId.set(n.documentId, n);
      if (n.kind === 'case' && n.caseId) idx.byCaseId.set(n.caseId, n);
    }
    nodeIndexCache.set(nodes, idx);
  }
  return idx;
}

// ---------------------------------------------------------------------------
// Presentation helpers
// ---------------------------------------------------------------------------

export const KIND_META: Record<TreeNodeKind, { label: string; icon: string }> = {
  case: { label: 'Case', icon: '◆' },
  document: { label: 'Document', icon: '▤' },
  person: { label: 'Person', icon: '●' },
  sheet: { label: 'Sheet', icon: '▦' },
  event: { label: 'Event', icon: '◷' },
};

export const KIND_ORDER: TreeNodeKind[] = ['case', 'document', 'person', 'sheet', 'event'];

/** Node counts per kind for the tree view header. */
export function countByKind(nodes: TreeNode[]): Record<TreeNodeKind, number> {
  const out: Record<TreeNodeKind, number> = {
    case: 0,
    document: 0,
    person: 0,
    sheet: 0,
    event: 0,
  };
  for (const n of nodes) out[n.kind]++;
  return out;
}
