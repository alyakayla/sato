<script lang="ts">
  import ProviderProblem from '$lib/components/ProviderProblem.svelte';
  import { problemFromStatus, setProviderStatus } from '$lib/providerHealth.svelte';
  import { askBeforeDeleting, confirmDelete, setAskBeforeDeleting } from '$lib/confirm.svelte';
  import { LOCALES, getLocale, setLocale, t } from '$lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { notify, errorText, guard, categories, refreshCategories } from '$lib/stores.svelte';
  import { CATEGORY_SWATCHES, DEFAULT_SWATCH } from '$lib/palette';
  import { panelLayout, resetLayout, setLayout } from '$lib/panels.svelte';
  import { RAIL_DOCKS, isDefaultLayout, type Side } from '$lib/layout';

  const SIDES: Side[] = ['left', 'right'];
  import type { ProviderStatus, Settings as SettingsData } from '$lib/types';
  import type { MessageKey } from '$lib/i18n/index.svelte';

  let settings = $state<SettingsData | null>(null);
  let status = $state<ProviderStatus | null>(null);
  let testing = $state(false);
  let saving = $state(false);
  let showKey = $state(false);

  let newCategory = $state({ name: '', practiceArea: '', color: DEFAULT_SWATCH, description: '' });

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      settings = await api.getSettings();
    } catch (e) {
      notify('danger', t('settings.err.load', { error: errorText(e) }));
    }
  }

  async function save() {
    if (!settings) return;
    saving = true;
    const result = await guard(t('settings.err.saveProvider'), () => api.saveProvider(settings!.provider));
    saving = false;
    if (result) {
      status = result;
      setProviderStatus(result);
      notify(result.configured ? 'ok' : 'warn', t('settings.providerSaved'));
    }
  }

  async function test() {
    testing = true;
    const result = await guard(t('settings.err.test'), () => api.testProvider());
    testing = false;
    if (result) {
      status = result;
      setProviderStatus(result);
      notify(result.configured ? 'ok' : 'warn', result.configured ? t('settings.connected') : t('settings.unreachable'));
    }
  }

  async function changeLimit(value: number) {
    if (!settings) return;
    settings.retrievalLimit = value;
    const ok = await guard(t('settings.err.limit'), () => api.setRetrievalLimit(value));
    if (ok !== null) notify('ok', t('settings.limitSet', { n: value }));
  }

  async function addCategory() {
    if (!newCategory.name.trim()) return;
    const ok = await guard(t('settings.err.addCategory'), () =>
      api.saveCategory({
        name: newCategory.name.trim(),
        practiceArea: newCategory.practiceArea.trim(),
        color: newCategory.color,
        description: newCategory.description.trim() || null,
      }),
    );
    if (ok !== null) {
      newCategory = { name: '', practiceArea: '', color: DEFAULT_SWATCH, description: '' };
      await refreshCategories();
      notify('ok', t('settings.categoryAdded'));
    }
  }

  async function removeCategory(id: string, name: string) {
    if (!(await confirmDelete({ title: t('confirm.category.title'), message: t('confirm.category.body', { name }) }))) return;
    const ok = await guard(t('settings.err.deleteCategory'), () => api.deleteCategory(id));
    if (ok !== null) {
      await refreshCategories();
      notify('ok', t('settings.categoryDeleted'));
    }
  }
</script>

<div class="settings">
  {#if !settings}
    <div style="padding:20px;display:flex;flex-direction:column;gap:9px;max-width:620px">
      {#each Array(4) as _, i (i)}
        <div class="skeleton" style="height:60px"></div>
      {/each}
    </div>
  {:else}
    <div class="grid">
      <section class="panel">
        <div class="panel-head"><span class="panel-title">{t('settings.language')}</span></div>
        <div class="panel-body" style="padding:15px">
          <div class="seg" role="radiogroup" aria-label={t('settings.language')}>
            {#each LOCALES as l (l.id)}
              <button
                class:on={getLocale() === l.id}
                role="radio"
                aria-checked={getLocale() === l.id}
                onclick={() => setLocale(l.id)}
              >
                {l.name}
              </button>
            {/each}
          </div>
          <div class="hint">{t('settings.languageHint')}</div>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head"><span class="panel-title">{t('settings.confirmations')}</span></div>
        <div class="panel-body" style="padding:15px">
          <label class="check">
            <input
              type="checkbox"
              checked={askBeforeDeleting()}
              onchange={(e) => setAskBeforeDeleting((e.currentTarget as HTMLInputElement).checked)}
            />
            <span>{t('settings.askBeforeDeleting')}</span>
          </label>
          <div class="hint">{t('settings.askBeforeDeletingHint')}</div>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head"><span class="panel-title">{t('settings.layout')}</span></div>
        <div class="panel-body" style="padding:15px">
          <div class="field">
            <span class="field-label">{t('panel.rail')}</span>
            <div class="seg" role="radiogroup" aria-label={t('panel.rail')}>
              {#each RAIL_DOCKS as dock (dock)}
                <button
                  class:on={panelLayout.value.rail === dock}
                  role="radio"
                  aria-checked={panelLayout.value.rail === dock}
                  onclick={() => setLayout({ ...panelLayout.value, rail: dock })}
                >
                  {t(`edge.${dock}`)}
                </button>
              {/each}
            </div>
          </div>
          <div class="field">
            <span class="field-label">{t('panel.chat')}</span>
            <div class="seg" role="radiogroup" aria-label={t('panel.chat')}>
              {#each SIDES as side (side)}
                <button
                  class:on={panelLayout.value.chat === side}
                  role="radio"
                  aria-checked={panelLayout.value.chat === side}
                  onclick={() => setLayout({ ...panelLayout.value, chat: side })}
                >
                  {t(`edge.${side}`)}
                </button>
              {/each}
            </div>
          </div>
          <div class="hint">{t('settings.layoutHint')}</div>
          <button
            class="btn btn-sm"
            style="margin-top:10px"
            onclick={resetLayout}
            disabled={isDefaultLayout(panelLayout.value)}
          >
            {t('panel.reset')}
          </button>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <span class="panel-title">{t('settings.provider')}</span>
          {#if status}
            <span
              class="badge badge-dot"
              class:badge-ok={status.configured}
              class:badge-danger={!status.configured}
            >
              {status.configured ? t('settings.connected') : t('settings.unreachable')}
            </span>
          {/if}
        </div>
        <div class="panel-body" style="padding:15px">
          <div class="field">
            <label for="p-kind">{t('settings.backend')}</label>
            <select id="p-kind" bind:value={settings.provider.kind}>
              <option value="ollama">{t('settings.ollama')}</option>
              <option value="openai">{t('settings.openai')}</option>
            </select>
          </div>
          <div class="field">
            <label for="p-url">{t('settings.baseUrl')}</label>
            <input
              id="p-url"
              bind:value={settings.provider.baseUrl}
              placeholder={settings.provider.kind === 'ollama'
                ? 'http://localhost:11434'
                : 'https://api.openai.com/v1'}
            />
          </div>
          <div class="field-row">
            <div class="field">
              <label for="p-chat">{t('settings.chatModel')}</label>
              <input id="p-chat" bind:value={settings.provider.chatModel} placeholder="llama3.1" />
            </div>
            <div class="field">
              <label for="p-embed">{t('settings.embedModel')}</label>
              <input
                id="p-embed"
                bind:value={settings.provider.embeddingModel}
                placeholder="nomic-embed-text"
              />
            </div>
          </div>
          {#if settings.provider.kind === 'openai'}
            <div class="field">
              <label for="p-key">{t('settings.apiKey')}</label>
              <div class="key-row">
                <input
                  id="p-key"
                  type={showKey ? 'text' : 'password'}
                  bind:value={settings.provider.apiKey}
                  placeholder="sk-…"
                />
                <button class="btn btn-sm" onclick={() => (showKey = !showKey)}>
                  {showKey ? t('common.hide') : t('common.show')}
                </button>
              </div>
              <div class="hint">
                {t('settings.apiKeyHint')}
              </div>
            </div>
          {/if}

          {#if status && problemFromStatus(status)}
            <ProviderProblem info={problemFromStatus(status)!} />
          {:else if status}
            <div class="alert {status.configured ? 'alert-ok' : 'alert-danger'}">{status.detail}</div>
          {/if}

          <div class="row" style="gap:8px;margin-top:13px">
            <button class="btn" onclick={test} disabled={testing}>
              {#if testing}<span class="spinner"></span>{/if} {t('settings.test')}
            </button>
            <button class="btn btn-primary" onclick={save} disabled={saving}>
              {#if saving}<span class="spinner"></span>{/if} {t('common.save')}
            </button>
          </div>

          {#if settings.provider.kind === 'ollama'}
            <div class="alert alert-warn" style="margin-top:13px">
              {t('settings.pullBefore')}
              <code>ollama pull {settings.provider.chatModel}</code> {t('qa.formulasAnd')}
              <code>ollama pull {settings.provider.embeddingModel}</code>{t('settings.pullAfter')}
            </div>
          {/if}
        </div>
      </section>

      <section class="panel">
        <div class="panel-head"><span class="panel-title">{t('settings.retrieval')}</span></div>
        <div class="panel-body" style="padding:15px">
          <div class="field">
            <label for="r-limit">{t('settings.perQuestion', { n: settings.retrievalLimit })}</label>
            <input
              id="r-limit"
              type="range"
              min="3"
              max="25"
              step="1"
              value={settings.retrievalLimit}
              onchange={(e) => changeLimit(Number((e.currentTarget as HTMLInputElement).value))}
            />
            <div class="hint">
              {t('settings.perQuestionHint')}
            </div>
          </div>
          <dl class="facts">
            <div><dt>{t('settings.chunkSize')}</dt><dd>{t('settings.characters', { n: settings.chunkTargetChars })}</dd></div>
            {#if status?.embeddingDim}
              <div><dt>{t('settings.embedWidth')}</dt><dd>{t('settings.dimensions', { n: status.embeddingDim })}</dd></div>
            {/if}
          </dl>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head"><span class="panel-title">{t('settings.categories')}</span></div>
        <div class="panel-body" style="padding:15px">
          {#if categories.value.length === 0}
            <p class="hint" style="margin-bottom:12px">{t('settings.noCategories')}</p>
          {:else}
            <div class="cat-list">
              {#each categories.value as c (c.id)}
                <div class="cat-row">
                  <span class="dot" style="background:{c.color}"></span>
                  <div class="grow">
                    <div class="row" style="gap:7px">
                      <span style="font-size:12.5px;font-weight:500">{c.name}</span>
                      <span class="badge badge-muted">{t('tree.count.case', { n: c.caseCount })}</span>
                    </div>
                    {#if c.description}
                      <div class="faint" style="font-size:11px">{c.description}</div>
                    {/if}
                  </div>
                  <button class="btn btn-ghost btn-sm danger" onclick={() => removeCategory(c.id, c.name)}>
                    ✕
                  </button>
                </div>
              {/each}
            </div>
          {/if}

          <div class="divider"></div>

          <div class="field">
            <label for="c-name">{t('settings.newCategory')}</label>
            <input id="c-name" bind:value={newCategory.name} placeholder={t('settings.categoryPlaceholder')} />
          </div>
          <div class="field-row">
            <div class="field">
              <label for="c-area">{t('settings.practiceArea')}</label>
              <input id="c-area" bind:value={newCategory.practiceArea} placeholder={t('settings.practicePlaceholder')} />
            </div>
            <div class="field">
              <label for="c-color">{t('settings.colour')}</label>
              <div class="swatches">
                {#each CATEGORY_SWATCHES as sw (sw.value)}
                  <button
                    class="swatch"
                    class:on={newCategory.color === sw.value}
                    style="background:{sw.value}"
                    aria-label={t(`swatch.${sw.name}` as MessageKey)}
                    title={t(`swatch.${sw.name}` as MessageKey)}
                    onclick={() => (newCategory.color = sw.value)}
                  ></button>
                {/each}
              </div>
            </div>
          </div>
          <div class="field">
            <label for="c-desc">{t('common.description')}</label>
            <input id="c-desc" bind:value={newCategory.description} placeholder={t('settings.descPlaceholder')} />
          </div>
          <button class="btn btn-primary btn-sm" onclick={addCategory} disabled={!newCategory.name.trim()}>
            {t('settings.addCategory')}
          </button>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head"><span class="panel-title">{t('settings.how')}</span></div>
        <div class="panel-body" style="padding:15px">
          <ol class="steps">
            <li>{t('settings.how1')}</li>
            <li>{t('settings.how2')}</li>
            <li>{t('settings.how3')}</li>
            <li>{t('settings.how4')}</li>
          </ol>
          <p class="hint" style="margin-top:11px">
            {t('settings.howPrivacy')}
          </p>
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .check {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    cursor: pointer;
  }
  .check input {
    width: 14px;
    height: 14px;
    margin: 0;
    accent-color: var(--ink);
  }

  /* A label for a radio group, styled like the form labels around it. */
  .field-label {
    display: block;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .settings {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px 18px 20px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 14px;
    align-items: start;
    max-width: 1240px;
  }

  .key-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 7px;
  }

  .facts {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 13px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .facts div {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }

  .facts dt {
    color: var(--text-tertiary);
  }

  .facts dd {
    color: var(--text-secondary);
  }

  input[type='range'] {
    padding: 0;
    height: 20px;
    background: transparent;
    border: none;
  }

  .cat-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .cat-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 7px;
    border-radius: var(--radius-sm);
  }

  .cat-row:hover {
    background: var(--sunken);
  }

  .danger:hover {
    color: var(--danger);
  }

  .swatches {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }

  .swatch {
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm);
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
  }

  .swatch.on {
    border-color: var(--text);
  }

  .steps {
    margin-left: 17px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
</style>
