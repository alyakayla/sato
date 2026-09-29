// WCAG contrast gate for the DESIGN.md palette. Run: bun contrast.ts
type RGB = [number, number, number];

const hex = (h: string): RGB => {
  const s = h.replace('#', '');
  const f = s.length === 3 ? s.split('').map((c) => c + c).join('') : s;
  return [0, 2, 4].map((i) => parseInt(f.slice(i, i + 2), 16)) as RGB;
};

const lin = (c: number) => {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
};

const lum = (rgb: RGB) => {
  const [r, g, b] = rgb.map(lin);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

const ratio = (a: string, b: string) => {
  const l1 = lum(hex(a));
  const l2 = lum(hex(b));
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
};

// Composite a translucent token over an opaque base, as the browser will.
const over = (rgba: string, base: string) => {
  const m = rgba.match(/rgba?\(([^)]+)\)/)!;
  const p = m[1].split(',').map((x) => parseFloat(x));
  const [r, g, b, a] = p.length === 4 ? p : [...p, 1];
  const B = hex(base);
  const out = [r, g, b].map((v, i) => Math.round(v * a + B[i] * (1 - a)));
  return '#' + out.map((v) => v.toString(16).padStart(2, '0')).join('');
};

const T = {
  bg: '#F4F2EF',
  surface: '#FFFFFF',
  surface2: '#FAF8F6',
  sunken: '#EDEAE5',

  text: '#1F1C18',
  textSecondary: '#57514A',
  textTertiary: '#6F685E',

  accent: '#B04A26',
  accentSoft: 'rgba(176,74,38,0.09)',
  ink: '#211E1A',
  white: '#FFFFFF',

  ok: '#2F6F4E',
  okSoft: 'rgba(47,111,78,0.10)',
  warn: '#8A5B08',
  warnSoft: 'rgba(138,91,8,0.11)',
  danger: '#A6362A',
  dangerSoft: 'rgba(166,54,42,0.10)',

  borderControl: '#878176',
  border: '#E5E1DB',
};

// D = design minimum. 4.5 = body text, 3.0 = control boundary / large text.
const checks: Array<[string, string, string, number]> = [
  // --- body text on every surface it can land on -------------------------
  ['text', 'text', T.bg, 4.5],
  ['text', 'text', T.surface, 4.5],
  ['text', 'text', T.surface2, 4.5],
  ['text', 'text', T.sunken, 4.5],
  ['text-secondary', 'textSecondary', T.bg, 4.5],
  ['text-secondary', 'textSecondary', T.surface, 4.5],
  ['text-secondary', 'textSecondary', T.surface2, 4.5],
  ['text-secondary', 'textSecondary', T.sunken, 4.5],
  ['text-tertiary', 'textTertiary', T.bg, 4.5],
  ['text-tertiary', 'textTertiary', T.surface, 4.5],
  ['text-tertiary', 'textTertiary', T.surface2, 4.5],
  // Quick-find hints on the highlighted item; rail/row-action glyphs at rest.
  ['text-tertiary on sunken', 'textTertiary', T.sunken, 4.5],

  // --- links / mentions ----------------------------------------------------
  ['accent on bg', 'accent', T.bg, 4.5],
  ['accent on surface', 'accent', T.surface, 4.5],
  ['accent on surface-2', 'accent', T.surface2, 4.5],

  // --- primary button ------------------------------------------------------
  ['white on ink', 'white', T.ink, 4.5],

  // --- badges and alerts: coloured text on its own soft fill ---------------
  ['ok on ok-soft/surface', 'ok', over(T.okSoft, T.surface), 4.5],
  ['ok on ok-soft/bg', 'ok', over(T.okSoft, T.bg), 4.5],
  ['warn on warn-soft/surface', 'warn', over(T.warnSoft, T.surface), 4.5],
  ['warn on warn-soft/bg', 'warn', over(T.warnSoft, T.bg), 4.5],
  ['danger on danger-soft/surface', 'danger', over(T.dangerSoft, T.surface), 4.5],
  ['danger on danger-soft/bg', 'danger', over(T.dangerSoft, T.bg), 4.5],
  ['accent on accent-soft/surface', 'accent', over(T.accentSoft, T.surface), 4.5],

  // --- body text must survive landing on a soft fill ----------------------
  ['text on accent-soft', 'text', over(T.accentSoft, T.surface), 4.5],
  ['text on ok-soft', 'text', over(T.okSoft, T.surface), 4.5],
  ['text on danger-soft', 'text', over(T.dangerSoft, T.surface), 4.5],

  // --- control boundaries (WCAG 1.4.11) -----------------------------------
  ['border-control on surface', 'borderControl', T.surface, 3.0],
  ['border-control on bg', 'borderControl', T.bg, 3.0],
  ['border-control on sunken', 'borderControl', T.sunken, 3.0],

  // --- focus ring must be visible against the surface it sits on ----------
  ['ink focus on bg', 'ink', T.bg, 3.0],
  ['ink focus on surface', 'ink', T.surface, 3.0],
];

const resolve = (v: string) =>
  v.startsWith('#') ? v : (T[v as keyof typeof T] as string);
void dChecks;

let fails = 0;
const rows = checks.map(([label, fgV, bgV, min]) => {
  const fg = resolve(fgV);
  const bg = resolve(bgV);
  const r = ratio(fg, bg);
  const ok = r >= min;
  if (!ok) fails++;
  return `${ok ? 'PASS' : 'FAIL'}  ${r.toFixed(2).padStart(6)}  (min ${min})  ${label}  [${fg} on ${bg}]`;
});

console.log(rows.join('\n'));
console.log(`\n${checks.length - fails}/${checks.length} pass, ${fails} fail`);

// Decorative borders are exempt but should still be visible as a line.
console.log(`\nborder (decorative, exempt) vs surface: ${ratio(T.border, T.surface).toFixed(2)}`);
console.log(`border (decorative, exempt) vs bg:      ${ratio(T.border, T.bg).toFixed(2)}`);

// ===========================================================================
// Dark theme - a warm near-black, not cold slate. Structure mirrors light.
// ===========================================================================

const D = {
  bg: '#191714',
  surface: '#211E1A',
  surface2: '#1D1B17',
  sunken: '#131210',
  overlay: '#26231F',

  text: '#F2EEE8',
  textSecondary: '#BDB6AB',
  textTertiary: '#9B9387',

  accent: '#E08A5A',
  accentSoft: 'rgba(224,138,90,0.13)',
  ink: '#F2EEE8',
  bgInk: '#191714',

  ok: '#5CB98A',
  okSoft: 'rgba(92,185,138,0.13)',
  warn: '#D9A441',
  warnSoft: 'rgba(217,164,65,0.13)',
  danger: '#E8756A',
  dangerSoft: 'rgba(232,117,106,0.13)',

  borderControl: '#776F63',
  border: '#33302A',
};

const dChecks: Array<[string, string, number]> = [
  ['text', 'text', 4.5],
  ['text', 'textSecondary', 4.5],
  ['textSecondary', 'textSecondary', 4.5],
  ['textTertiary', 'textTertiary', 4.5],
  ['textTertiary', 'textTertiary', 4.5],
];

const dPairs: Array<[string, string, string, number]> = [
  ['text on bg', 'text', 'bg', 4.5],
  ['text on surface', 'text', 'surface', 4.5],
  ['text on surface-2', 'text', 'surface2', 4.5],
  ['text on sunken', 'text', 'sunken', 4.5],
  ['text-secondary on bg', 'textSecondary', 'bg', 4.5],
  ['text-secondary on surface', 'textSecondary', 'surface', 4.5],
  ['text-secondary on sunken', 'textSecondary', 'sunken', 4.5],
  ['text-tertiary on bg', 'textTertiary', 'bg', 4.5],
  ['text-tertiary on surface', 'textTertiary', 'surface', 4.5],
  ['text-tertiary on surface-2', 'textTertiary', 'surface2', 4.5],
  ['text-tertiary on sunken', 'textTertiary', 'sunken', 4.5],
  // Quick find and the composer menus float on --overlay, not --surface.
  ['text on overlay', 'text', 'overlay', 4.5],
  ['text-tertiary on overlay', 'textTertiary', 'overlay', 4.5],
  ['accent on bg', 'accent', 'bg', 4.5],
  ['accent on surface', 'accent', 'surface', 4.5],
  ['ink text on ink fill', 'bgInk', 'ink', 4.5],
  ['ok on ok-soft', 'ok', over(D.okSoft, D.surface), 4.5],
  ['warn on warn-soft', 'warn', over(D.warnSoft, D.surface), 4.5],
  ['danger on danger-soft', 'danger', over(D.dangerSoft, D.surface), 4.5],
  ['accent on accent-soft', 'accent', over(D.accentSoft, D.surface), 4.5],
  ['text on accent-soft', 'text', over(D.accentSoft, D.surface), 4.5],
  ['border-control on surface', 'borderControl', 'surface', 3.0],
  ['border-control on bg', 'borderControl', 'bg', 3.0],
  ['border-control on sunken', 'borderControl', 'sunken', 3.0],
  ['ink focus on bg', 'ink', 'bg', 3.0],
  ['ink focus on surface', 'ink', 'surface', 3.0],
];

let dFails = 0;
console.log('\n--- dark ---');
const dRows = dPairs.map(([label, fgV, bgV, min]) => {
  const fg = resolveD(fgV);
  const bg = resolveD(bgV);
  const r = ratio(fg, bg);
  const ok = r >= min;
  if (!ok) dFails++;
  return `${ok ? 'PASS' : 'FAIL'}  ${r.toFixed(2).padStart(6)}  (min ${min})  ${label}  [${fg} on ${bg}]`;
});
console.log(dRows.join('\n'));
console.log(`\n${dPairs.length - dFails}/${dPairs.length} pass, ${dFails} fail`);
console.log(`\nborder (decorative, exempt) vs surface: ${ratio(D.border, D.surface).toFixed(2)}`);

function resolveD(v: string) {
  return v.startsWith('#') ? v : (D[v as keyof typeof D] as string);
}
