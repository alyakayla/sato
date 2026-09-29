<script lang="ts" module>
  import type { Snippet } from 'svelte';

  /** One property: a plain value, or a snippet for badges, links and the like. */
  export interface PageProp {
    label: string;
    value?: string | number | null;
    mono?: boolean;
    title?: string;
    content?: Snippet;
  }
</script>

<script lang="ts">
  import type { TreeNodeKind } from '$lib/types';
  import { KIND_META } from '$lib/tree';
  import { t } from '$lib/i18n/index.svelte';

  /**
   * The header every "page" shares — a case, a document, a sheet. Glyph, title,
   * one line of properties, and page actions. Nothing is boxed: type and space
   * carry the hierarchy (DESIGN §15).
   *
   * Only a case title is serif (A11); everything else uses the sans `h2` role.
   * Pass `titleContent` to swap the title for an inline edit field that matches
   * it exactly.
   */

  interface Props {
    kind: TreeNodeKind;
    title: string;
    serif?: boolean;
    /** A short handle shown above the title, e.g. the case reference. */
    eyebrow?: string;
    eyebrowColor?: string | null;
    properties?: PageProp[];
    titleContent?: Snippet;
    actions?: Snippet;
  }

  let {
    kind,
    title,
    serif = false,
    eyebrow,
    eyebrowColor = null,
    properties = [],
    titleContent,
    actions,
  }: Props = $props();
</script>

<header class="page-head">
  <div class="ph-main">
    <div class="ph-eyebrow">
      <span class="ph-glyph" aria-hidden="true">{KIND_META[kind].icon}</span>
      {#if eyebrow}
        <span class="mono ph-handle" style:color={eyebrowColor ?? undefined}>{eyebrow}</span>
      {:else}
        <span>{t(`kind.${kind}`)}</span>
      {/if}
    </div>

    {#if titleContent}
      {@render titleContent()}
    {:else if serif}
      <h1 class="ph-title">{title}</h1>
    {:else}
      <h2 class="ph-title truncate" {title}>{title}</h2>
    {/if}

    {#if properties.length}
      <dl class="props">
        {#each properties as p (p.label)}
          <div class="prop">
            <dt class="prop-label">{p.label}</dt>
            <dd class="prop-value" class:mono={p.mono} title={p.title}>
              {#if p.content}{@render p.content()}{:else}{p.value ?? '—'}{/if}
            </dd>
          </div>
        {/each}
      </dl>
    {/if}
  </div>

  {#if actions}
    <div class="ph-actions">{@render actions()}</div>
  {/if}
</header>

<style>
  .page-head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    flex-wrap: wrap;
  }
  .ph-main {
    flex: 1 1 320px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ph-eyebrow {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .ph-glyph {
    font-size: 10px;
  }
  .ph-handle {
    font-weight: 600;
    color: var(--text-secondary);
  }

  .ph-title {
    margin: 0;
    min-width: 0;
  }

  .ph-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
