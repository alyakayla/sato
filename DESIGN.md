# satō — Design Specification

A design system for a legal case-management desktop app. Tauri + Svelte.

This document is executable. Every rule here is either a token, a number, or a
pass/fail check. If something is not specified, it is not permitted.

---

## 1. What this app is

satō holds matters, documents, deadlines, people, and a chat that can answer
questions about them. It is a **tool a professional stares at for hours**, not a
product page or a consumer app.

That single fact settles most arguments:

- Density beats spaciousness. Information per screen is a feature.
- The chrome recedes. Content is the interface.
- Nothing bounces, glows, or celebrates. It should feel like good stationery.
- Warmth, not playfulness. Confident, quiet, legible.

## 2. References

We take from two products and explicitly reject parts of both.

**Notion** — we take:
- Warm low-chroma neutrals derived from a single hue (~35°), never blue-grey.
- **Ink-coloured primary actions.** Notion's primary button is near-black with
  white text. Colour is not used for chrome.
- Colour reserved for *user content* — page icons, labels, mentions — not for
  affordances.
- Borders so faint they are almost subliminal; grouping is done with whitespace
  and subtle surface tints.
- Typography carrying the hierarchy, with tight confident spacing.

**Manus** — we take:
- A warm off-white canvas rather than pure white, so white cards genuinely lift.
- Soft, diffuse, warm-tinted shadows with no hard edges.
- Comfortable hit targets and confident internal padding.

**We reject** from both: heavy multi-layer elevation stacks, glassmorphism,
gradient meshes, and any "AI sparkle" iconography.

## 3. Principles

1. **Ink, not colour, for actions.** A primary button is `--ink`. If every CTA
   is a saturated fill, nothing reads as primary.
2. **Colour means data, never chrome.** The only legal uses of a chromatic hue
   are: user-assigned category swatches, status/kind semantics, links and
   mentions. Everything else is neutral or ink.
3. **Warm, single-hue neutrals.** Every grey has red > green > blue. Every
   shadow is brown-tinted. This is the difference between "chosen" and "default".
4. **Type does the work.** Hierarchy comes from size, weight, and space — not
   from colour or from boxes.
5. **One edge per surface.** A card has a border *or* a shadow. Both is a
   double outline and reads as unedited CSS.
6. **Measure, don't eyeball.** Contrast ratios and spacing are solved, not
   guessed. See §12.

## 4. Anti-patterns

Each of these is currently in the codebase or is a common default. They are
banned. This section is the most important part of the document.

| # | Anti-pattern | Why it reads as "AI-generated" | Instead |
|---|---|---|---|
| A1 | **Saturated primary button** | A coloured fill on every CTA is the loudest tell there is. | `--ink` fill, white text. |
| A2 | **Pill buttons** (`border-radius: 999px` on every button) | Stadium buttons on every control is marketing-page styling. | `--radius-sm` (8px). Pills only for tags and the segmented-control thumb. |
| A3 | **Accent-coloured focus ring** | If accent means "focused" it stops meaning "actionable". | Ink focus ring. |
| A4 | **Border *and* shadow on a card** | A double edge is the signature of untouched default CSS. | Border for flat surfaces, shadow for floating ones. |
| A5 | **Loud outline on secondary buttons** | A 3:1 border on a quiet button makes it shout. | `--border` outline; the fill defines the button. Only *inputs* need a 3:1 boundary. |
| A6 | **A dot on every badge** | Four statuses × coloured dot = traffic-light soup. | Dot only where the status is genuinely stateful. |
| A7 | **Gradient sweep skeleton** | The `linear-gradient(90deg, …)` shimmer is a stock loading cliché. | Flat `--sunken` block, no animation. |
| A8 | **Accent-coloured spinner** | Draws the eye to a background task. | The muted square spinner (§15.11). |
| A9 | **`backdrop-filter` blur on dialogs** | Heavy, dated, and it makes text behind it unreadable. | Flat warm scrim. |
| A10 | **Big rounded icon tile in empty states** | The 52px rounded square + centred text is a template. | Plain glyph, muted, no tile. |
| A11 | **Serif used decoratively** | A display face on UI chrome is a costume. | Serif only for the matter/case title. |
| A12 | **Saturated semantic colours** | If `--ok` is vivid, it competes with the accent. | Semantic hues are desaturated relative to the accent. |
| A13 | **Uppercase micro-labels everywhere** | Tracked-out caps is a dashboard tic. | Sentence case. Caps only for `.eyebrow` section dividers. |
| A14 | **Centre-aligned hero inside a tool** | Tools are not landing pages. | Left-align, align to the content grid. |

## 5. Colour

### 5.1 Neutral ramp — light

Single warm hue (~35°), chroma rising with depth. All values verified — see §12.

| Token | Value | Use |
|---|---|---|
| `--bg` | `#F4F2EF` | App canvas. Warm paper, never `#FFF`. |
| `--surface` | `#FFFFFF` | Cards, panels, rows. Lifts *forward* off the canvas. |
| `--surface-2` | `#FAF8F6` | Table headers, subtle insets. |
| `--sunken` | `#EDEAE5` | Wells, tracks, inset areas. |
| `--overlay` | `#FFFFFF` | Menus, dialogs, popovers. |
| `--input` | `#FFFFFF` | Field fill. |

### 5.2 Neutral ramp — dark

A warm near-black, not cold slate.

| Token | Value |
|---|---|
| `--bg` | `#191714` |
| `--surface` | `#211E1A` |
| `--surface-2` | `#1D1B17` |
| `--sunken` | `#131210` |
| `--overlay` | `#26231F` |
| `--input` | `#1B1815` |

### 5.3 Text

Four steps. Every one clears 4.5:1 on every surface it can land on.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--text` | `#1F1C18` | `#F2EEE8` | Primary. |
| `--text-secondary` | `#57514A` | `#BDB6AB` | Body, muted labels. |
| `--text-tertiary` | `#6F685E` | `#9B9387` | Hints, placeholders, captions. |
| `--text-disabled` | `#989187` | `#6E675D` | Decorative only. **Never load-bearing text.** |

Naming is deliberate: `secondary` and `tertiary` describe a hierarchy, whereas
`muted`/`faint` describe an intensity and invite use on anything.

### 5.4 Accent — meaning is narrow

| Token | Light | Dark |
|---|---|---|
| `--accent` | `#B04A26` | `#E08A5A` |
| `--accent-hover` | `#9A3E1E` | `#EE9B6E` |
| `--accent-soft` | `rgba(176,74,38,.09)` | `rgba(224,138,90,.13)` |
| `--accent-border` | `rgba(176,74,38,.28)` | `rgba(224,138,90,.32)` |

**Legal uses of accent, exhaustively:** links, `@mentions`, the selected row in a
tree/table, the active segment of the view switcher, and the inline `code` chip.

**Illegal uses:** primary buttons, focus rings, headings, icons in navigation,
the app mark, decorative backgrounds.

The accent is a warm terracotta held at higher chroma than the neutrals, so it
separates by *saturation* rather than by hue. That is what keeps it from
dissolving into the warm greys.

### 5.5 Ink — the action colour

| Token | Light | Dark |
|---|---|---|
| `--ink` | `#211E1A` | `#F2EEE8` |
| `--ink-hover` | `#100E0C` | `#FFFFFF` |
| `--ink-ink` | `#FFFFFF` | `#191714` |

In dark mode the primary button inverts: a light button on a dark canvas. The
*role* is constant even though the colour flips.

### 5.6 Semantics

Desaturated relative to the accent (A12) so the accent stays the loudest thing
on screen.

| Role | Light | Dark | Soft fill (light) |
|---|---|---|---|
| `--ok` | `#2F6F4E` | `#5CB98A` | `rgba(47,111,78,.10)` |
| `--warn` | `#8A5B08` | `#D9A441` | `rgba(138,91,8,.11)` |
| `--danger` | `#A6362A` | `#E8756A` | `rgba(166,54,42,.10)` |

### 5.7 Borders

| Token | Light | Dark | Rule |
|---|---|---|---|
| `--border` | `#E5E1DB` | `#33302A` | Decorative dividers. **Exempt** from contrast minimums. |
| `--border-control` | `#878176` | `#776F63` | The only cue a field is a field. **Holds 3:1.** |
| `--border-strong` | `#6E675C` | `#8A8175` | Control hover. |

## 6. Typography

**Families** (all bundled locally, never fetched — the app must render identically
offline):

- `--font` — Plus Jakarta Sans Variable. All UI.
- `--font-display` — Newsreader Variable. The matter title, and nothing else.
- `--font-mono` — JetBrains Mono Variable. Case handles, formulas, code.

**Scale.** Sizes are a ladder, not a set of arbitrary values. Line-heights
tighten as size grows.

| Role | Size | Line-height | Weight | Family | Tracking |
|---|---|---|---|---|---|
| Display (case title) | 25px | 1.2 | 500 | serif | −0.02em |
| `h1` | 21px | 1.25 | 600 | sans | −0.015em |
| `h2` | 16px | 1.35 | 600 | sans | −0.01em |
| `h3` | 14px | 1.4 | 600 | sans | −0.005em |
| Body | 13.5px | 1.55 | 400 | sans | 0 |
| Secondary | 12.5px | 1.5 | 400 | sans | 0 |
| Caption | 11.5px | 1.45 | 400 | sans | 0 |
| Eyebrow | 10.5px | 1.3 | 700 | sans | +0.08em, uppercase |
| Mono | 12px | 1.6 | 400 | mono | 0 |

**Rules**
- Base body size is 13.5px. Do not drop below 11.5px for real content.
- Tracking is `0` for all body text. Negative only above 14px.
- `font-variant-numeric: tabular-nums` is global — this is a data app and
  columns of numbers must align.
- **Serif is for voice only** (A11). An inline edit field for the case title
  must match family, size, and weight exactly, so the title does not reflow when
  clicked.
- Measure: prose is capped at `70ch`. `.empty-hint` at `38ch`.

## 7. Spacing

4px base grid. The scale is a fixed set; arbitrary values are a smell.

`2 · 4 · 6 · 8 · 12 · 16 · 20 · 24 · 32 · 40 · 48 · 64`

- Control heights: `--control-sm` 28px, `--control-md` 32px, `--control-lg` 36px.
- Absolute hit-target floor: **30px**. This is a mouse-driven desktop tool.
- Density: the tree row and table row are 30px. Virtualised lists multiply this
  constant in JS — changing one without the other silently breaks layout.

## 8. Radius

Not everything is round. Roundness is semantic: it says "this is a tag" or
"this is a control". Applying it uniformly says nothing.

| Token | Value | Use |
|---|---|---|
| `--radius-xs` | 5px | Chips, tiny tags, inline code. |
| `--radius-sm` | 8px | Inputs, buttons, tree rows. **The default.** |
| `--radius-md` | 10px | Cards, dropdown items. |
| `--radius-lg` | 14px | Panels, dialogs, popovers. |
| `--radius-xl` | 20px | Reserved. |
| `--radius-full` | 999px | Tags, avatars, segmented-control thumb. **Only these.** |

## 9. Elevation

Brown-tinted, diffuse, no hard edges. Two shadows layered (contact + ambient) at
every level — a single hard shadow looks pasted on.

| Token | Light | Dark |
|---|---|---|
| `--shadow-xs` | `0 1px 2px rgba(53,38,22,.05)` | `0 1px 2px rgba(0,0,0,.30)` |
| `--shadow-sm` | `0 1px 2px rgba(53,38,22,.05), 0 2px 6px rgba(53,38,22,.04)` | `0 1px 2px rgba(0,0,0,.34), 0 2px 6px rgba(0,0,0,.26)` |
| `--shadow-md` | `0 2px 4px rgba(53,38,22,.05), 0 8px 20px rgba(53,38,22,.07)` | `0 2px 4px rgba(0,0,0,.30), 0 10px 24px rgba(0,0,0,.40)` |
| `--shadow-lg` | `0 10px 24px rgba(53,38,22,.09), 0 28px 64px rgba(53,38,22,.14)` | `0 10px 20px rgba(0,0,0,.36), 0 28px 64px rgba(0,0,0,.50)` |

**One edge per surface (Principle 5).**
- `.card` — border only, no shadow. It sits in a layout.
- `.panel` — border only.
- `.overlay` / `dialog` / popover / toast — shadow, no border. They float.
- Nothing gets both.

## 10. Components

### Buttons
- Default: `--surface` fill, `--border` outline (A5), `--radius-sm`, `--shadow-xs`.
- Primary: `--ink` fill, `--ink-ink` text, no outline.
- Ghost: transparent, no outline, `--sunken` on hover.
- Danger: transparent, `--danger` text, `--danger-soft` on hover.
- Active: `translateY(1px)`, no shadow.
- Disabled: `opacity: .45`, no shadow, `cursor: not-allowed`.

### Inputs
- `--input` fill, `--border-control` 1px, `--radius-sm`, 32px min-height.
- Focus: border goes to `--ink`, ring is a 3px `--sunken` halo. **Not accent** (A3).
- Placeholder: `--text-tertiary`.
- Labels: 11.5px, 600, `--text-secondary`, sentence case.

### Focus
Ink ring, 2px, offset 2px, `:focus-visible` only. Pointer users never see it;
keyboard users always do. Never `outline: none` without a replacement.

### Nav & view switcher
- Rail/sidebar on `--bg`, content on `--surface`. The tone change does the
  grouping; no border needed.
- Segmented control: `--sunken` track, `--radius-full`, white/`--surface` thumb
  with `--shadow-xs`.
- Active segment uses `--text` + the thumb. **Not accent** — the thumb already
  communicates selection, and a second coloured cue is noise.

### Tables
- `th` on `--surface-2`, 11.5px `--text-secondary`, sticky.
- Row height 30px, zebra via `--stripe` (a translucent warm wash).
- Hover `--stripe-hover`; selected `--accent-soft` + a 2px `--accent` left bar.

### Chat
- User message: `--ink` fill, `--ink-ink` text, `--radius-md`.
- Assistant message: `--surface` fill, `--border`, no shadow.
- Citations: accent links.
- The composer is a single input well with `--sunken` fill and an inline
  toolbar. The mention popup is a floating `--overlay` card with `--shadow-lg`.

### Empty states
Left-aligned, no icon tile (A10). A single muted glyph, the title in serif, one
line of hint, then the action. No exclamation, no illustration.

## 11. Motion

| Token | Value |
|---|---|
| `--dur-fast` | 120ms |
| `--dur` | 180ms |
| `--dur-slow` | 240ms |
| `--ease` | `cubic-bezier(.22, 1, .36, 1)` — a settle curve |

- Animate `transform` and `opacity`. Colour transitions are fine for hover.
- `prefers-reduced-motion: reduce` collapses all durations to 0.01ms globally.
- The theme swap transitions background and colour only, so it does not strobe.
- Nothing enters with a bounce or a scale-up. Panels and dialogs fade.
- **Entrances** are a fade plus a few pixels of travel from the side they open
  toward. There are two keyframes, shared app-wide in `app.css`:
  - `menu-drop` (−4px → 0), for menus that open downward: QuickAdd, the
    context menu, the conversation list, and expanded citations.
  - `menu-rise` (+4px → 0), for anything that opens upward: the composer's
    command menu and @ strip, and new chat messages.
- Dialogs rise 6px as they fade in (`dialog-in`), and their scrim fades with
  them.
- **Changing views and panels** uses view transitions
  (`lib/viewTransition.ts`): the change renders once and the browser animates
  between snapshots.
  - Switching views fades the old stage out (`--dur-fast`) and settles the new
    one up 6px (`--dur`). The rail and assistant stay put.
  - Docking the rail, moving the assistant, or hiding it (Ctrl+J) glides the
    three panels to their new boxes (`--dur-slow`).
  - Only what changes is captured. The page root is never snapshotted, and the
    panels are named only during layout changes.
  - It adapts: if a view switch takes more than 120ms to prepare (only on very
    heavy pages), view switches go instant for the session. Reduced motion
    disables all of it.
- Reduced motion sets transitions to `0s`, never a tiny non-zero value. A
  non-zero duration gives every element a transition of `all`, which made a
  theme swap animate the whole page.
- **Exits are instant.** A closing menu has nothing left to say.
- **Deletes pop** (`lib/pop.ts`), the one deliberate exception to "nothing
  celebrates". A deleted item needs visible confirmation where it stood.
  - Every visible copy (tagged `data-artifact`) swells to 1.05 and shrinks
    away over 560ms, while an ink ring and eight dots burst from its centre
    (720ms).
  - **Rollback** (`lib/undoDelete.ts`): the delete is held, not run. The item
    is hidden everywhere, and the toast offers "→ Rollback" (an arrow and an
    underlined word, in the toast's colour) for 7s. The real delete runs when
    that time is up. Rollback, or Ctrl+Z outside a text field, cancels it and
    the item settles back in. Starting another delete confirms the pending
    one, so only one is ever held.
  - A key matches everything nested under it, so `person:p1` pops the person
    everywhere and `person:p1:c2` only on that case.
  - If nothing is on screen, the burst plays where the user clicked. Reduced
    motion skips it.
- **Press:** every small control settles 1px on `:active`, the same as `.btn`:
  rail icons, row actions, chips, tabs, crumbs, and menu items. Every button
  eases its hover colours through a zero-specificity `:where(button)` rule, so
  component transitions still win.

## 12. Verification gates

These are pass/fail, and they run.

**Contrast.** `bun run scripts/contrast-gate.ts` — 57 pairs across both themes.
Body text ≥ 4.5:1, control boundaries ≥ 3:1, focus rings ≥ 3:1. Soft fills are
composited over their real base before measuring, because a 10%-alpha green over
white is not white.

**Type checks.**
- `bun run check` — 0 errors, 0 warnings.
- `bun test` — all green.
- `bun run build` — succeeds.

**Token discipline.** No raw hex, no `px` radius literal, and no `rgba()` shadow
may appear in any `.svelte` file. All of it lives in `app.css`. This is enforced
by grep, not by eye:

```
rg -n '#[0-9a-fA-F]{3,8}|rgba?\(' src --glob '*.svelte'
```

Zero matches is the target.

## 13. Migration map

Ordered by blast radius. Each step is independently shippable.

| Step | File | Change |
|---|---|---|
| 1 | `src/app.css` | Replace token blocks, radii, shadows, type scale, buttons, focus, dialog, empty states, badges. Add `--ink*`, `--text-disabled`, `--surface-2`, `--dur-fast/slow`. Rename `--text-muted`→`--text-secondary`, `--text-faint`→`--text-tertiary`. |
| 2 | `lib/theme.ts` | No change. |
| 3 | `WorkspaceHeader.svelte` | Segmented control: thumb-based, ink focus. |
| 4 | `chat/Composer.svelte` | Composer well, floating mention popup (`--overlay` + `--shadow-lg`). |
| 5 | `chat/MessageBubble.svelte` | User bubble → `--ink`. |
| 6 | `chat/ChatPanel.svelte` | Empty state → no icon tile, left-aligned. |
| 7 | `views/TreeGrid.svelte` | 30px rows, `--stripe` tokens, selected left bar. Keep `ROW_H` and CSS in sync. |
| 8 | `views/CaseDetail.svelte` | Case title → 25px serif; edit field matches exactly. |
| 9 | `views/TableView.svelte` | `th` on `--surface-2`, selected left bar. |
| 10 | `views/CalendarView.svelte` | `KIND_COLOR` → semantic tokens. |
| 11 | `views/Settings.svelte` | Warm swatch set; all ≥ 4.5:1 on every light surface. |
| 12 | `views/DocumentViewer.svelte` | Toolbar in `--surface-2`, no coloured chrome. |
| 13 | `components/*` | `Toast` (shadow, no border), `Modal`, `QuickAdd`, `StatusBadge`, `SheetGrid`. |
| 14 | `views/SheetView.svelte`, `People.svelte`, `SearchView.svelte` | Token sweep. |

| 15 | `components/Rail.svelte`, `+layout.svelte`, `WorkspaceHeader.svelte` | Rail; collapsible assistant; view switcher removed from the header. |
| 16 | `components/CommandPalette.svelte`, `Modal.svelte` | Quick find on a headless, top-anchored `Modal`. |
| 17 | `components/PageHeader.svelte` → `CaseDetail`, `DocumentViewer`, `SheetView` | Shared page header and properties. |
| 18 | `TreeGrid.svelte`, `TableView.svelte` | `.row-actions`, `A` to ask. |
| 19 | `chat/Composer.svelte`, `lib/commands.ts` | `/` menu. |
| 20 | `src-tauri/src/commands.rs`, `lib/steps.ts`, `ChatPanel`, `MessageBubble` | `chat:step` events, live checklist, collapsed trail. |
| 21 | `lib/i18n/*`, every component | EN/PT dictionaries, `t()` / `label()`, language switch in rail, quick find and Settings. |
| 22 | `lib/layout.ts`, `lib/panels.svelte.ts`, `PanelGrip.svelte`, `+layout.svelte` | Rail docks to any edge, assistant to either side, by drag or keyboard; persisted; reset. |

**Category swatches** stay a full hue wheel — categories must be distinguishable
at a glance — but pitched warm: sienna, ochre, olive, pine, teal, plum,
rosewood. All seven must clear 4.5:1 on `--bg`, `--surface`, and `--surface-2`.

## 14. Definition of done

- [ ] `app.css` is the only file containing colour, radius, or shadow literals.
- [ ] No primary button anywhere uses `--accent`.
- [ ] No button uses `--radius-full` unless it is a tag or segmented thumb.
- [ ] No card has both a border and a shadow.
- [ ] No `backdrop-filter` anywhere.
- [ ] No gradient skeleton.
- [ ] Empty states have no icon tile.
- [ ] Serif appears only on the case title.
- [ ] 57/57 contrast pairs pass, both themes.
- [ ] `check`, `test`, `build` all green.
- [ ] The rail is present; the header has no view switcher.
- [ ] Ctrl+K opens quick find from every pane; Ctrl+J toggles the assistant.
- [ ] Case, document and sheet pages share `PageHeader`.
- [ ] No `✦` anywhere (`rg '✦' src` is empty).
- [ ] A chat turn shows its steps live, then collapses to one summary line.
- [ ] No user-facing English literal in a `.svelte` file; every string goes through `t()`.
- [ ] `pt` has every `en` key with matching placeholders (`bun test`).
- [ ] The rail docks to all four edges and the assistant to both sides, by drag and by keyboard, and the layout survives a restart.
- [ ] The shell fills the window height at every layout.
- [ ] Typing `@…` never searches; the documents are searched only on send.
- [ ] Every artifact, on every surface that shows it, opens the same context menu on right-click and on Shift+F10.
- [ ] No Tauri command is synchronous (`rg '#\[tauri::command\]\npub fn' -U` is empty).

## 15. Structure & interaction

§2–§14 took Notion's and Manus's *surface*. This section takes their *shape*:
where things live, how you move, and how the agent shows its work.

**From Notion:** quick find, pages with a consistent header and properties,
affordances that appear on hover, the `/` menu.
**From Manus:** the conversation beside the work, and an agent that shows its
steps instead of a spinner.

### 15.1 Shell

Default: `rail (44px) | assistant (300–720px, collapsible) | workspace`, filling
the full window height. Where the rail docks and which side the assistant sits
on are user-owned (§15.10).

- The **rail** sits on `--bg` and holds destinations only: quick find, the
  assistant toggle, Tree, Table, Calendar, Search, People, then theme and
  Settings pinned to the bottom. Glyphs are 32px square with `--radius-sm`.
  Active is a `--sunken` fill with `--text`. **Never accent** (§5.4).
- The **workspace header** holds location (breadcrumb) and page-level actions
  only. It does not switch views; the rail does.
- No borders between rail and chat. The tone change does the grouping.
- A collapsed assistant is hidden, not unmounted, so its draft and scroll
  survive. The state persists (`sato.chatCollapsed`).

### 15.2 Quick find

- Ctrl+K (⌘K) opens it everywhere, and a second press closes it.
- It is a headless dialog, centred like every other dialog: `--overlay`,
  `--shadow-lg`, no border (§9), flat scrim (A9).
- The input is the header: 15px, borderless, no focus ring. The whole dialog
  is the focus context.
- With an empty query it shows Recent (the last 5 distinct panes), then Actions.
  With a query it shows tree matches grouped in `KIND_ORDER` (max 5 per kind),
  then matching Actions.
- The highlighted item is a `--sunken` fill. ↑/↓ move, Enter runs, Esc closes.
  The pointer moves the highlight but never steals the cursor.

### 15.3 Pages

A case, a document and a sheet are pages. They share `PageHeader`:

1. An eyebrow line: kind glyph, then the handle (mono) or the kind name, 11.5px
   `--text-tertiary`.
2. The title. Serif `h1` for a case only (A11); sans `h2` for everything else.
   An inline edit field must match the title exactly (§6).
3. Properties: **one wrapping line** of label/value pairs, 20px apart. Label is
   12.5px `--text-tertiary`, value is 13.5px `--text`. Badges and links are
   allowed as values. Nothing is boxed.
4. Page actions, right-aligned, wrapping under the title when narrow.

Notion stacks properties vertically. We keep them on one line because every
page here is a working surface (a viewer or a grid), and vertical space belongs
to the content.

### 15.4 Hover affordances

- Secondary row commands (`.row-actions`) are `opacity: 0` at rest. They
  appear on `:hover` and `:focus-within`, and on the keyboard-selected row.
- Every hover action has a keyboard path. In the tree and in tables, `Enter`
  opens and `A` asks the assistant.
- They must fit inside the 30px row. A hover action never changes row height.

### 15.5 Agent steps

- While a turn runs, the thread shows a checklist in place of a spinner:
  **Reading question → Searching documents → Drafting answer**.
- Glyphs: `○` pending, the square spinner (§15.11) running, `✓` done. The text
  is 11.5px `--text-tertiary`, moving to `--text-secondary` once a step has
  started.
- The search step carries its result: "8 passages in 3 documents".
- The backend emits `chat:step` `{conversationId, step, state, detail}`. A
  later step implies earlier ones are done, so the list never regresses.
- When the reply lands, the steps collapse into a disclosure on the message:
  "✓ Searched 8 passages in 3 documents". The citations sit inside it. It is
  derived from the citations, so older messages get it too.

### 15.6 Composer

- `@` is a **confirmation, not a search.** While the user types a handle,
  one strip above the input shows the full path it resolves to, and Tab fills
  it in. There is no list to pick from, and no ranking on keystrokes.
  - Resolution is an exact or prefix match on the full handle, falling back to
    the file name. Both are binary searches over arrays sorted once per tree
    (`mentionLookupFor`), so a keystroke costs O(log n) at any tree size.
  - The strip is in flow, not floating: the kind glyph, then `@path` in mono
    `--text`, then "+N more" when the prefix is ambiguous, then `Tab to
    complete` pushed to the far end. Once the handle is complete it reads
    "✓ Found". With no match it reads "Nothing matches @…". Esc hides it for
    that mention.
  - The documents are searched only when the message is sent: mentions are
    resolved then, and retrieval is scoped to them.
- `/` at position 0 opens the command menu (`summarize`, `timeline`,
  `deadlines`, `compare`, `draft`). A command expands into a plain-text prompt
  with the caret placed after the first `@`, so the mention strip takes over.
  Nothing new is sent to the backend.
- The command menu floats: `--overlay`, `--shadow-lg`, no border.

### 15.7 Identity

The assistant's mark is `§`, the section sign: a lawyer's glyph, not an AI's.
A sparkle (`✦`) is banned as the §2 rejection of "AI sparkle" iconography.

### 15.8 Shortcuts

| Keys | Action |
|---|---|
| Ctrl/⌘ + K | Quick find |
| Ctrl/⌘ + J | Show / hide assistant |
| Alt + ← / → | Back / forward |
| Enter · A | Open · Ask about the selected tree or table row |
| `/` (at start) | Composer commands |
| `@` | Reference a node |
| Arrow keys on a panel's ⠿ handle | Move that panel (rail: any edge; others: ← / →) |
| Esc while dragging a panel | Cancel the move |

### 15.8b Responsiveness

- The window never freezes on the backend. Every Tauri command is `async`, so
  none runs on the main thread. A synchronous command waiting on the database
  lock used to freeze the whole UI.
- Retrieval runs on a blocking thread, off the async runtime, against a
  read-only SQLite connection, so indexing writes never make a question wait.
- Keystrokes never trigger a search (§15.6).
- **Answers stream.** Tokens arrive as `chat:delta` events, batched to about
  25 per second in Rust and applied once per animation frame in the panel. The
  transcript follows the stream only while the reader is already at the
  bottom.
- Chat messages resolve their @mentions through one handle map per tree
  (`nodeKeyMap`), never a map per message.

**Retrieval architecture.**
- **The vector database** (`src-tauri/src/vectordb.rs`) is embedded and in
  memory, with no server.
  - Each point is a chunk: its text, page and character range, plus its
    unit-length embedding.
  - It persists as one checksummed segment file per document under
    `vectors/`, written atomically.
  - Queries are planned: an exact scan below 20k candidates (and always for an
    @mentioned document), HNSW above that. The graph builds on a background
    thread, and newer points are scanned exactly until they are folded in.
  - Deletes leave tombstones, and the store compacts once 25% of slots are
    dead. Measured HNSW recall@10 is 0.995 against exact search.
- **SQLite** keeps documents, cases and the keyword index (`passages_fts`,
  diacritic-insensitive). Keyword queries OR their terms and drop English and
  Portuguese stopwords. Reciprocal rank fusion merges the two rankings.
- **At startup**, the two stores are reconciled: orphan segments are dropped,
  and indexed documents missing their vectors are flagged for reindex.
- **Pre-vector-database installs** migrate once, automatically.

### 15.11 The spinner

"Your action is being processed." A 10px square with a 2px `--text-tertiary`
outline and 2px corners. It tumbles a quarter turn at a time on `--ease`, one
full turn per 1.6s, so it reads as work in progress without pulling the eye.
It is CSS-only, so it keeps moving while the app waits on the backend. It is
the one spinner in the app: buttons, empty states, the tree's refresh, and the
running chat step all use `.spinner`. Under `prefers-reduced-motion` it holds
still as a static square.

### 15.12 Context menu

Right-click any artifact (case, document, sheet, person, event) wherever it
appears to get the same menu: the tree, tables, the case page's chips, chat
mentions and citations, search results, calendar events, and rosters. The
ContextMenu key or Shift+F10 opens it from the keyboard on a focused tree or
table row.

- **Contents**, in fixed order: Open (↗) and Ask the assistant (§); then the
  kind's own actions (Edit details… / Rename… / Edit person… / Edit event…,
  Reindex, the case's Table/Calendar/Roster) and Copy mention (@); then the
  destructive entries last.
- **Destructive entries** are `--danger` text with a `--danger-soft` hover.
  That is the only colour in the menu, and a semantic one. Every delete
  confirms first. A person offers "Remove from case" (not destructive) above
  "Delete person…".
- **The surface:** a muted header naming the artifact, 30px items, a
  tertiary glyph column, and 1px `--border` separators. It floats: `--overlay`,
  `--shadow-lg`, no border, `menu-drop` entrance. It clamps and flips to stay
  on screen.
- **Keyboard:** focus lands on the first item. ↑/↓/Home/End move, Enter runs,
  and Esc or Tab closes and returns focus to where it was. A press outside, a
  scroll, a resize, or the window losing focus also closes it.
- **Routing, not reimplementing:** every entry calls what the owning view
  already does. Edit entries set `editRequest`, and the view opens its own
  editor once its data has loaded (the calendar first jumps to the event's
  month). Deletes bump `artifactsVersion`, so views that keep their own rows
  refetch, and a deleted page that is open returns to the tree.

### 15.15 Case connections

The case page has a small window, 240px tall and bordered like the stats card,
showing everything filed under the case as a tree on an endless dot grid.

- **Layout** (`lib/caseGraph.ts`, tested): the case on the left, in ink like a
  primary action; one group per kind in a fixed order (documents, people,
  sheets, events), in `--sunken` with its count; then the items. Each group is
  centred on its items, and each edge is an S-curve.
- **Moving around:** drag or scroll to pan, Ctrl+scroll to zoom about the
  pointer (0.3–2.5×), double-click empty space to fit, plus −/+/fit buttons.
  From the keyboard: arrows pan, `+` and `-` zoom, `0` fits, and Tab moves
  through items.
- **Items:** click (or Enter) opens one; right-click (or Shift+F10) opens the
  usual context menu. Hover or focus lights the item's branch back to the
  case, in ink.
- **Empty:** with nothing filed, the window runs Game of Life on the surface
  colour, with ink cells, fed by a sine wave front sweeping across, so it never
  settles. That's black on white in light mode. It runs at about 12 fps, pauses
  off screen and in the background, and shows a still frame under reduced
  motion.

### 15.14 AI provider problems

When the assistant can't work, the app says why and how to fix it. It never
shows a raw network error.

- **Diagnosis** (`Provider::diagnose`): is the server reachable (4s timeout),
  is the key accepted, and does it have both configured models? Failures are
  classified as `unreachable`, `timeout`, `model_missing`, `unauthorized` or
  `other`, and the full cause chain is kept as detail.
- **Where it shows:** the same `ProviderProblem` component appears in three
  places, all as a warn-tinted card:
  - a compact, expandable banner at the top of the chat, checked when the chat
    opens and whenever a turn degrades or fails;
  - inside a failed reply, from the JSON diagnosis stored in `error`;
  - in Settings, under the provider.
- **What it says** depends on the problem:
  - Ollama unreachable: install it (link), start it (`ollama serve`), and pull
    both models, with copy-on-click commands.
  - Model missing: `ollama pull <model>`.
  - Timeout: the model may still be loading.
  - Unauthorized: check the key.
  - The address is always shown, with "Check again" and Settings actions.
- **Degrade, don't fail:** if embedding fails, the turn continues on keyword
  matches, and the step reads "Semantic search unavailable — using keyword
  matches". Only a failed chat model ends the turn.

### 15.13 Delete confirmation

Every delete asks first, in an in-app dialog, never the browser's `confirm()`,
which blocks the page and can't be translated.

- The title names the kind of item ("Delete this document?"). The body names
  the item and what goes with it. A secondary line says it can't be undone.
- Buttons are Cancel and Delete. **Focus starts on Cancel**, so an Enter
  pressed out of habit never deletes. Esc cancels.
- "Don't ask me again" is only remembered if the user then confirms. Settings
  → Confirmations → "Ask before deleting" turns it back on.

### 15.9 Language

- English and Português (Brasil) ship today. `src/lib/i18n/en.ts` is the source
  dictionary. Every other locale is typed `Record<MessageKey, string>`, so a
  missing key fails `bun run check`, and `tests/i18n_layout_spec.ts` fails on
  mismatched `{placeholders}` or incomplete `_one`/`_other` plural pairs.
- **Switching:** quick find (Ctrl+K) lists "Language: …" for every other
  locale, and Settings has a segmented control. The rail carries no language
  control. The choice persists (`sato.locale`). First launch follows the
  OS language.
- The switch is live: `t()` reads the locale from `$state`, so nothing reloads.
  Dates, times and weekday names go through `Intl` with the active locale.
- **Stored data is never translated.** Case statuses, roles and event kinds are
  English values in the database. Only their display goes through `label()`.
  The backend sends counts, not sentences (`chat:step` carries `passages` and
  `documents`), so the UI words them.
- Slash-command templates are translated, so a Portuguese user sends a
  Portuguese prompt.
- Not yet translated: backend-authored strings (provider error details,
  indexing stage names, failed-reply messages). They arrive as finished English
  sentences and need codes from Rust before they can be localised.

### 15.10 Panel layout

Two independent choices, not one column order:

- **Rail dock:** `left`, `right`, `top` or `bottom`. On the sides it is a 44px
  column. On the top or bottom it is a 44px bar with the same contents on the
  other axis, plus a 1px `--border` hairline on its inner edge, because it
  sits against `--bg` headers where there is no tone change to group it.
- **Assistant side:** `left` or `right` of the workspace.

Panels are placed with CSS `order`, never by moving DOM nodes, so a layout
change never remounts the assistant or the workspace view.

- Each panel has a `⠿` grip: across the top of the rail (along its start when
  horizontal), and at the start of the assistant's and the workspace's
  headers. The grip is `--text-tertiary` at rest, with a `--sunken` fill and a
  `grab` cursor on hover.
- **Drag:** press the grip and move 4px to lift the panel. It dims to 45%, a
  label follows the pointer (`--overlay`, `--shadow-md`, no border), and a
  preview shows where it will land: a 2px `--ink` outline over a `--sunken`
  wash. It is never accent (§5.4).
  - The rail docks to the nearest window edge, measured relative to the
    window's size so top and bottom are as easy to reach as the sides. The
    preview is a band along that edge.
  - The assistant or workspace goes to whichever half of the content area the
    pointer is over. The preview is a column of the panel's width on that side.
  - Release to drop; Esc cancels. A drop that would change nothing shows no
    preview.
- It uses pointer events, not HTML5 drag-and-drop, because Tauri's file-drop
  handler swallows native drag events in the Windows webview.
- **Keyboard:** focus a grip. On the rail, ← → ↑ ↓ dock it to that edge; on
  the others, ← / → pick the side. Quick find offers the other three docks,
  the other assistant side and, when not default, "Reset panel layout".
  Settings has a segmented control for each choice plus reset.
- The assistant's resize handle and its 1px divider follow it to whichever
  side faces the workspace.
- The layout persists (`sato.layout`). A malformed stored value falls back
  to the default field by field, so no panel can be lost.
- The rail never lets its buttons shrink: a window too short for it scrolls
  the rail instead.
