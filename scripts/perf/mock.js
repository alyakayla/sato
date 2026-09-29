// Injected before the app loads: a fake Tauri bridge backed by in-memory data
// shaped like a busy real install.
(() => {
  const CASES = window.__MOCK_CASES ?? 300;
  const DOCS_PER_CASE = window.__MOCK_DOCS ?? 30;
  const MESSAGES = window.__MOCK_MESSAGES ?? 80;
  const PEOPLE = window.__MOCK_PEOPLE ?? 2;
  const now = new Date().toISOString();

  const nodes = [];
  const cases = [];
  for (let c = 0; c < CASES; c++) {
    const cid = `c${c}`;
    const ref = `case-${String(c).padStart(3, '0')}`;
    cases.push({ id: cid, reference: ref, title: `Matter ${c} v. Respondent`, categoryId: null, categoryName: null, categoryColor: null, status: 'Open', description: null, openedAt: now, closedAt: null, createdAt: now, updatedAt: now, documentCount: DOCS_PER_CASE, peopleCount: 2 });
    nodes.push({ id: `case:${cid}`, nodeKey: ref, kind: 'case', label: ref, parentId: null, depth: 0, ordinal: nodes.length, status: 'Open', detail: `Matter ${c} v. Respondent`, color: null, caseId: cid, documentId: null, sizeBytes: null, updatedAt: now, startsAt: null });
    for (let d = 0; d < DOCS_PER_CASE; d++) {
      const did = `d${c}_${d}`;
      nodes.push({ id: `document:${did}`, nodeKey: `${ref}/document-${d}.pdf`, kind: 'document', label: `document-${d}.pdf`, parentId: `case:${cid}`, depth: 1, ordinal: nodes.length, status: 'Ready', detail: null, color: null, caseId: cid, documentId: did, sizeBytes: 120000, updatedAt: now, startsAt: null });
    }
    for (let p = 0; p < PEOPLE; p++) {
      nodes.push({ id: `person:p${c}_${p}:${cid}`, nodeKey: `${ref}/person-${p}`, kind: 'person', label: `Person ${p}`, parentId: `case:${cid}`, depth: 1, ordinal: nodes.length, status: null, detail: 'Client', color: null, caseId: cid, documentId: null, sizeBytes: null, updatedAt: now, startsAt: null });
    }
  }

  const para = (i) =>
    `According to @case-${String(i % CASES).padStart(3, '0')}/document-${i % DOCS_PER_CASE}.pdf the lessee must give ` +
    `sixty days' written notice before termination [1]. The clause in @case-${String((i + 7) % CASES).padStart(3, '0')}/document-2.pdf ` +
    `is broader and covers assignment as well [2]. `.repeat(3);
  const citations = (i) =>
    Array.from({ length: 8 }, (_, k) => ({ chunkId: `d${i % CASES}_${k}:${k}`, documentId: `d${i % CASES}_${k}`, documentName: `document-${k}.pdf`, caseReference: `case-${String(i % CASES).padStart(3, '0')}`, page: k + 1, score: 1 - k * 0.08, snippet: 'The lessee shall give sixty days written notice before termination of this agreement. '.repeat(3) }));

  let messages = [];
  for (let i = 0; i < MESSAGES; i++) {
    const user = i % 2 === 0;
    messages.push({ id: `m${i}`, conversationId: 'conv1', role: user ? 'user' : 'assistant', content: user ? `What does @case-${String(i % CASES).padStart(3, '0')}/document-1.pdf say about notice?` : para(i), citations: user ? [] : citations(i), error: null, createdAt: now });
  }
  const conversations = [{ id: 'conv1', title: 'Notice periods', caseId: null, createdAt: now, updatedAt: now, messageCount: MESSAGES }];

  // --- event bridge ---
  const callbacks = new Map();
  const listeners = new Map(); // event -> [{id, cb}]
  let nextId = 1;
  window.__mockEmit = (event, payload) => {
    for (const l of listeners.get(event) ?? []) l.cb({ event, id: l.id, payload });
  };

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  // `window.__MOCK_PROVIDER_DOWN = true` simulates Ollama not running.
  const down = () => window.__MOCK_PROVIDER_DOWN === true;
  const providerStatus = () => ({
    name: 'ollama', configured: !down(), detail: down() ? 'error sending request' : 'connected', embeddingDim: 768,
    problem: down() ? 'unreachable' : null, missingModels: [], baseUrl: 'http://localhost:11434', chatModel: 'llama3.1', embeddingModel: 'nomic-embed-text',
  });

  async function sendMessage({ conversationId, content }) {
    const emit = (step, state, extra = {}) => window.__mockEmit('chat:step', { conversationId, step, state, ...extra });
    if (down()) {
      emit('embed', 'running');
      await sleep(30);
      emit('embed', 'skipped', { problem: 'unreachable' });
      emit('search', 'done', { passages: 3, documents: 2 });
      emit('generate', 'running');
      await sleep(60);
      const error = JSON.stringify({ stage: 'chat', problem: 'unreachable', provider: 'ollama', url: 'http://localhost:11434', model: 'llama3.1', embeddingModel: 'nomic-embed-text', detail: 'error sending request' });
      const a = { id: `f${messages.length}`, conversationId, role: 'assistant', content: 'The chat model could not be used.', citations: [], error, createdAt: now };
      messages = [...messages, { id: `u${messages.length}`, conversationId, role: 'user', content, citations: [], error: null, createdAt: now }, a];
      return a;
    }
    emit('embed', 'running');
    await sleep(80);
    emit('embed', 'done');
    emit('search', 'done', { passages: 8, documents: 5 });
    emit('generate', 'running');
    const answer = para(3).repeat(3);
    const pieces = answer.match(/.{1,24}/g);
    for (const p of pieces) {
      await sleep(40);
      window.__mockEmit('chat:delta', { conversationId, text: p });
    }
    const u = { id: `u${messages.length}`, conversationId, role: 'user', content, citations: [], error: null, createdAt: now };
    const a = { id: `a${messages.length}`, conversationId, role: 'assistant', content: answer, citations: citations(3), error: null, createdAt: now };
    messages = [...messages, u, a];
    return a;
  }

  const handlers = {
    list_cases: () => cases,
    list_documents: () => nodes.filter((n) => n.kind === 'document').map((n) => ({ id: n.documentId, caseId: n.caseId, fileName: n.label, sourcePath: 'x', storedPath: 'x', mime: 'application/pdf', sizeBytes: 120000, checksum: n.documentId, pageCount: 12, wordCount: 4000, indexStatus: 'Ready', indexError: null, createdAt: now, chunkCount: 20, caseTitle: 'Matter', caseReference: n.nodeKey.split('/')[0] })),
    list_categories: () => [],
    list_people: () => [],
    case_tree: () => ({ nodes, generatedAt: now }),
    list_conversations: () => conversations,
    list_messages: () => messages,
    create_conversation: () => conversations[0],
    send_message: sendMessage,
    test_provider: providerStatus,
    get_settings: () => ({ provider: { kind: 'ollama', baseUrl: 'http://localhost:11434', chatModel: 'llama3.1', embeddingModel: 'nomic-embed-text', apiKey: '' }, retrievalLimit: 8, chunkTargetChars: 1200 }),
    'plugin:event|listen': ({ event, handler }) => {
      const cb = callbacks.get(handler);
      const id = nextId++;
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push({ id, cb });
      return id;
    },
    'plugin:event|unlisten': () => null,
  };

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback: (cb) => {
      const id = nextId++;
      callbacks.set(id, cb);
      return id;
    },
    unregisterCallback: (id) => callbacks.delete(id),
    convertFileSrc: (p) => p,
    invoke: async (cmd, args) => {
      const h = handlers[cmd];
      const v = h ? await h(args ?? {}) : cmd.startsWith('list_') ? [] : null;
      // Structured-clone like real IPC, so the app cannot share our objects.
      return v == null ? v : JSON.parse(JSON.stringify(v));
    },
  };
})();
