<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { t } from '$lib/i18n/index.svelte';
  import { notify, openPane } from '$lib/stores.svelte';
  import { checkProvider, providerHealth, type ProblemInfo } from '$lib/providerHealth.svelte';

  /**
   * Explains why the assistant cannot work and exactly how to fix it:
   * install/start Ollama, pull a model, fix a key. Used as the chat banner
   * (`compact`, expandable), inside a failed reply, and in Settings.
   */

  let { info, compact = false }: { info: ProblemInfo; compact?: boolean } = $props();

  let open = $state(false);
  /** The full explanation shows unless this is the compact banner, collapsed. */
  const expanded = $derived(!compact || open);

  const ollama = $derived(info.provider === 'ollama');
  const name = $derived(ollama ? 'Ollama' : info.provider === 'openai' ? 'OpenAI' : info.provider);

  /** Commands that would fix it, in order. */
  const commands = $derived.by((): string[] => {
    if (!ollama) return [];
    if (info.problem === 'model_missing') {
      const models = info.missingModels.length ? info.missingModels : [info.chatModel];
      return models.map((m) => `ollama pull ${m}`);
    }
    if (info.problem === 'unreachable') {
      return [info.chatModel, info.embeddingModel].filter(Boolean).map((m) => `ollama pull ${m}`);
    }
    return [];
  });

  const title = $derived(
    info.problem === 'unreachable'
      ? t('problem.unreachable.title', { provider: name })
      : info.problem === 'model_missing'
        ? t('problem.model_missing.title')
        : info.problem === 'timeout'
          ? t('problem.timeout.title', { provider: name })
          : info.problem === 'unauthorized'
            ? t('problem.unauthorized.title')
            : t('problem.other.title', { provider: name }),
  );

  async function copy(cmd: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(cmd);
      notify('ok', t('problem.copied'));
    } catch {
      notify('warn', t('menu.copyFailed'));
    }
  }

  async function recheck(): Promise<void> {
    const s = await checkProvider();
    if (s?.configured) notify('ok', t('problem.fixed', { provider: name }));
  }
</script>

<div class="problem" class:compact role="status">
  <div class="p-head">
    <span class="p-icon" aria-hidden="true">!</span>
    <span class="p-title">{title}</span>
    {#if compact}
      <button class="link p-toggle" onclick={() => (open = !open)} aria-expanded={expanded}>
        {expanded ? t('problem.hide') : t('problem.howToFix')}
      </button>
    {/if}
  </div>

  {#if expanded}
    <div class="p-body">
      {#if info.problem === 'unreachable' && ollama}
        <ol class="p-steps">
          <li>
            {t('problem.unreachable.install')}
            <button class="link" onclick={() => void openUrl('https://ollama.com/download')}>ollama.com/download</button>
          </li>
          <li>{t('problem.unreachable.start')} <code>ollama serve</code></li>
          {#if commands.length}<li>{t('problem.unreachable.pull')}</li>{/if}
        </ol>
      {:else if info.problem === 'unreachable'}
        <p>{t('problem.unreachable.cloud', { url: info.url })}</p>
      {:else if info.problem === 'model_missing'}
        <p>{t('problem.model_missing.body')}</p>
      {:else if info.problem === 'timeout'}
        <p>{t('problem.timeout.body')}</p>
      {:else if info.problem === 'unauthorized'}
        <p>{t('problem.unauthorized.body')}</p>
      {:else}
        <p>{t('problem.other.body')}</p>
      {/if}

      {#if commands.length}
        <div class="p-cmds">
          {#each commands as cmd (cmd)}
            <button class="p-cmd mono" onclick={() => void copy(cmd)} title={t('problem.copy')}>
              <span class="truncate">{cmd}</span>
              <span class="p-copy" aria-hidden="true">⧉</span>
            </button>
          {/each}
        </div>
      {/if}

      {#if info.url}
        <p class="p-meta">{t('problem.address', { url: info.url })}</p>
      {/if}
      {#if info.detail && info.problem === 'other'}
        <p class="p-meta mono">{info.detail}</p>
      {/if}

      <div class="p-actions">
        <button class="btn btn-sm" onclick={() => void recheck()} disabled={providerHealth.checking}>
          {#if providerHealth.checking}<span class="spinner"></span>{/if}
          {t('problem.recheck')}
        </button>
        <button class="btn btn-ghost btn-sm" onclick={() => openPane({ kind: 'settings' })}>{t('view.settings')}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  /* A warning, not an error page: the warn tint, one hairline, in flow. */
  .problem {
    border: 1px solid var(--warn-border);
    background: var(--warn-soft);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    font-size: 12.5px;
    color: var(--text);
    animation: menu-drop var(--dur-fast) var(--ease);
  }
  .p-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .p-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius-full);
    background: var(--warn);
    color: var(--surface);
    font-size: 10.5px;
    font-weight: 700;
  }
  .p-title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
  }
  .p-toggle {
    flex-shrink: 0;
    font-size: 12px;
  }
  .p-body {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .p-body p {
    margin: 0;
    line-height: 1.5;
    color: var(--text-secondary);
  }
  .p-steps {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .p-cmds {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .p-cmd {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .p-cmd:hover {
    border-color: var(--border-strong);
  }
  .p-copy {
    margin-left: auto;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }
  .p-meta {
    font-size: 11.5px;
    color: var(--text-tertiary) !important;
    overflow-wrap: anywhere;
  }
  .p-actions {
    display: flex;
    gap: 6px;
  }
</style>
