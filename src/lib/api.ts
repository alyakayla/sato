import { invoke } from '@tauri-apps/api/core';
import { intlLocale, t } from './i18n/index.svelte';
import type {
  CalendarEvent,
  Case,
  CaseFilter,
  CaseRoster,
  CaseTree,
  Category,
  Chunk,
  Conversation,
  DashboardStats,
  Document,
  DocumentText,
  EventInput,
  IndexProgress,
  Message,
  Person,
  ProviderConfig,
  ProviderStatus,
  SearchHit,
  Settings,
  Spreadsheet,
} from './types';

/**
 * Runs a command that returns nothing and resolves to `true` once it succeeds.
 *
 * Tauri resolves a Rust `()` to `null` — the same value `guard()` uses for
 * "failed". Checks like `if (ok !== null)` therefore treated every successful
 * delete, link or save as a failure: nothing refreshed, no toast, a sheet
 * that stayed "Unsaved". A distinct success value fixes every caller at once.
 */
function run(cmd: string, args?: Record<string, unknown>): Promise<true> {
  return invoke<void>(cmd, args).then(() => true as const);
}

/**
 * Thin typed wrapper over the Rust command surface. Keeping every invoke in one
 * place means a command rename is a single edit here rather than a hunt
 * through components.
 */
export const api = {
  dashboard: () => invoke<DashboardStats>('dashboard'),

  getSettings: () => invoke<Settings>('get_settings'),
  saveProvider: (config: ProviderConfig) =>
    invoke<ProviderStatus>('save_provider', { config }),
  testProvider: () => invoke<ProviderStatus>('test_provider'),
  setRetrievalLimit: (limit: number) => run('set_retrieval_limit', { limit }),

  listCategories: () => invoke<Category[]>('list_categories'),
  saveCategory: (input: {
    id?: string;
    name: string;
    practiceArea: string;
    color: string;
    description?: string | null;
  }) => invoke<Category>('save_category', input),
  deleteCategory: (id: string) => run('delete_category', { id }),

  listCases: (filter?: CaseFilter) => invoke<Case[]>('list_cases', { filter }),
  getCase: (id: string) => invoke<Case>('get_case', { id }),
  saveCase: (input: {
    id?: string;
    reference?: string | null;
    title: string;
    categoryId?: string | null;
    status?: string | null;
    description?: string | null;
    openedAt?: string | null;
  }) => invoke<Case>('save_case', { input }),
  deleteCase: (id: string) => run('delete_case', { id }),

  listPeople: (search?: string) => invoke<Person[]>('list_people', { search }),
  savePerson: (input: {
    id?: string;
    fullName: string;
    role?: string;
    organization?: string | null;
    email?: string | null;
    phone?: string | null;
    notes?: string | null;
  }) => invoke<Person>('save_person', input),
  deletePerson: (id: string) => run('delete_person', { id }),
  caseRoster: (caseId: string) => invoke<CaseRoster>('case_roster', { caseId }),
  linkPerson: (caseId: string, personId: string, roleInCase?: string) =>
    run('link_person', { caseId, personId, roleInCase }),
  unlinkPerson: (caseId: string, personId: string) =>
    run('unlink_person', { caseId, personId }),

  listDocuments: (opts: { caseId?: string | null; search?: string } = {}) =>
    invoke<Document[]>('list_documents', { caseId: opts.caseId ?? null, search: opts.search }),
  getDocument: (id: string) => invoke<Document>('get_document', { id }),
  listChunks: (documentId: string) => invoke<Chunk[]>('list_chunks', { documentId }),
  importDocuments: (paths: string[], caseId?: string | null) =>
    invoke<Document[]>('import_documents', { paths, caseId: caseId ?? null }),
  reindexDocument: (id: string) => run('reindex_document', { id }),
  deleteDocument: (id: string) => run('delete_document', { id }),
  readDocumentText: (id: string) => invoke<DocumentText>('read_document_text', { id }),
  searchDocuments: (query: string, caseId?: string | null, limit?: number) =>
    invoke<SearchHit[]>('search_documents', { query, caseId: caseId ?? null, limit }),

  listConversations: (caseId?: string | null) =>
    invoke<Conversation[]>('list_conversations', { caseId: caseId ?? null }),
  createConversation: (title?: string, caseId?: string | null) =>
    invoke<Conversation>('create_conversation', { title: title ?? null, caseId: caseId ?? null }),
  deleteConversation: (id: string) => run('delete_conversation', { id }),
  listMessages: (conversationId: string) =>
    invoke<Message[]>('list_messages', { conversationId }),
  /**
   * `documentIds` scopes retrieval to the documents the user @-mentioned, so
   * "@case-01/answer.pdf what did it say?" is answered from that file rather
   * than the whole corpus.
   */
  sendMessage: (conversationId: string, content: string, documentIds?: string[]) =>
    invoke<Message>('send_message', {
      conversationId,
      content,
      documentIds: documentIds ?? null,
    }),

  listSpreadsheets: (caseId?: string | null) =>
    invoke<Spreadsheet[]>('list_spreadsheets', { caseId: caseId ?? null }),
  getSpreadsheet: (id: string) => invoke<Spreadsheet>('get_spreadsheet', { id }),
  createSpreadsheet: (name?: string, caseId?: string | null) =>
    invoke<Spreadsheet>('create_spreadsheet', { name: name ?? null, caseId: caseId ?? null }),
  saveSpreadsheet: (input: {
    id: string;
    name?: string | null;
    data: string;
    rows?: number;
    cols?: number;
  }) => run('save_spreadsheet', input),
  deleteSpreadsheet: (id: string) => run('delete_spreadsheet', { id }),

  listEvents: (opts: { caseId?: string | null; from?: string; to?: string } = {}) =>
    invoke<CalendarEvent[]>('list_events', {
      caseId: opts.caseId ?? null,
      from: opts.from ?? null,
      to: opts.to ?? null,
    }),
  saveEvent: (input: EventInput) => invoke<CalendarEvent>('save_event', { input }),
  deleteEvent: (id: string) => run('delete_event', { id }),

  caseTree: () => invoke<CaseTree>('case_tree'),
};

export type IndexProgressEvent = IndexProgress;

/** Human-friendly byte size. */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

// `toLocaleDateString` builds a new `Intl.DateTimeFormat` on every call —
// expensive enough to dominate a table of a thousand dated rows. One
// formatter per locale, reused.
const dateFormats = new Map<string, Intl.DateTimeFormat>();

export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const locale = intlLocale();
  let fmt = dateFormats.get(locale);
  if (!fmt) {
    fmt = new Intl.DateTimeFormat(locale, { year: 'numeric', month: 'short', day: 'numeric' });
    dateFormats.set(locale, fmt);
  }
  return fmt.format(d);
}

export function formatRelative(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const seconds = Math.floor((Date.now() - d.getTime()) / 1000);
  if (seconds < 60) return t('time.justNow');
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t('time.minutesAgo', { n: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t('time.hoursAgo', { n: hours });
  const days = Math.floor(hours / 24);
  if (days < 30) return t('time.daysAgo', { n: days });
  return formatDate(iso);
}
