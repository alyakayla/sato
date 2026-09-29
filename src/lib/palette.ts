/**
 * The practice-area swatch set.
 *
 * Categories are *user content*, the one place a chromatic hue is unambiguously
 * legal, so they keep a full hue wheel - sienna, ochre, olive, pine, teal,
 * plum, rosewood - pitched warm so the set sits in the same light as the rest
 * of the palette rather than fighting it.
 *
 * The values live here rather than in `app.css` because a category colour is
 * *data*, not styling: it is written to the database, read back as a string,
 * and applied as an inline colour wherever a category appears (the tree's
 * category dot, the case reference, the settings list). Design tokens cannot
 * be persisted. Everything that is styling - surfaces, text, borders, radius,
 * shadow - still lives in `app.css` and nowhere else.
 *
 * Every value was solved numerically, not eyeballed: each clears 4.5:1 against
 * `--bg`, `--surface`, `--surface-2` *and* `--sunken`, so a swatch stays
 * legible as a 7px dot and as a run of label text. The binding constraint is
 * `--sunken`; these are the lightest steps that clear it.
 */

export interface Swatch {
  /** Stable key, also the `aria-label` and tooltip. */
  name: string;
  /** Hex, stored in the database with the category. */
  value: string;
}

export const CATEGORY_SWATCHES: Swatch[] = [
  { name: 'sienna', value: '#A45029' },
  { name: 'ochre', value: '#89610A' },
  { name: 'olive', value: '#57702F' },
  { name: 'pine', value: '#227560' },
  { name: 'teal', value: '#2A7088' },
  { name: 'plum', value: '#8455A2' },
  { name: 'rosewood', value: '#A84962' },
];

/** The swatch a new category starts on: the first step of the wheel. */
export const DEFAULT_SWATCH = CATEGORY_SWATCHES[0]!.value;
