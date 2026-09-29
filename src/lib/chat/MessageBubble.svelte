<script lang="ts">
  import { documentTarget, openArtifactMenu, openNodeMenu } from '$lib/artifactMenu';
  import { t } from '$lib/i18n/index.svelte';
  import type { Message, TreeNode } from '$lib/types';
  import { segmentMessage } from '$lib/tree';
  import { openNode, openPane, notify } from '$lib/stores.svelte';
  import { formatRelative } from '$lib/api';
  import { KIND_META } from '$lib/tree';
  import { citationCounts } from '$lib/steps';
  import ProviderProblem from '$lib/components/ProviderProblem.svelte';
  import { problemFromReply } from '$lib/providerHealth.svelte';

  let { message, nodes = [] }: { message: Message; nodes?: TreeNode[] } = $props();

  const segments = $derived(segmentMessage(message.content, nodes));
  const isUser = $derived(message.role === 'user');
  /** A provider failure, explained; null for ordinary or plain-text errors. */
  const failure = $derived(message.error ? problemFromReply(message.error) : null);
  const summary = $derived.by(() => {
    const c = citationCounts(message.citations);
    return c
      ? t('step.summary', {
          passages: t('step.passages', { n: c.passages }),
          documents: t('step.documents', { n: c.documents }),
        })
      : null;
  });

  function openCitation(documentId: string, page: number | null) {
    // Prefer the tree so a document that has been unfiled still routes the
    // same way; fall back to the raw id when it is no longer in the tree.
    const node = nodes.find((n) => n.documentId === documentId);
    if (node) {
      openNode(node);
    } else {
      openPane({ kind: 'document', documentId });
    }
    if (page != null) notify('ok', t('chat.openedAtPage', { page }));
  }
</script>

<div class="row-msg" class:user={isUser}>
  {#if !isUser}
    <div class="avatar" class:error={!!message.error} title={t('chat.assistant')}>
      {#if message.error}!{:else}<span class="mark">§</span>{/if}
    </div>
  {/if}

  <div class="stack-msg">
    <div class="meta">
      {#if isUser}
        <span class="who">{t('chat.you')}</span>
      {:else}
        <span class="who">Counsel</span>
      {/if}
      <span class="faint" title={message.createdAt}>{formatRelative(message.createdAt)}</span>
    </div>

    <div class="bubble" class:is-error={!!message.error}>
      {#if failure}
        <ProviderProblem info={failure} />
      {:else if message.error}
        <div class="err-line">{message.error}</div>
      {/if}
      {#if message.content && !failure}
        <div class="body">
          {#each segments as seg, i (i)}
            {#if seg.mention}
              {#if seg.node}
                <button
                  class="mention"
                  class:orphan={!seg.node}
                  title={seg.node ? t('chat.openKind', { kind: t(`kind.${seg.node.kind}`).toLowerCase() }) : t('chat.notFound')}
                  onclick={() => seg.node && openNode(seg.node)}
                  oncontextmenu={(e) => seg.node && openNodeMenu(e, seg.node)}
                >
                  <span class="m-kind">{KIND_META[seg.node.kind].icon}</span>
                  {seg.node.label}
                </button>
              {:else}
                <span class="mention missing" title={t('chat.missing')}>{seg.text}</span>
              {/if}
            {:else}
              <span>{seg.text}</span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>

    {#if summary}
      <!-- The finished turn keeps its trail: the steps collapse to one line,
           and the passages it read are one click away. -->
      <details class="trail">
        <summary><span class="trail-glyph" aria-hidden="true">✓</span>{summary}</summary>
      <div class="cites">
        {#each message.citations as c (c.chunkId)}
          <button
            class="cite"
            title={c.snippet}
            onclick={() => openCitation(c.documentId, c.page)}
            oncontextmenu={(e) => openArtifactMenu(e, documentTarget(c.documentId, c.documentName))}
          >
            <span class="c-name truncate">{c.documentName}</span>
            {#if c.caseReference}<span class="c-ref faint">{c.caseReference}</span>{/if}
            {#if c.page != null}<span class="c-page faint">p.{c.page}</span>{/if}
            <span class="c-score" style={`--p:${Math.round(c.score * 100)}%`}></span>
          </button>
        {/each}
      </div>
      </details>
    {/if}
  </div>

  {#if isUser}
    <div class="avatar me" title={t('chat.you')}>{t('chat.you')}</div>
  {/if}
</div>

<style>
  .row-msg {
    animation: menu-rise var(--dur) var(--ease);
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 2px 0;
  }
  .row-msg.user {
    flex-direction: row-reverse;
  }

  .avatar {
    width: 22px;
    height: 22px;
    border-radius: var(--radius-full);
    flex-shrink: 0;
    display: grid;
    place-items: center;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.02em;
    /* Neutral chrome. The assistant is not an accent-coloured badge; the only
       thing here allowed colour is a genuine failure. */
    background: var(--sunken);
    color: var(--text-tertiary);
    border: 1px solid var(--border);
    margin-top: 18px;
  }
  .avatar.error {
    background: var(--danger-soft);
    color: var(--danger);
    border-color: var(--danger-border);
  }
  .avatar.me {
    background: var(--sunken);
    color: var(--text-secondary);
    border-color: var(--border);
    font-size: 8.5px;
  }
  .mark {
    font-size: 11px;
    line-height: 1;
  }

  .stack-msg {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    max-width: 86%;
  }
  .row-msg.user .stack-msg {
    align-items: flex-end;
  }

  .meta {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 10.5px;
    padding: 0 2px;
  }
  .row-msg.user .meta {
    flex-direction: row-reverse;
  }
  .who {
    font-weight: 600;
    color: var(--text-secondary);
    letter-spacing: 0.01em;
  }

  /* The assistant is the surface: flat, bordered, no shadow. The user is ink,
     because the user's own words are the one thing in this pane that should
     read as a statement rather than as a quote. */
  .bubble {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 8px 11px;
    font-size: 13.5px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .row-msg.user .bubble {
    background: var(--ink);
    border-color: var(--ink);
    color: var(--ink-ink);
  }
  .bubble.is-error {
    border-color: var(--danger-border);
  }
  .err-line {
    color: var(--danger);
    font-size: 12.5px;
    margin-bottom: 4px;
  }

  /* A mention is an inline chip, which is one of the legal accent uses.
     Inside the ink user bubble it inverts rather than fighting the fill. */
  .body :global(.mention) {
    font: inherit;
    display: inline;
    padding: 0 3px;
    margin: 0 -1px;
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-xs);
    background: var(--accent-soft);
    color: var(--accent);
    cursor: pointer;
    text-align: left;
  }
  .body :global(.mention:hover) {
    background: var(--accent-border);
  }
  .row-msg.user .body :global(.mention) {
    background: transparent;
    border-color: currentColor;
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .body :global(.m-kind) {
    font-size: 0.9em;
  }
  .body :global(.mention.missing) {
    background: transparent;
    border-color: var(--border);
    color: var(--text-tertiary);
    text-decoration: line-through;
    cursor: default;
  }

  .trail > summary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-tertiary);
    cursor: pointer;
    list-style: none;
    padding: 2px 0;
    border-radius: var(--radius-xs);
  }
  .trail > summary::-webkit-details-marker {
    display: none;
  }
  .trail > summary:hover {
    color: var(--text-secondary);
  }
  .trail[open] > summary {
    margin-bottom: 4px;
  }
  .trail-glyph {
    font-size: 10px;
  }

  .cites {
    animation: menu-drop var(--dur-fast) var(--ease);
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 100%;
  }

  /* A citation is a pointer back into the corpus, so the accent bar is doing
     real work: it is the only thing marking these as navigable. */
  .cite {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 2px solid var(--accent);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: 11.5px;
    text-align: left;
    transition:
      background-color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease);
    max-width: 100%;
  }
  .cite:hover {
    background: var(--sunken);
    border-left-color: var(--accent-hover);
  }

  .c-name {
    font-weight: 500;
    min-width: 0;
  }
  .c-ref,
  .c-page {
    font-size: 11px;
    white-space: nowrap;
  }
  .c-score {
    margin-left: auto;
    height: 3px;
    width: 28px;
    border-radius: var(--radius-full);
    background: var(--border);
    position: relative;
    flex-shrink: 0;
  }
  .c-score::after {
    content: '';
    position: absolute;
    inset: 0 auto 0 0;
    width: var(--p);
    border-radius: var(--radius-full);
    background: var(--accent);
  }
</style>
