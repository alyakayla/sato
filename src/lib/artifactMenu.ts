import type { CalendarEvent, Case, Person, Spreadsheet, TreeNode } from './types';
import { targetOf, type ArtifactTarget } from './artifacts';
import { nodeIndex } from './tree';

export type { ArtifactTarget };
import { api } from './api';
import { t } from './i18n/index.svelte';
import { openContextMenu, type MenuEntry } from './contextMenu.svelte';
import { confirmDelete } from './confirm.svelte';
import {
  artifactsChanged,
  askAbout,
  chatCollapsed,
  editRequest,
  guard,
  notify,
  openPane,
  pane,
  refreshAll,
  refreshPeople,
  toggleChat,
  treeNodes,
} from './stores.svelte';

/**
 * Right-click actions for anything in the case tree — cases, documents,
 * sheets, people and events — wherever it appears: the tree, tables, the case
 * page, chat mentions and citations, search results, the calendar, rosters.
 *
 * Every entry does what the owning view already does; this module only routes
 * to it, so behaviour cannot drift between the menu and the view.
 */

function isShowing(target: ArtifactTarget): boolean {
  const p = pane.value;
  return (
    (target.kind === 'case' && p.kind === 'case' && p.caseId === target.id) ||
    (target.kind === 'document' && p.kind === 'document' && p.documentId === target.id) ||
    (target.kind === 'sheet' && p.kind === 'sheet' && p.sheetId === target.id)
  );
}

/** After a delete: refresh shared data, tell local views, and leave a dead page. */
async function afterDelete(target: ArtifactTarget, message: string): Promise<void> {
  notify('ok', message);
  if (isShowing(target)) openPane({ kind: 'grid' });
  await refreshAll();
  if (target.kind === 'person') await refreshPeople();
  artifactsChanged();
}

function ask(nodeKey: string): void {
  if (chatCollapsed.value) toggleChat();
  askAbout(nodeKey);
}

async function copyMention(nodeKey: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(`@${nodeKey}`);
    notify('ok', t('menu.copied', { mention: `@${nodeKey}` }));
  } catch {
    notify('warn', t('menu.copyFailed'));
  }
}

function open(target: ArtifactTarget): void {
  switch (target.kind) {
    case 'case':
      return openPane({ kind: 'case', caseId: target.id });
    case 'document':
      return openPane({ kind: 'document', documentId: target.id });
    case 'sheet':
      return openPane({ kind: 'sheet', sheetId: target.id });
    case 'person':
      return openPane({ kind: 'people', caseId: target.caseId });
    case 'event':
      return openPane({ kind: 'calendar', caseId: target.caseId });
  }
}

export function artifactMenu(target: ArtifactTarget): MenuEntry[] {
  const out: MenuEntry[] = [];
  const openLabel =
    target.kind === 'event' ? t('menu.openInCalendar') : target.kind === 'person' ? t('menu.viewRoster') : t('common.open');
  out.push({ id: 'open', glyph: '↗', label: openLabel, hint: 'Enter', run: () => open(target) });

  const key = target.nodeKey;
  if (key) out.push({ id: 'ask', glyph: '§', label: t('common.askAssistant'), hint: 'A', run: () => ask(key) });

  out.push('separator');

  switch (target.kind) {
    case 'case':
      out.push(
        {
          id: 'edit',
          glyph: '✎',
          label: t('menu.editCase'),
          run: () => {
            editRequest.set({ kind: 'case', id: target.id });
            openPane({ kind: 'case', caseId: target.id });
          },
        },
        { id: 'table', glyph: '▦', label: t('view.table'), run: () => openPane({ kind: 'table', caseId: target.id }) },
        { id: 'cal', glyph: '◷', label: t('view.calendar'), run: () => openPane({ kind: 'calendar', caseId: target.id }) },
        { id: 'roster', glyph: '●', label: t('view.roster'), run: () => openPane({ kind: 'people', caseId: target.id }) },
      );
      break;
    case 'document':
      out.push({
        id: 'reindex',
        glyph: '↻',
        label: t('doc.reindex'),
        run: async () => {
          const ok = await guard(t('doc.reindex'), () => api.reindexDocument(target.id));
          if (ok !== null) notify('ok', t('doc.requeued'));
        },
      });
      break;
    case 'sheet':
      out.push({
        id: 'rename',
        glyph: '✎',
        label: t('menu.rename'),
        run: () => {
          editRequest.set({ kind: 'sheet', id: target.id });
          openPane({ kind: 'sheet', sheetId: target.id });
        },
      });
      break;
    case 'person':
      out.push({
        id: 'edit',
        glyph: '✎',
        label: t('menu.editPerson'),
        run: () => {
          editRequest.set({ kind: 'person', id: target.id });
          openPane({ kind: 'people', caseId: target.caseId });
        },
      });
      break;
    case 'event':
      out.push({
        id: 'edit',
        glyph: '✎',
        label: t('menu.editEvent'),
        run: () => {
          editRequest.set({ kind: 'event', id: target.id, startsAt: target.startsAt });
          openPane({ kind: 'calendar', caseId: target.caseId });
        },
      });
      break;
  }

  if (key) out.push({ id: 'copy', glyph: '@', label: t('menu.copyMention'), run: () => copyMention(key) });

  out.push('separator');

  switch (target.kind) {
    case 'case':
      out.push({
        id: 'delete',
        glyph: '✕',
        label: t('menu.deleteCase'),
        danger: true,
        run: async () => {
          if (!(await confirmDelete({ title: t('confirm.case.title'), message: t('confirm.case.body', { name: target.label }) }))) return;
          const ok = await guard(t('case.err.delete'), () => api.deleteCase(target.id));
          if (ok !== null) await afterDelete(target, t('case.deleted'));
        },
      });
      break;
    case 'document':
      out.push({
        id: 'delete',
        glyph: '✕',
        label: t('menu.deleteDocument'),
        danger: true,
        run: async () => {
          if (!(await confirmDelete({ title: t('confirm.document.title'), message: t('confirm.document.body', { name: target.label }) }))) return;
          const ok = await guard(t('doc.err.delete'), () => api.deleteDocument(target.id));
          if (ok !== null) await afterDelete(target, t('doc.deleted'));
        },
      });
      break;
    case 'sheet':
      out.push({
        id: 'delete',
        glyph: '✕',
        label: t('menu.deleteSheet'),
        danger: true,
        run: async () => {
          if (!(await confirmDelete({ title: t('confirm.sheet.title'), message: t('confirm.sheet.body', { name: target.label }) }))) return;
          const ok = await guard(t('sheet.err.delete'), () => api.deleteSpreadsheet(target.id));
          if (ok !== null) await afterDelete(target, t('sheet.deleted'));
        },
      });
      break;
    case 'person': {
      const caseId = target.caseId;
      if (caseId) {
        out.push({
          id: 'unlink',
          glyph: '−',
          label: t('menu.removeFromCase'),
          run: async () => {
            const ok = await guard(t('people.err.unlink'), () => api.unlinkPerson(caseId, target.id));
            if (ok !== null) await afterDelete(target, t('menu.removed'));
          },
        });
      }
      out.push({
        id: 'delete',
        glyph: '✕',
        label: t('menu.deletePerson'),
        danger: true,
        run: async () => {
          if (!(await confirmDelete({ title: t('confirm.person.title'), message: t('confirm.person.body', { name: target.label }) }))) return;
          const ok = await guard(t('people.err.delete'), () => api.deletePerson(target.id));
          if (ok !== null) await afterDelete(target, t('people.deleted'));
        },
      });
      break;
    }
    case 'event':
      out.push({
        id: 'delete',
        glyph: '✕',
        label: t('menu.deleteEvent'),
        danger: true,
        run: async () => {
          if (!(await confirmDelete({ title: t('confirm.event.title'), message: t('confirm.event.body', { name: target.label }) }))) return;
          const ok = await guard(t('cal.err.delete'), () => api.deleteEvent(target.id));
          if (ok !== null) await afterDelete(target, t('cal.deleted'));
        },
      });
      break;
  }
  return out;
}

export function openArtifactMenu(at: MouseEvent | HTMLElement, target: ArtifactTarget): void {
  openContextMenu(at, artifactMenu(target), target.label);
}

// --- Targets from plain records ----------------------------------------------
// Views that hold rows rather than tree nodes (tables, calendar, search) build
// targets here; the handle for Ask / Copy mention comes from the live tree.

function keyOf(nodeId: string): string | undefined {
  return nodeIndex(treeNodes.value).byId.get(nodeId)?.nodeKey;
}

export function caseTarget(c: Pick<Case, 'id' | 'title'>): ArtifactTarget {
  return { kind: 'case', id: c.id, label: c.title, nodeKey: keyOf(`case:${c.id}`) };
}

export function documentTarget(id: string, label: string): ArtifactTarget {
  return { kind: 'document', id, label, nodeKey: keyOf(`document:${id}`) };
}

export function sheetTarget(s: Pick<Spreadsheet, 'id' | 'name'>): ArtifactTarget {
  return { kind: 'sheet', id: s.id, label: s.name, nodeKey: keyOf(`sheet:${s.id}`) };
}

/** A person, optionally in the context of one case (enables "Remove from case"). */
export function personTarget(p: Pick<Person, 'id' | 'fullName'>, caseId: string | null): ArtifactTarget {
  const node = treeNodes.value.find((n) =>
    caseId ? n.id === `person:${p.id}:${caseId}` : n.id.startsWith(`person:${p.id}:`),
  );
  return { kind: 'person', id: p.id, label: p.fullName, caseId, nodeKey: node?.nodeKey };
}

export function eventTarget(e: Pick<CalendarEvent, 'id' | 'title' | 'caseId' | 'startsAt'>): ArtifactTarget {
  return {
    kind: 'event',
    id: e.id,
    label: e.title,
    caseId: e.caseId,
    startsAt: e.startsAt,
    nodeKey: keyOf(`event:${e.id}`),
  };
}

/** Convenience for surfaces that hold tree nodes. */
export function openNodeMenu(at: MouseEvent | HTMLElement, node: TreeNode): void {
  const target = targetOf(node);
  if (target) openArtifactMenu(at, target);
  else if (at instanceof MouseEvent) at.preventDefault();
}
