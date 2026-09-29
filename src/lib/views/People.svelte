<script lang="ts">
  import { deleteWithUndo } from '$lib/undoDelete';
  import { confirmDelete } from '$lib/confirm.svelte';
  import { openArtifactMenu, personTarget } from '$lib/artifactMenu';
  import { label, t } from '$lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import Modal from '$lib/components/Modal.svelte';
  import {
    activeCaseId,
    artifactsVersion,
    cases,
    editRequest,
    errorText,
    guard,
    notify,
    people,
    refreshCases,
    refreshPeople,
    scopedCaseId,
    takeEditRequest,
  } from '$lib/stores.svelte';
  import { PERSON_ROLES, type Person, type RosterEntry } from '$lib/types';

  let search = $state('');
  let showEditor = $state(false);
  let editing = $state<Person | null>(null);
  let saving = $state(false);
  let roster = $state<RosterEntry[]>([]);
  let rosterLoading = $state(false);
  let showLink = $state(false);
  let linkCandidate = $state('');
  let linkRole = $state('');

  let form = $state({
    fullName: '',
    role: 'Client',
    organization: '',
    email: '',
    phone: '',
    notes: '',
  });

  onMount(() => {
    void refreshPeople();
  });

  // "Edit person…" from a context menu: open the editor once the directory has
  // the person.
  $effect(() => {
    const req = editRequest.value;
    if (req?.kind !== 'person') return;
    const p = people.value.find((x) => x.id === req.id);
    if (p && takeEditRequest('person', p.id)) openEdit(p);
  });

  // Load the roster of whichever case is active.
  $effect(() => {
    void artifactsVersion.value;
    const caseId = activeCaseId.value;
    if (!caseId) {
      roster = [];
      return;
    }
    rosterLoading = true;
    void (async () => {
      try {
        roster = (await api.caseRoster(caseId)).people;
      } catch (e) {
        notify('warn', t('people.err.roster', { error: errorText(e) }));
      } finally {
        rosterLoading = false;
      }
    })();
  });

  const filtered = $derived(
    people.value.filter((p) => {
      const term = search.trim().toLowerCase();
      if (!term) return true;
      return (
        p.fullName.toLowerCase().includes(term) ||
        (p.organization ?? '').toLowerCase().includes(term) ||
        (p.email ?? '').toLowerCase().includes(term)
      );
    }),
  );

  const activeCase = $derived(cases.value.find((c) => c.id === activeCaseId.value) ?? null);
  const inRoster = $derived(new Set(roster.map((r) => r.person.id)));

  function openCreate() {
    editing = null;
    form = { fullName: '', role: 'Client', organization: '', email: '', phone: '', notes: '' };
    showEditor = true;
  }

  function openEdit(p: Person) {
    editing = p;
    form = {
      fullName: p.fullName,
      role: p.role,
      organization: p.organization ?? '',
      email: p.email ?? '',
      phone: p.phone ?? '',
      notes: p.notes ?? '',
    };
    showEditor = true;
  }

  async function save() {
    if (!form.fullName.trim()) {
      notify('warn', 'A person needs a name');
      return;
    }
    saving = true;
    const result = await guard(t('people.err.save'), () =>
      api.savePerson({
        id: editing?.id,
        fullName: form.fullName.trim(),
        role: form.role,
        organization: form.organization.trim() || null,
        email: form.email.trim() || null,
        phone: form.phone.trim() || null,
        notes: form.notes.trim() || null,
      }),
    );
    saving = false;
    if (result) {
      showEditor = false;
      await refreshPeople();
      notify('ok', editing ? t('people.updated') : t('people.added'));
    }
  }

  async function remove(p: Person) {
    if (!(await confirmDelete({ title: t('confirm.person.title'), message: t('confirm.person.body', { name: p.fullName }) }))) return;
    await deleteWithUndo({
      key: `person:${p.id}`,
      message: t('people.deleted'),
      commit: async () => (await guard(t('people.err.delete'), () => api.deletePerson(p.id))) !== null,
      after: async () => {
        await refreshPeople();
        await refreshCases();
      },
    });
  }

  async function link() {
    if (!activeCaseId.value || !linkCandidate) return;
    const ok = await guard(t('people.err.link'), () =>
      api.linkPerson(activeCaseId.value!, linkCandidate, linkRole.trim() || undefined),
    );
    if (ok !== null) {
      roster = (await api.caseRoster(activeCaseId.value!)).people;
      await refreshCases();
      showLink = false;
      linkCandidate = '';
      linkRole = '';
    }
  }

  async function unlink(p: Person) {
    if (!activeCaseId.value) return;
    const caseId = activeCaseId.value;
    await deleteWithUndo({
      key: `person:${p.id}:${caseId}`,
      message: t('menu.removed'),
      commit: async () => (await guard(t('people.err.unlink'), () => api.unlinkPerson(caseId, p.id))) !== null,
      after: async () => {
        roster = roster.filter((r) => r.person.id !== p.id);
        await refreshCases();
      },
    });
  }
</script>

<div class="people">
  <div class="list">
    <div class="toolbar">
      <input placeholder={t('people.search')} bind:value={search} />
      <button class="btn btn-primary" onclick={openCreate}>{t('people.add')}</button>
    </div>

    <div class="panel grow">
      <div class="panel-head">
        <span class="panel-title">{t('tree.count.person', { n: filtered.length })}</span>
        {#if scopedCaseId.value}
          <span class="faint" style="font-size:11px">{t('people.firmWide')}</span>
        {/if}
      </div>
      <div class="panel-body">
        {#if filtered.length === 0}
          <div class="empty">
            <div class="empty-glyph">⚇</div>
            <div class="empty-title">{t('people.emptyTitle')}</div>
            <div class="empty-hint">
              {t('people.emptyHint')}
            </div>
            <button class="btn btn-primary btn-sm" onclick={openCreate}>{t('people.add')}</button>
          </div>
        {:else}
          <table class="data">
            <thead>
              <tr>
                <th>{t('common.name')}</th>
                <th>{t('common.role')}</th>
                <th>{t('field.organisation')}</th>
                <th>{t('people.contact')}</th>
                <th style="text-align:right">{t('kinds.case')}</th>
                <th style="text-align:right">{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {#each filtered as p (p.id)}
                <tr data-artifact={`person:${p.id}`} ondblclick={() => openEdit(p)} oncontextmenu={(e) => openArtifactMenu(e, personTarget(p, null))}>
                  <td>{p.fullName}</td>
                  <td><span class="badge badge-muted">{label(p.role)}</span></td>
                  <td class="truncate muted" style="max-width:180px">{p.organization ?? '—'}</td>
                  <td class="truncate faint mono" style="max-width:190px">
                    {p.email ?? p.phone ?? '—'}
                  </td>
                  <td style="text-align:right" class="mono">{p.caseCount}</td>
                  <td style="text-align:right">
                    <div class="actions">
                      <button class="btn btn-ghost btn-sm" onclick={() => openEdit(p)}>{t('common.edit')}</button>
                      <button class="btn btn-ghost btn-sm danger" onclick={() => remove(p)}>{t('common.delete')}</button>
                    </div>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>
  </div>

  <div class="roster">
    <div class="panel" style="height:100%">
      <div class="panel-head">
        <span class="panel-title">{t('people.roster')}</span>
        {#if activeCase}
          <button class="btn btn-sm" onclick={() => (showLink = true)}>{t('common.add')}</button>
        {/if}
      </div>
      <div class="panel-body">
        {#if !activeCase}
          <div class="empty">
            <div class="empty-glyph">⚖</div>
            <div class="empty-title">{t('people.noCase')}</div>
            <div class="empty-hint">{t('people.noCaseHint')}</div>
          </div>
        {:else if rosterLoading}
          <div style="padding:14px;display:flex;flex-direction:column;gap:8px">
            {#each Array(4) as _, i (i)}
              <div class="skeleton" style="height:30px"></div>
            {/each}
          </div>
        {:else if roster.length === 0}
          <div class="empty">
            <div class="empty-glyph">⚇</div>
            <div class="empty-title">{t('people.nobody')}</div>
            <div class="empty-hint">{t('people.nobodyHint')}</div>
            <button class="btn btn-primary btn-sm" onclick={() => (showLink = true)}>{t('people.addToCase')}</button>
          </div>
        {:else}
          <div class="roster-list">
            {#each roster as entry (entry.person.id)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="roster-row"
                data-artifact={`person:${entry.person.id}:${activeCaseId.value}`}
                oncontextmenu={(e) => openArtifactMenu(e, personTarget(entry.person, activeCaseId.value))}
              >
                <div class="grow">
                  <div class="row" style="gap:7px">
                    <span class="name">{entry.person.fullName}</span>
                    <span class="badge badge-muted">{label(entry.person.role)}</span>
                  </div>
                  {#if entry.link.roleInCase}
                    <div class="role-in-case">{entry.link.roleInCase}</div>
                  {/if}
                  {#if entry.person.organization}
                    <div class="faint" style="font-size:11px">{entry.person.organization}</div>
                  {/if}
                </div>
                <button class="btn btn-ghost btn-sm danger" onclick={() => unlink(entry.person)}>
                  {t('people.remove')}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<Modal open={showEditor} title={editing ? t('people.edit') : t('people.add')} onclose={() => (showEditor = false)}>
  <div class="field">
    <label for="p-name">{t('people.fullName')}</label>
    <input id="p-name" bind:value={form.fullName} placeholder={t('people.namePlaceholder')} />
  </div>
  <div class="field-row">
    <div class="field">
      <label for="p-role">{t('common.role')}</label>
      <select id="p-role" bind:value={form.role}>
        {#each PERSON_ROLES as r (r)}
          <option value={r}>{label(r)}</option>
        {/each}
      </select>
    </div>
    <div class="field">
      <label for="p-org">{t('field.organisation')}</label>
      <input id="p-org" bind:value={form.organization} placeholder={t('people.orgPlaceholder')} />
    </div>
  </div>
  <div class="field-row">
    <div class="field">
      <label for="p-email">{t('field.email')}</label>
      <input id="p-email" type="email" bind:value={form.email} />
    </div>
    <div class="field">
      <label for="p-phone">{t('field.phone')}</label>
      <input id="p-phone" bind:value={form.phone} />
    </div>
  </div>
  <div class="field">
    <label for="p-notes">{t('common.notes')}</label>
    <textarea id="p-notes" rows="3" bind:value={form.notes}></textarea>
  </div>

  {#snippet footer()}
    <button class="btn" onclick={() => (showEditor = false)}>{t('common.cancel')}</button>
    <button class="btn btn-primary" onclick={save} disabled={saving}>
      {#if saving}<span class="spinner"></span>{/if}
      {editing ? t('common.saveChanges') : t('people.add')}
    </button>
  {/snippet}
</Modal>

<Modal open={showLink} title={t('people.addTo', { case: activeCase?.reference ?? t('qa.thisCase') })} onclose={() => (showLink = false)}>
  <div class="field">
    <label for="link-person">{t('kind.person')}</label>
    <select id="link-person" bind:value={linkCandidate}>
      <option value="">{t('people.choose')}</option>
      {#each people.value.filter((p) => !inRoster.has(p.id)) as p (p.id)}
        <option value={p.id}>{p.fullName} — {p.role}</option>
      {/each}
    </select>
  </div>
  <div class="field">
    <label for="link-role">{t('people.roleOnCase')}</label>
    <input id="link-role" bind:value={linkRole} placeholder={t('people.roleOnCasePlaceholder')} />
    <div class="hint">{t('people.roleOnCaseHint')}</div>
  </div>

  {#snippet footer()}
    <button class="btn" onclick={() => (showLink = false)}>{t('common.cancel')}</button>
    <button class="btn btn-primary" onclick={link} disabled={!linkCandidate}>{t('people.addToCase')}</button>
  {/snippet}
</Modal>

<style>
  .people {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 14px;
    padding: 16px 18px 18px;
    overflow: hidden;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

  .roster {
    min-height: 0;
  }

  .toolbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
  }

  .roster-list {
    padding: 6px;
  }

  .roster-row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
  }

  .roster-row:hover {
    background: var(--sunken);
  }

  .name {
    font-size: 12.5px;
    font-weight: 500;
  }

  .role-in-case {
    font-size: 11px;
    color: var(--text-secondary);
    margin-top: 1px;
  }

  .actions {
    display: flex;
    gap: 1px;
    justify-content: flex-end;
    opacity: 0.5;
  }

  tr:hover .actions {
    opacity: 1;
  }

  .actions .danger:hover,
  .roster-row .danger:hover {
    color: var(--danger);
  }
</style>
