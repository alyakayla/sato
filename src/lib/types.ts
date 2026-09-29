// Shapes mirrored from src-tauri/src/models.rs.

export interface Category {
  id: string;
  name: string;
  practiceArea: string;
  color: string;
  description: string | null;
  createdAt: string;
  caseCount: number;
}

export interface Case {
  id: string;
  reference: string;
  title: string;
  categoryId: string | null;
  status: string;
  description: string | null;
  openedAt: string;
  closedAt: string | null;
  createdAt: string;
  updatedAt: string;
  categoryName: string | null;
  categoryColor: string | null;
  documentCount: number;
  peopleCount: number;
}

export interface Person {
  id: string;
  fullName: string;
  role: string;
  organization: string | null;
  email: string | null;
  phone: string | null;
  notes: string | null;
  createdAt: string;
  caseCount: number;
}

export interface CasePerson {
  personId: string;
  caseId: string;
  roleInCase: string;
}

export interface RosterEntry {
  link: CasePerson;
  person: Person;
}

export interface CaseRoster {
  case: Case;
  people: RosterEntry[];
}

export interface Document {
  id: string;
  caseId: string | null;
  fileName: string;
  sourcePath: string;
  storedPath: string;
  mime: string;
  sizeBytes: number;
  checksum: string;
  pageCount: number | null;
  wordCount: number | null;
  indexStatus: 'Pending' | 'Indexing' | 'Ready' | 'Failed';
  indexError: string | null;
  createdAt: string;
  chunkCount: number;
  caseTitle: string | null;
  caseReference: string | null;
}

export interface Chunk {
  id: string;
  documentId: string;
  ordinal: number;
  startChar: number;
  endChar: number;
  page: number | null;
  text: string;
  tokenEstimate: number;
}

export interface SearchHit {
  chunk: Chunk;
  score: number;
  documentName: string;
  caseReference: string | null;
}

export interface Citation {
  chunkId: string;
  documentId: string;
  documentName: string;
  caseReference: string | null;
  page: number | null;
  score: number;
  snippet: string;
}

export interface Message {
  id: string;
  conversationId: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  citations: Citation[];
  error: string | null;
  createdAt: string;
}

/** Progress through one chat turn, emitted by the backend as `chat:step`. */
export interface ChatStep {
  conversationId: string;
  step: 'embed' | 'search' | 'generate';
  /** `skipped`: the embedding failed, so retrieval used keyword matches only. */
  state: 'running' | 'done' | 'skipped';
  problem?: ProviderProblemKind;
  /** Set on the search step: how much retrieval found. */
  passages?: number | null;
  documents?: number | null;
}

export interface Conversation {
  id: string;
  title: string;
  caseId: string | null;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

export interface Spreadsheet {
  id: string;
  name: string;
  caseId: string | null;
  rows: number;
  cols: number;
  data: string;
  createdAt: string;
  updatedAt: string;
  caseTitle: string | null;
  caseReference: string | null;
}

export interface IndexProgress {
  documentId: string;
  fileName: string;
  stage: string;
  chunksDone: number;
  chunksTotal: number;
  status: string;
}

export type ProviderProblemKind = 'unreachable' | 'timeout' | 'model_missing' | 'unauthorized' | 'other';

export interface ProviderStatus {
  name: string;
  configured: boolean;
  detail: string;
  embeddingDim: number | null;
  problem: ProviderProblemKind | null;
  missingModels: string[];
  baseUrl: string;
  chatModel: string;
  embeddingModel: string;
}

export interface ProviderConfig {
  kind: 'ollama' | 'openai';
  baseUrl: string;
  chatModel: string;
  embeddingModel: string;
  apiKey: string;
}

export interface Settings {
  provider: ProviderConfig;
  chunkTargetChars: number;
  retrievalLimit: number;
}

export interface StatusCount {
  status: string;
  count: number;
}

export interface CategoryCount {
  name: string;
  color: string;
  count: number;
}

export interface DashboardStats {
  openCases: number;
  totalCases: number;
  documents: number;
  indexedDocuments: number;
  chunks: number;
  people: number;
  categories: number;
  spreadsheets: number;
  recentDocuments: Document[];
  casesByStatus: StatusCount[];
  categoryBreakdown: CategoryCount[];
}

export interface DocumentText {
  document: Document;
  text: string;
  pages: string[];
}

export interface CaseFilter {
  categoryId?: string | null;
  status?: string | null;
  search?: string | null;
}

export interface CalendarEvent {
  id: string;
  caseId: string | null;
  title: string;
  description: string | null;
  kind: 'Hearing' | 'Filing' | 'Meeting' | 'Deadline' | 'Other';
  startsAt: string;
  endsAt: string | null;
  allDay: boolean;
  location: string | null;
  color: string | null;
  createdAt: string;
  updatedAt: string;
  caseTitle: string | null;
  caseReference: string | null;
}

export type EventInput = Omit<
  CalendarEvent,
  'id' | 'createdAt' | 'updatedAt' | 'caseTitle' | 'caseReference'
> & { id?: string };

export type TreeNodeKind = 'case' | 'document' | 'person' | 'sheet' | 'event';

export interface TreeNode {
  id: string;
  /** Mention handle, e.g. `case-01/document-abc.pdf`. */
  nodeKey: string;
  kind: TreeNodeKind;
  label: string;
  parentId: string | null;
  depth: number;
  ordinal: number;
  status: string | null;
  detail: string | null;
  color: string | null;
  caseId: string | null;
  documentId: string | null;
  sizeBytes: number | null;
  updatedAt: string;
  startsAt: string | null;
}

export interface CaseTree {
  nodes: TreeNode[];
  generatedAt: string;
}

export const CASE_STATUSES = ['Open', 'Pending', 'On Hold', 'Closed'] as const;
export const PERSON_ROLES = [
  'Client',
  'Opposing Party',
  'Witness',
  'Judge',
  'Expert',
  'Other',
] as const;
export const EVENT_KINDS = ['Hearing', 'Filing', 'Meeting', 'Deadline', 'Other'] as const;
