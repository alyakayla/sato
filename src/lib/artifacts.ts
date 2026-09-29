import type { TreeNode } from './types';

/**
 * What a context menu acts on. Kept free of app state so the node-id parsing
 * — the part that is easy to get subtly wrong — can be tested directly.
 */
export type ArtifactTarget =
  | { kind: 'case'; id: string; label: string; nodeKey?: string }
  | { kind: 'document'; id: string; label: string; nodeKey?: string }
  | { kind: 'sheet'; id: string; label: string; nodeKey?: string }
  | { kind: 'person'; id: string; label: string; caseId: string | null; nodeKey?: string }
  | { kind: 'event'; id: string; label: string; caseId: string | null; startsAt: string | null; nodeKey?: string };

/** Maps a tree node to its target. Node ids are `kind:<id>` (people: `person:<id>:<case>`). */
export function targetOf(node: TreeNode): ArtifactTarget | null {
  const [, id, extra] = node.id.split(':');
  const base = { label: node.kind === 'case' ? (node.detail ?? node.label) : node.label, nodeKey: node.nodeKey };
  switch (node.kind) {
    case 'case':
      return node.caseId ? { kind: 'case', id: node.caseId, ...base } : null;
    case 'document':
      return node.documentId ? { kind: 'document', id: node.documentId, ...base } : null;
    case 'sheet':
      return id ? { kind: 'sheet', id, ...base } : null;
    case 'person':
      return id ? { kind: 'person', id, caseId: extra ?? node.caseId, ...base } : null;
    case 'event':
      return id ? { kind: 'event', id, caseId: node.caseId, startsAt: node.startsAt, ...base } : null;
  }
}
