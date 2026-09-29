<script lang="ts">
  import { label, t } from '$lib/i18n/index.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { api } from '$lib/api';
  import { CASE_STATUSES, type Case } from '$lib/types';
  import {
    cases,
    categories,
    createRequest,
    guard,
    notify,
    openPane,
    pane,
    refreshAll,
    refreshTree,
  } from '$lib/stores.svelte';
  import Modal from '$lib/components/Modal.svelte';

  /**
   * The workspace's create actions, in one place.
   *
   * These used to live in per-view sidebars; with a chat-first layout there is
   * no sidebar, so every creation path is reachable from the header.
   *
   * Scope follows the current pane: importing from a case detail files the
   * upload against that case, and from the tree it files as unfiled.
   */

  let menuOpen = $state(false);
  let root: HTMLElement | undefined = $state();

  let showCase = $state(false);
  let caseDraft = $state({ reference: '', title: '', categoryId: '', status: 'Open', description: '' });

  let showSheet = $state(false);
  let sheetName = $state('');

  let importing = $state(false);

  const scopeCaseId = $derived(
    pane.value.kind === 'case' ? pane.value.caseId : null,
  );
  const scopeLabel = $derived(
    scopeCaseId ? cases.value.find((c) => c.id === scopeCaseId)?.reference ?? t('qa.thisCase') : null,
  );

  $effect(() => {
    if (!menuOpen) return;
    const close = (e: MouseEvent) => {
      if (root && !root.contains(e.target as Node)) menuOpen = false;
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') menuOpen = false;
    };
    // Deferred so the click that opened the menu does not immediately close it.
    const t = setTimeout(() => {
      window.addEventListener('mousedown', close);
      window.addEventListener('keydown', esc);
    }, 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener('mousedown', close);
      window.removeEventListener('keydown', esc);
    };
  });

  function closeMenu(): void {
    menuOpen = false;
  }

  // --- Documents -----------------------------------------------------------

  async function importDocuments(): Promise<void> {
    closeMenu();
    const picked = await open({
      multiple: true,
      filters: [
        {
          name: 'Documents',
          extensions: ['pdf', 'docx', 'doc', 'txt', 'md', 'rtf', 'html', 'htm', 'csv', 'json', 'xml'],
        },
      ],
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    if (paths.length === 0) return;

    importing = true;
    const result = await guard(t('qa.importFailed'), () => api.importDocuments(paths, scopeCaseId));
    importing = false;
    if (result) {
      notify('ok', t('qa.imported', { n: result.length }));
      await refreshTree();
    }
  }

  // --- Case ----------------------------------------------------------------

  /** Suggests the next free `case-NN` so references are easy to type. */
  function suggestReference(): string {
    const used = new Set(cases.value.map((c) => c.reference.toLowerCase()));
    for (let i = 1; i < 1000; i++) {
      const candidate = `case-${String(i).padStart(2, '0')}`;
      if (!used.has(candidate)) return candidate;
    }
    return `case-${Date.now().toString(36)}`;
  }

  function startCase(): void {
    closeMenu();
    caseDraft = {
      reference: suggestReference(),
      title: '',
      categoryId: cases.value[0]?.categoryId ?? '',
      status: 'Open',
      description: '',
    };
    showCase = true;
  }

  async function saveCase(): Promise<void> {
    if (!caseDraft.title.trim()) {
      notify('warn', t('qa.needsTitle'));
      return;
    }
    const created = await guard(t('qa.createCaseFailed'), () =>
      api.saveCase({
        reference: caseDraft.reference.trim() || undefined,
        title: caseDraft.title.trim(),
        categoryId: caseDraft.categoryId || null,
        status: caseDraft.status,
        description: caseDraft.description.trim() || null,
        openedAt: new Date().toISOString(),
      }),
    );
    if (created) {
      showCase = false;
      notify('ok', t('qa.caseCreated', { reference: created.reference }));
      await refreshAll();
      openPane({ kind: 'case', caseId: created.id });
    }
  }

  // --- Spreadsheet ---------------------------------------------------------

  function startSheet(): void {
    closeMenu();
    sheetName = '';
    showSheet = true;
  }

  async function saveSheet(): Promise<void> {
    const created = await guard(t('qa.createSheetFailed'), () =>
      api.createSpreadsheet(sheetName.trim() || undefined, scopeCaseId),
    );
    if (created) {
      showSheet = false;
      notify('ok', t('qa.sheetCreated'));
      await refreshTree();
      openPane({ kind: 'sheet', sheetId: created.id });
    }
  }

  $effect(() => {
    const req = createRequest.value;
    if (!req) return;
    createRequest.set(null);
    if (req === 'case') startCase();
    else if (req === 'sheet') startSheet();
    else void importDocuments();
  });

  const items = $derived<{ label: string; icon: string; hint: string; run: () => void }[]>([
    { label: t('create.case'), icon: '◆', hint: t('create.caseHint'), run: startCase },
    { label: t('create.import'), icon: '⤒', hint: t('create.importHint'), run: importDocuments },
    { label: t('create.sheet'), icon: '▦', hint: t('create.sheetHint'), run: startSheet },
    {
      label: t('create.event'),
      icon: '◷',
      hint: t('create.eventHint'),
      run: () => {
        closeMenu();
        openPane({ kind: 'calendar', caseId: scopeCaseId });
      },
    },
  ]);
</script>

<div class="qa" bind:this={root}>
  <button
    class="btn btn-sm add"
    class:busy={importing}
    onclick={() => (menuOpen = !menuOpen)}
    title={t('qa.addHint')}
  >
    {#if importing}<span class="spinner"></span>{:else}＋{/if}
    <span>{t('common.add')}</span>
  </button>

  {#if menuOpen}
    <div class="menu" role="menu">
      {#each items as it (it.label)}
        <button class="item" onclick={it.run} role="menuitem">
          <span class="i">{it.icon}</span>
          <span class="body">
            <span class="l">{it.label}</span>
            <span class="h faint">{it.hint}</span>
          </span>
        </button>
      {/each}
      {#if scopeLabel}
        <div class="scope-note faint">{t('qa.filedUnder', { case: scopeLabel })}</div>
      {/if}
    </div>
  {/if}
</div>

<Modal open={showCase} title={t('create.case')} onclose={() => (showCase = false)}>
  <div class="field-row">
    <div class="field">
      <label for="nc-ref">{t('field.reference')}</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="nc-ref" class="mono" bind:value={caseDraft.reference} placeholder="case-01" />
    </div>
    <div class="field">
      <label for="nc-status">{t('common.status')}</label>
      <select id="nc-status" bind:value={caseDraft.status}>
        {#each CASE_STATUSES as s (s)}<option value={s}>{label(s)}</option>{/each}
      </select>
    </div>
  </div>

  <div class="field">
    <label for="nc-title">{t('field.title')}</label>
    <input id="nc-title" bind:value={caseDraft.title} placeholder={t('qa.titlePlaceholder')} />
  </div>

  <div class="field">
    <label for="nc-cat">{t('common.category')}</label>
    <select id="nc-cat" bind:value={caseDraft.categoryId}>
      <option value="">{t('qa.noCategory')}</option>
      {#each categories.value as c (c.id)}
        <option value={c.id}>{c.name}</option>
      {/each}
    </select>
  </div>

  <div class="field">
    <label for="nc-desc">{t('common.description')}</label>
    <textarea id="nc-desc" rows="3" bind:value={caseDraft.description}></textarea>
  </div>

  <p class="hint">
    {t('qa.refHintBefore')} <code>@</code> {t('qa.refHintAfter')}
  </p>

  {#snippet footer()}
    <button class="btn" onclick={() => (showCase = false)}>{t('common.cancel')}</button>
    <button class="btn btn-primary" onclick={() => void saveCase()}>{t('qa.createCase')}</button>
  {/snippet}
</Modal>

<Modal open={showSheet} title={t('create.sheet')} onclose={() => (showSheet = false)}>
  <div class="field">
    <label for="ns-name">{t('common.name')}</label>
    <!-- svelte-ignore a11y_autofocus -->
    <input id="ns-name" bind:value={sheetName} placeholder={t('qa.sheetPlaceholder')} />
  </div>
  <p class="hint">
    {t('qa.formulasBefore')} <code>=SUM(B2:B10)</code> {t('qa.formulasAnd')} <code>=ROUND(B2*1.15, 2)</code> {t('qa.formulasAfter')}
  </p>

  {#snippet footer()}
    <button class="btn" onclick={() => (showSheet = false)}>{t('common.cancel')}</button>
    <button class="btn btn-primary" onclick={() => void saveSheet()}>{t('common.create')}</button>
  {/snippet}
</Modal>

<style>
  .qa {
    position: relative;
  }

  .add {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }

  /* A floating menu: shadow, no border. */
  .menu {
    animation: menu-drop var(--dur-fast) var(--ease);
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 60;
    width: 240px;
    background: var(--overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: 4px;
    overflow: hidden;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    min-height: 34px;
    padding: 6px 8px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
  }
  .item:hover {
    background: var(--sunken);
  }
  .i {
    font-size: 12px;
    color: var(--text-tertiary);
    width: 14px;
    text-align: center;
    flex-shrink: 0;
  }
  .body {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .l {
    font-size: 12.5px;
    color: var(--text);
  }
  .h {
    font-size: 10.5px;
  }

  .scope-note {
    padding: 6px 8px 4px;
    font-size: 10.5px;
    border-top: 1px solid var(--border);
    margin-top: 4px;
  }
</style>
