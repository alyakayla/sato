<script lang="ts">
  import { deleteWithUndo } from '$lib/undoDelete';
  import { confirmDelete } from '$lib/confirm.svelte';
  import { eventTarget, openArtifactMenu } from '$lib/artifactMenu';
  import { intlLocale, label, t } from '$lib/i18n/index.svelte';
  import type { CalendarEvent, Case, EventInput } from '$lib/types';
  import { EVENT_KINDS } from '$lib/types';
  import { api } from '$lib/api';
  import { artifactsVersion, cases, editRequest, guard, notify, openPane, refreshTree, takeEditRequest } from '$lib/stores.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  let { caseId = null }: { caseId?: string | null } = $props();

  /**
   * A month grid plus an agenda for the selected day.
   *
   * Dates are handled as local calendar days throughout: the backend stores RFC
   * 3339, but every comparison here is on the local `YYYY-MM-DD` key so a
   * deadline at 09:00 lands on the right square rather than shifting a day.
   */

  // Monday-first short names in the current language. 2024-01-01 was a Monday.
  const WEEKDAYS = $derived(
    Array.from({ length: 7 }, (_, i) =>
      new Date(2024, 0, 1 + i).toLocaleDateString(intlLocale(), { weekday: 'short' }),
    ),
  );

  // Event kinds borrow the semantic tokens rather than owning hex values, so a
  // hearing reads as "urgent" in either theme instead of being a fixed red
  // that only works against the old dark surfaces. A user-chosen colour on the
  // event still wins.
  const KIND_COLOR: Record<string, string> = {
    Hearing: 'var(--danger)',
    Filing: 'var(--accent)',
    Meeting: 'var(--ok)',
    Deadline: 'var(--warn)',
    Other: 'var(--text-tertiary)',
  };

  let events = $state<CalendarEvent[]>([]);
  let loading = $state(true);
  let cursor = $state(new Date());
  let selectedDay = $state(ymd(new Date()));
  let view = $state<'month' | 'agenda'>('month');
  let showAll = $state(false);

  // Editing
  let editing = $state<EventInput | null>(null);
  let isNew = $state(false);

  function ymd(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }

  function fromYmd(s: string): Date {
    const [y, m, d] = s.split('-').map(Number);
    return new Date(y, m - 1, d);
  }

  /** Midnight on the first of the displayed month. */
  const monthStart = $derived(new Date(cursor.getFullYear(), cursor.getMonth(), 1));

  /**
   * Six weeks covering the month, Monday-first. A fixed 42 cells keeps the grid
   * from reflowing between months, which is what a calendar should do.
   */
  const cells = $derived.by((): { date: Date; key: string; inMonth: boolean }[] => {
    // getDay() is Sunday-first; shift so Monday is 0.
    const offset = (monthStart.getDay() + 6) % 7;
    const start = new Date(monthStart);
    start.setDate(start.getDate() - offset);
    const out: { date: Date; key: string; inMonth: boolean }[] = [];
    for (let i = 0; i < 42; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      out.push({ date: d, key: ymd(d), inMonth: d.getMonth() === cursor.getMonth() });
    }
    return out;
  });

  const byDay = $derived.by(() => {
    const map = new Map<string, CalendarEvent[]>();
    for (const e of events) {
      const key = ymd(new Date(e.startsAt));
      const list = map.get(key) ?? [];
      list.push(e);
      map.set(key, list);
    }
    return map;
  });

  const dayEvents = $derived(byDay.get(selectedDay) ?? []);

  const monthLabel = $derived(
    cursor.toLocaleDateString(intlLocale(), { month: 'long', year: 'numeric' }),
  );

  const upcoming = $derived(
    events
      .filter((e) => new Date(e.startsAt).getTime() >= startOfToday() && (!caseId || e.caseId === caseId))
      .slice(0, 40),
  );

  function startOfToday(): number {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d.getTime();
  }

  async function load(): Promise<void> {
    loading = true;
    // Six weeks of slack either side covers every visible cell.
    const from = new Date(monthStart);
    from.setDate(from.getDate() - 42);
    const to = new Date(monthStart);
    to.setDate(to.getDate() + 84);
    const data = await guard(t('cal.err.load'), () =>
      api.listEvents({ caseId, from: from.toISOString(), to: to.toISOString() }),
    );
    if (data) events = data;
    loading = false;
    // "Edit event…" from a context menu: open it once this window has loaded it.
    const req = editRequest.value;
    if (req?.kind === 'event') {
      const ev = events.find((x) => x.id === req.id);
      if (ev && takeEditRequest('event', ev.id)) edit(ev);
    }
  }

  $effect(() => {
    void caseId;
    void monthStart.getTime();
    void artifactsVersion.value;
    void load();
  });

  // An edit request for an event elsewhere in time moves the calendar to it;
  // the reload that follows opens the editor.
  $effect(() => {
    const req = editRequest.value;
    if (req?.kind !== 'event' || !req.startsAt) return;
    const d = new Date(req.startsAt);
    if (Number.isNaN(d.getTime())) return;
    if (d.getFullYear() !== cursor.getFullYear() || d.getMonth() !== cursor.getMonth()) {
      cursor = new Date(d.getFullYear(), d.getMonth(), 1);
    }
    selectedDay = ymd(d);
  });

  function shiftMonth(n: number): void {
    cursor = new Date(cursor.getFullYear(), cursor.getMonth() + n, 1);
  }

  function today(): void {
    cursor = new Date();
    selectedDay = ymd(new Date());
  }

  // --- Editing -------------------------------------------------------------

  function toLocalInput(iso: string): string {
    const d = new Date(iso);
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  function blankEvent(dayKey: string): EventInput {
    const d = fromYmd(dayKey);
    d.setHours(9, 0, 0, 0);
    return {
      id: undefined,
      caseId,
      title: '',
      description: null,
      kind: 'Deadline',
      startsAt: d.toISOString(),
      endsAt: null,
      allDay: false,
      location: null,
      color: null,
    };
  }

  function newEvent(dayKey = selectedDay): void {
    editing = blankEvent(dayKey);
    isNew = true;
  }

  function edit(e: CalendarEvent): void {
    editing = { ...e, id: e.id };
    isNew = false;
  }

  async function save(): Promise<void> {
    const draft = editing;
    if (!draft) return;
    if (!draft.title.trim()) {
      notify('warn', t('cal.needsTitle'));
      return;
    }
    const ok = await guard(t('cal.err.save'), () =>
      api.saveEvent({ ...draft, title: draft.title.trim(), caseId: draft.caseId || null }),
    );
    if (ok) {
      editing = null;
      notify('ok', isNew ? t('cal.added') : t('cal.updated'));
      await load();
      await refreshTree();
    }
  }

  async function remove(id: string): Promise<void> {
    const name = editing?.title || t('kind.event');
    if (!(await confirmDelete({ title: t('confirm.event.title'), message: t('confirm.event.body', { name }) }))) return;
    editing = null;
    await deleteWithUndo({
      key: `event:${id}`,
      message: t('cal.deleted'),
      commit: async () => (await guard(t('cal.err.delete'), () => api.deleteEvent(id))) !== null,
      after: async () => {
        await load();
        await refreshTree();
      },
    });
  }

  function time(e: CalendarEvent): string {
    const d = new Date(e.startsAt);
    if (e.allDay) return t('cal.allDay');
    return d.toLocaleTimeString(intlLocale(), { hour: 'numeric', minute: '2-digit' });
  }

  const todayKey = ymd(new Date());
</script>

<div class="cal">
  <div class="toolbar">
    <div class="nav">
      <button class="btn btn-ghost btn-sm" onclick={() => shiftMonth(-1)} title={t('cal.prev')}>‹</button>
      <button class="btn btn-sm" onclick={today}>{t('cal.today')}</button>
      <button class="btn btn-ghost btn-sm" onclick={() => shiftMonth(1)} title={t('cal.next')}>›</button>
      <span class="month">{monthLabel}</span>
    </div>

    <div class="spacer"></div>

    <label class="toggle">
      <input type="checkbox" bind:checked={showAll} />
      <span>{t('common.allCases')}</span>
    </label>

    <div class="seg">
      <button class="s" class:on={view === 'month'} onclick={() => (view = 'month')}>{t('cal.month')}</button>
      <button class="s" class:on={view === 'agenda'} onclick={() => (view = 'agenda')}>{t('cal.agenda')}</button>
    </div>

    <button class="btn btn-primary btn-sm" onclick={() => newEvent()}>{t('create.event')}</button>
  </div>

  <div class="body">
    {#if loading}
      <div class="empty" style="height: 100%"><span class="spinner"></span></div>
    {:else if view === 'month'}
      <div class="grid-wrap">
        <div class="weekdays">
          {#each WEEKDAYS as d (d)}<div class="wd">{d}</div>{/each}
        </div>
        <div class="grid">
          {#each cells as cell (cell.key)}
            {@const list = byDay.get(cell.key) ?? []}
            <button
              class="day"
              class:out={!cell.inMonth}
              class:today={cell.key === todayKey}
              class:sel={cell.key === selectedDay}
              onclick={() => (selectedDay = cell.key)}
              ondblclick={() => newEvent(cell.key)}
            >
              <span class="dnum">{cell.date.getDate()}</span>
              <div class="pills">
                {#each list.slice(0, 3) as e (e.id)}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <span
                    oncontextmenu={(ev) => openArtifactMenu(ev, eventTarget(e))}
                    class="pill tint"
                    data-artifact={`event:${e.id}`}
                    style={`--c:${e.color ?? KIND_COLOR[e.kind] ?? KIND_COLOR.Other}`}
                    title={`${e.title} · ${time(e)}`}
                  >
                    {e.allDay ? '' : time(e).replace(/:\d\d/, '')} {e.title}
                  </span>
                {/each}
                {#if list.length > 3}
                  <span class="more">{t('cal.more', { n: list.length - 3 })}</span>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      </div>

      <aside class="agenda">
        <div class="ag-head">
          <span class="ag-date">{fromYmd(selectedDay).toLocaleDateString(intlLocale(), { weekday: 'long', month: 'long', day: 'numeric' })}</span>
          <button class="btn btn-ghost btn-sm" onclick={() => newEvent()} title={t('cal.addOnDay')}>+</button>
        </div>
        <div class="ag-list">
          {#if dayEvents.length === 0}
            <div class="faint pad">{t('cal.nothingScheduled')}</div>
          {/if}
          {#each dayEvents as e (e.id)}
            {@const c = e.color ?? KIND_COLOR[e.kind] ?? KIND_COLOR.Other}
            <button class="ag-item" data-artifact={`event:${e.id}`} style={`--c:${c}`} onclick={() => edit(e)} oncontextmenu={(ev) => openArtifactMenu(ev, eventTarget(e))}>
              <span class="ag-bar"></span>
              <span class="ag-body">
                <span class="ag-title">{e.title}</span>
                <span class="ag-meta faint">
                  {time(e)}
                  {#if e.location}· {e.location}{/if}
                  {#if e.caseReference}· {e.caseReference}{/if}
                </span>
              </span>
            </button>
          {/each}
        </div>

        {#if upcoming.length > 0}
          <div class="eyebrow ag-sub">{t('cal.nextUp')}</div>
          <div class="ag-list sub">
            {#each upcoming.slice(0, 8) as e (e.id)}
              <button class="ag-item" data-artifact={`event:${e.id}`} onclick={() => edit(e)} oncontextmenu={(ev) => openArtifactMenu(ev, eventTarget(e))}>
                <span class="ag-bar" style={`--c:${e.color ?? KIND_COLOR[e.kind] ?? KIND_COLOR.Other}`}></span>
                <span class="ag-body">
                  <span class="ag-title tiny">{e.title}</span>
                  <span class="ag-meta faint tiny">
                    {new Date(e.startsAt).toLocaleDateString(intlLocale(), { month: 'short', day: 'numeric' })} · {time(e)}
                  </span>
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </aside>
    {:else}
      <div class="agenda-full">
        {#if upcoming.length === 0}
          <div class="empty" style="height: 100%">
            <div class="empty-glyph">◷</div>
            <div class="empty-title">{t('cal.noUpcoming')}</div>
            <div class="empty-hint">{t('cal.noUpcomingHint')}</div>
          </div>
        {:else}
          {#each upcoming as e (e.id)}
            {@const c = e.color ?? KIND_COLOR[e.kind] ?? KIND_COLOR.Other}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="row-item" data-artifact={`event:${e.id}`} style={`--c:${c}`} oncontextmenu={(ev) => openArtifactMenu(ev, eventTarget(e))}>
              <span class="ag-bar"></span>
              <div class="r-when">
                <span class="r-date">{new Date(e.startsAt).toLocaleDateString(intlLocale(), { month: 'short', day: 'numeric' })}</span>
                <span class="r-time faint">{time(e)}</span>
              </div>
              <div class="r-main">
                <div class="r-title">{e.title}</div>
                {#if e.description}<div class="faint tiny r-desc">{e.description}</div>{/if}
              </div>
              <span class="badge badge-muted">{label(e.kind)}</span>
              {#if e.caseId}
                <button class="badge badge-accent as-btn" onclick={() => openPane({ kind: 'case', caseId: e.caseId! })}>
                  {e.caseReference}
                </button>
              {/if}
              {#if e.location}<span class="faint tiny r-loc">{e.location}</span>{/if}
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
</div>

<Modal
  open={editing !== null}
  title={isNew ? t('create.event') : t('cal.editEvent')}
  onclose={() => (editing = null)}
>
  {#if editing}
    <div class="field">
      <label for="ev-title">{t('field.title')}</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="ev-title" bind:value={editing.title} placeholder={t('cal.titlePlaceholder')} />
    </div>

    <div class="field-row">
      <div class="field">
        <label for="ev-kind">{t('tree.kind')}</label>
        <select id="ev-kind" bind:value={editing.kind}>
          {#each EVENT_KINDS as k (k)}<option value={k}>{label(k)}</option>{/each}
        </select>
      </div>
      <div class="field">
        <label for="ev-case">{t('common.case')}</label>
        <select id="ev-case" bind:value={editing.caseId}>
          <option value={null}>{t('cal.noCase')}</option>
          {#each cases.value as c (c.id)}
            <option value={c.id}>{c.reference} · {c.title}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="field-row">
      <div class="field">
        <label for="ev-start">{t('table.col.starts')}</label>
        <input
          id="ev-start"
          type="datetime-local"
          value={toLocalInput(editing.startsAt)}
          oninput={(e) => (editing!.startsAt = new Date((e.currentTarget as HTMLInputElement).value).toISOString())}
        />
      </div>
      <div class="field">
        <label for="ev-loc">{t('field.location')}</label>
        <input id="ev-loc" bind:value={editing.location} placeholder={t('cal.locPlaceholder')} />
      </div>
    </div>

    <label class="check">
      <input type="checkbox" bind:checked={editing.allDay} />
      <span>{t('cal.allDay')}</span>
    </label>

    <div class="field">
      <label for="ev-desc">{t('common.notes')}</label>
      <textarea id="ev-desc" rows="3" bind:value={editing.description}></textarea>
    </div>
  {/if}

  {#snippet footer()}
    {#if editing?.id}
      <button class="btn btn-danger" onclick={() => void remove(editing!.id!)}>{t('common.delete')}</button>
    {/if}
    <div class="spacer"></div>
    <button class="btn" onclick={() => (editing = null)}>{t('common.cancel')}</button>
    <button class="btn btn-primary" onclick={() => void save()}>{t('common.save')}</button>
  {/snippet}
</Modal>

<style>
  .cal {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .month {
    font-size: 13.5px;
    font-weight: 600;
    margin-left: 8px;
    min-width: 150px;
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  /* The view switcher is the global segmented control, unchanged: a sunken
     track and a surface thumb. No local override, because a second treatment
     of the same control is exactly the inconsistency the system exists to
     prevent. */
  .s {
    font-size: 11.5px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 232px;
    overflow: hidden;
  }

  .grid-wrap {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .weekdays {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
  }
  .wd {
    padding: 5px 8px;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-secondary);
  }

  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    grid-template-rows: repeat(6, minmax(0, 1fr));
  }

  .day {
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    cursor: pointer;
    padding: 3px 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: stretch;
    text-align: left;
    overflow: hidden;
    min-height: 0;
  }
  .day:hover {
    background: var(--sunken);
  }
  .day.out {
    background: var(--bg);
  }
  .day.out .dnum {
    color: var(--text-disabled);
  }
  .day.sel {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent-border);
  }
  /* Today is a data point, not a call to action, so the day number is ink on
     surface rather than an accent fill. */
  .day.today .dnum {
    background: var(--ink);
    color: var(--ink-ink);
    border-radius: var(--radius-full);
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    font-weight: 600;
  }

  .dnum {
    font-size: 10.5px;
    color: var(--text-secondary);
    font-weight: 500;
    align-self: flex-start;
    padding: 1px 3px;
  }

  .pills {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-height: 0;
    overflow: hidden;
  }
  .pill {
    font-size: 10px;
    line-height: 1.4;
    padding: 0 4px;
    border-radius: var(--radius-xs);
    border-left: 2px solid var(--c);
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    font-size: 9.5px;
    color: var(--text-tertiary);
    padding-left: 4px;
  }

  .agenda {
    border-left: 1px solid var(--border);
    background: var(--surface-2);
    overflow: auto;
    min-height: 0;
  }
  .ag-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    background: var(--surface-2);
    z-index: 2;
  }
  .ag-date {
    font-size: 11.5px;
    font-weight: 600;
  }
  .ag-sub {
    padding: 9px 10px 4px;
    border-top: 1px solid var(--border);
  }
  .ag-list {
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .pad {
    padding: 10px;
    font-size: 11.5px;
  }

  .ag-item {
    display: flex;
    gap: 7px;
    align-items: stretch;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 5px 6px;
    cursor: pointer;
    text-align: left;
    width: 100%;
  }
  .ag-item:hover {
    background: var(--sunken);
  }
  .ag-bar {
    width: 2px;
    border-radius: var(--radius-full);
    background: var(--c, var(--accent));
    flex-shrink: 0;
  }
  .ag-body {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 1px;
  }
  .ag-title {
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ag-meta {
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tiny {
    font-size: 11px;
  }

  .agenda-full {
    overflow: auto;
    padding: 6px 12px 20px;
    background: var(--surface);
  }
  .row-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
  }
  .row-item:hover {
    background: var(--stripe-hover);
  }
  .r-when {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    width: 62px;
    flex-shrink: 0;
  }
  .r-date {
    font-size: 12px;
    font-weight: 600;
  }
  .r-time {
    font-size: 10px;
  }
  .r-main {
    flex: 1;
    min-width: 0;
  }
  .r-title {
    font-size: 12.5px;
  }
  .r-desc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .r-loc {
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .as-btn {
    cursor: pointer;
    border: 1px solid var(--accent-border);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
    margin: 10px 0;
    cursor: pointer;
  }
</style>
