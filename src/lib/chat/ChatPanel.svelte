<script lang="ts">
  import { deleteWithUndo } from '$lib/undoDelete';
  import { t } from '$lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import type { ChatStep, Conversation, Message } from '$lib/types';
  import { api } from '$lib/api';
  import { STEP_GLYPH, applyStep, initialSteps, type StepRow } from '$lib/steps';
  import {
    cases,
    chatCaseId,
    guard,
    treeNodes,
  } from '$lib/stores.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import PanelGrip from '$lib/components/PanelGrip.svelte';
  import Composer from './Composer.svelte';
  import ProviderProblem from '$lib/components/ProviderProblem.svelte';
  import { checkProvider, problemFromStatus, providerHealth } from '$lib/providerHealth.svelte';

  let conversations = $state<Conversation[]>([]);
  let activeId = $state<string | null>(null);
  let messages = $state<Message[]>([]);
  let loading = $state(false);
  let scroller: HTMLDivElement | undefined = $state();
  let showList = $state(false);
  /** The live checklist for the turn in flight; null when idle. */
  let steps = $state<StepRow[] | null>(null);
  /** The answer so far, as the model streams it; empty when idle. */
  let streamText = $state('');
  // Deltas arrive faster than frames; they are buffered and applied once per
  // frame so a fast model cannot trigger a render per token.
  let deltaBuf = '';
  let deltaFrame = 0;

  /** The in-flight answer, shaped as a message so it renders like one. */
  const streaming = $derived<Message | null>(
    streamText
      ? {
          id: 'streaming',
          conversationId: activeId ?? '',
          role: 'assistant',
          content: streamText,
          citations: [],
          error: null,
          createdAt: new Date().toISOString(),
        }
      : null,
  );

  const currentCaseLabel = $derived.by(() => {
    const id = chatCaseId.value;
    if (!id) return t('common.allCases');
    const c = cases.value.find((x) => x.id === id);
    return c ? `${c.reference} · ${c.title}` : t('common.allCases');
  });

  /**
   * Refreshes the transcript without re-rendering what did not change: rows
   * already on screen keep their object (so their bubble is untouched), and
   * only new or edited messages come from `fresh`.
   */
  function mergeMessages(current: Message[], fresh: Message[]): Message[] {
    const byId = new Map(current.map((m) => [m.id, m]));
    return fresh.map((m) => {
      const old = byId.get(m.id);
      return old && old.content === m.content && old.error === m.error && old.citations.length === m.citations.length
        ? old
        : m;
    });
  }

  /** Pins the transcript to the newest message. */
  function stickToBottom(): void {
    if (!scroller) return;
    requestAnimationFrame(() => {
      if (scroller) scroller.scrollTop = scroller.scrollHeight;
    });
  }

  /** Follows a streaming answer only while the reader is already at the end. */
  function followIfNearBottom(): void {
    if (!scroller) return;
    const gap = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    if (gap < 96) stickToBottom();
  }

  async function loadConversations(): Promise<void> {
    const data = await guard(t('chat.err.loadConversations'), () =>
      api.listConversations(chatCaseId.value),
    );
    if (!data) return;
    conversations = data;
    if (activeId && data.some((c) => c.id === activeId)) return;
    if (data.length > 0) {
      await select(data[0].id);
    } else {
      await newConversation();
    }
  }

  async function select(id: string): Promise<void> {
    activeId = id;
    showList = false;
    loading = true;
    const data = await guard(t('chat.err.loadMessages'), () => api.listMessages(id));
    messages = data ?? [];
    loading = false;
    stickToBottom();
  }

  async function newConversation(): Promise<void> {
    const created = await guard(t('chat.err.start'), () =>
      api.createConversation(undefined, chatCaseId.value),
    );
    if (!created) return;
    conversations = [created, ...conversations];
    await select(created.id);
  }

  async function send(text: string, documentIds: string[]): Promise<void> {
    if (!activeId) {
      await newConversation();
    }
    if (!activeId) return;

    // Optimistic echo so the composer feels instant; the server assigns the id.
    messages = [
      ...messages,
      {
        id: `pending-${Date.now()}`,
        conversationId: activeId,
        role: 'user',
        content: text,
        citations: [],
        error: null,
        createdAt: new Date().toISOString(),
      },
    ];
    loading = true;
    steps = initialSteps();
    streamText = '';
    deltaBuf = '';
    stickToBottom();

    const reply = await guard(t('chat.err.respond'), () =>
      api.sendMessage(activeId!, text, documentIds),
    );
    if (!reply) {
      loading = false;
      steps = null;
      streamText = '';
      return;
    }
    const fresh = await api.listMessages(activeId!);
    messages = mergeMessages(messages, fresh);
    if (reply.error) void checkProvider();
    loading = false;
    steps = null;
    streamText = '';
    stickToBottom();
    // A new conversation's title comes from its first message.
    await loadConversationsQuietly();
  }

  async function loadConversationsQuietly(): Promise<void> {
    const data = await guard(t('chat.err.refresh'), () =>
      api.listConversations(chatCaseId.value),
    );
    if (data) conversations = data;
  }

  async function removeConversation(id: string): Promise<void> {
    await deleteWithUndo({
      key: `conversation:${id}`,
      message: t('chat.deleted'),
      commit: async () => (await guard(t('chat.err.delete'), () => api.deleteConversation(id))) !== null,
      after: async () => {
        conversations = conversations.filter((c) => c.id !== id);
        if (activeId === id) {
          activeId = null;
          await loadConversations();
        }
      },
    });
  }

  // Conversations load from the case-scope effect below, which also runs on
  // mount; loading here as well raced it and could create two empty
  // conversations.
  /** Why the assistant cannot work right now, if it cannot. */
  const problem = $derived(providerHealth.status ? problemFromStatus(providerHealth.status) : null);

  onMount(() => {
    // Find out before the first question whether the provider is usable.
    void checkProvider();
    // Steps and deltas arrive while `sendMessage` is still awaiting; ignore
    // any from a conversation the user has since switched away from.
    const unlistenStep = listen<ChatStep>('chat:step', (e) => {
      if (!steps || e.payload.conversationId !== activeId) return;
      steps = applyStep(steps, e.payload);
      // Retrieval degraded: refresh the diagnosis so the banner can explain.
      if (e.payload.state === 'skipped') void checkProvider();
      followIfNearBottom();
    });
    const unlistenDelta = listen<{ conversationId: string; text: string }>('chat:delta', (e) => {
      if (!loading || e.payload.conversationId !== activeId) return;
      deltaBuf += e.payload.text;
      if (deltaFrame) return;
      deltaFrame = requestAnimationFrame(() => {
        deltaFrame = 0;
        streamText += deltaBuf;
        deltaBuf = '';
        followIfNearBottom();
      });
    });
    return () => {
      if (deltaFrame) cancelAnimationFrame(deltaFrame);
      void unlistenStep.then((fn) => fn());
      void unlistenDelta.then((fn) => fn());
    };
  });

  // Re-scope when the selected case changes so retrieval follows the header.
  $effect(() => {
    void chatCaseId.value;
    activeId = null;
    void loadConversations();
  });
</script>

<div class="chat">
  <header class="head">
    <PanelGrip id="chat" />
    <button class="title" onclick={() => (showList = !showList)} title={t('chat.conversations')}>
      <span class="dot-live"></span>
      <span class="t truncate">{t('chat.assistant')}</span>
    </button>
    <div class="row">
      <select
        class="scope"
        value={chatCaseId.value ?? ''}
        onchange={(e) => {
          chatCaseId.set(e.currentTarget.value || null);
        }}
        title={t('chat.scope')}
      >
        <option value="">{t('common.allCases')}</option>
        {#each cases.value as c (c.id)}
          <option value={c.id}>{c.reference}</option>
        {/each}
      </select>
      <button class="btn btn-sm" onclick={() => void newConversation()} title={t('chat.newConversation')}>
        {t('chat.new')}
      </button>
    </div>
  </header>

  {#if showList}
    <div class="conv-list">
      {#if conversations.length === 0}
        <div class="faint pad">{t('chat.noConversations')}</div>
      {/if}
      {#each conversations as c (c.id)}
        <div class="conv" data-artifact={`conversation:${c.id}`} class:active={c.id === activeId}>
          <button class="conv-main" onclick={() => void select(c.id)}>
            <span class="truncate">{c.title}</span>
            <span class="faint tiny">{t('chat.msgCount', { n: c.messageCount })}</span>
          </button>
          <button
            class="conv-del"
            title={t('chat.deleteConversation')}
            onclick={() => void removeConversation(c.id)}
          >
            ×
          </button>
        </div>
      {/each}
    </div>
  {/if}

  {#if problem}
    <div class="banner">
      <ProviderProblem info={problem} compact />
    </div>
  {/if}

  <div class="thread" bind:this={scroller}>
    {#if messages.length === 0}
      <div class="intro">
        <div class="empty-glyph">§</div>
        <div class="empty-title">{t('chat.emptyTitle')}</div>
        <div class="empty-hint">
          {t('chat.emptyHint1')}
          <code>@</code> {t('chat.emptyHint2')}
          <code>@case-01/answer.pdf</code> {t('chat.emptyHint3')}
        </div>
        <div class="intro-scope faint">
          {loading ? t('common.loading') : t('chat.searching', { scope: currentCaseLabel })}
        </div>
      </div>
    {:else}
      {#each messages as m (m.id)}
        <MessageBubble message={m} nodes={treeNodes.value} />
      {/each}
      {#if loading && streaming}
        <MessageBubble message={streaming} nodes={treeNodes.value} />
      {/if}
      {#if loading && steps}
        <ol class="steps" aria-live="polite" aria-label={t('chat.progress')}>
          {#each steps as s (s.key)}
            <li class="step" class:running={s.state === 'running'} class:done={s.state === 'done'}>
              <span class="step-glyph" aria-hidden="true">
                {#if s.state === 'running'}<span class="spinner"></span>{:else}{STEP_GLYPH[s.state]}{/if}
              </span>
              <span>{s.state === 'skipped' && s.key === 'embed' ? t('step.embedSkipped') : t(`step.${s.key}`)}</span>
              {#if s.passages != null && s.documents != null}
                <span class="step-detail">
                  · {t('step.found', {
                    passages: t('step.passages', { n: s.passages }),
                    documents: t('step.documents', { n: s.documents }),
                  })}
                </span>
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
    {/if}
  </div>

  <Composer nodes={treeNodes.value} disabled={loading} onSend={send} />
</div>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--bg);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--bg);
  }

  .title {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
    min-width: 0;
    color: var(--text);
  }
  .t {
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: 0.01em;
  }
  .dot-live {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--ok);
    box-shadow: 0 0 0 3px var(--ok-soft);
    flex-shrink: 0;
  }

  .scope {
    width: auto;
    max-width: 110px;
    padding: 4px 22px 4px 7px;
    font-size: 11px;
    background-position: right 5px center;
  }

  .conv-list {
    animation: menu-drop var(--dur-fast) var(--ease);
    border-bottom: 1px solid var(--border);
    max-height: 210px;
    overflow-y: auto;
    background: var(--surface);
    flex-shrink: 0;
  }
  .pad {
    padding: 12px;
    font-size: 12px;
  }
  .conv {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
  }
  .conv:last-child {
    border-bottom: none;
  }
  .conv.active {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .conv-main {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
    background: transparent;
    border: none;
    padding: 7px 10px;
    cursor: pointer;
    color: var(--text);
    font-size: 12px;
    text-align: left;
  }
  .tiny {
    font-size: 10px;
    flex-shrink: 0;
  }
  .conv-del {
    background: transparent;
    border: none;
    color: var(--text-tertiary);
    cursor: pointer;
    padding: 4px 8px;
    font-size: 14px;
    line-height: 1;
  }
  .conv-del:hover {
    color: var(--danger);
  }

  .banner {
    padding: 8px 10px 0;
    flex-shrink: 0;
  }

  .thread {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* The agent's live checklist. Muted throughout: progress is background
     information, and the answer that follows is what earns the eye. */
  .steps {
    animation: fade-in var(--dur) var(--ease);
    list-style: none;
    margin: 0;
    padding: 0 0 0 30px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .step {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }
  .step.running,
  .step.done {
    color: var(--text-secondary);
  }
  .step-glyph {
    width: 10px;
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
  }
  .step-detail {
    color: var(--text-tertiary);
  }

  .intro {
    margin: auto 0;
    padding: 0 12px;
  }
  .intro-scope {
    font-size: 11px;
  }
</style>
