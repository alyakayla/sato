import type { ChatStep, Citation } from './types';

/**
 * The agent's progress through a chat turn, shown as a checklist while the
 * reply is being produced. The backend reports each stage as it happens; the
 * panel folds those events into this fixed sequence so the list never
 * reorders or skips a line.
 */

export type StepKey = ChatStep['step'];
export type StepState = 'pending' | 'running' | 'done' | 'skipped';

export interface StepRow {
  key: StepKey;
  state: StepState;
  /** Retrieval result for the search step; labels are translated at display. */
  passages: number | null;
  documents: number | null;
}

export const STEP_ORDER: StepKey[] = ['embed', 'search', 'generate'];

/** Pending and done glyphs; a running step shows the square spinner instead. */
export const STEP_GLYPH: Record<StepState, string> = {
  pending: '○',
  running: '',
  done: '✓',
  skipped: '–',
};

export function initialSteps(): StepRow[] {
  return STEP_ORDER.map((key) => ({ key, state: 'pending', passages: null, documents: null }));
}

/**
 * Applies one backend event. A step reaching any state implies every step
 * before it has finished — the backend does not always report "done" for a
 * stage before starting the next, and the list must still read correctly.
 */
export function applyStep(
  rows: StepRow[],
  ev: Pick<ChatStep, 'step' | 'state'> & Partial<Pick<ChatStep, 'passages' | 'documents'>>,
): StepRow[] {
  const at = STEP_ORDER.indexOf(ev.step);
  if (at < 0) return rows;
  return rows.map((r, i) => {
    // Earlier steps are finished — but one that was skipped stays skipped.
    if (i < at) return r.state === 'skipped' ? r : { ...r, state: 'done' };
    if (i === at)
      return { ...r, state: ev.state, passages: ev.passages ?? r.passages, documents: ev.documents ?? r.documents };
    return r;
  });
}

/** What retrieval found for a finished reply, or null when it cited nothing. */
export function citationCounts(
  citations: Pick<Citation, 'documentId'>[],
): { passages: number; documents: number } | null {
  if (citations.length === 0) return null;
  return { passages: citations.length, documents: new Set(citations.map((c) => c.documentId)).size };
}
