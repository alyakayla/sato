import { en, type MessageKey } from './en';
import { pt } from './pt';

/**
 * A deliberately small i18n layer.
 *
 * `en` is the source of truth and every other dictionary is typed against its
 * keys, so a missing translation is a type error, not a blank label. `t` reads
 * the locale from `$state`, so any template that calls it re-renders when the
 * language changes — no reload, no per-component wiring.
 *
 * Stored data stays in English (case statuses, roles, event kinds are values
 * in the database). Only their *display* is translated, via `label`.
 */

export type Locale = 'en' | 'pt';

export const LOCALES: { id: Locale; short: string; name: string; intl: string }[] = [
  { id: 'en', short: 'EN', name: 'English', intl: 'en-US' },
  { id: 'pt', short: 'PT', name: 'Português (Brasil)', intl: 'pt-BR' },
];

const DICTS: Record<Locale, Record<MessageKey, string>> = { en, pt };

const KEY = 'sato.locale';

function initial(): Locale {
  const stored = localStorage.getItem(KEY);
  if (stored === 'en' || stored === 'pt') return stored;
  // First launch follows the OS language; after that the user's choice wins.
  return navigator.language?.toLowerCase().startsWith('pt') ? 'pt' : 'en';
}

let current = $state<Locale>(initial());
document.documentElement.lang = LOCALES.find((l) => l.id === current)!.intl;

export function getLocale(): Locale {
  return current;
}

export function setLocale(next: Locale): void {
  current = next;
  localStorage.setItem(KEY, next);
  document.documentElement.lang = intlLocale();
}

/** BCP 47 tag for `Intl` / `toLocale*` calls. */
export function intlLocale(): string {
  return LOCALES.find((l) => l.id === current)!.intl;
}

export type Vars = Record<string, string | number>;

/** `foo` when the dictionary has `foo_one` / `foo_other`. */
type PluralBase<K> = K extends `${infer B}_other` ? B : never;
export type TKey = MessageKey | PluralBase<MessageKey>;

/**
 * Translates `key`, filling `{name}` placeholders from `vars`.
 *
 * Plurals: when `vars.n` is given and `key_one` / `key_other` exist, the right
 * form is chosen with `Intl.PluralRules`, so languages with different plural
 * rules work without special cases here.
 */
export function t(key: TKey, vars?: Vars): string {
  return translate(current, key, vars);
}

export function translate(locale: Locale, key: TKey, vars?: Vars): string {
  const dict = DICTS[locale];
  let msg: string | undefined;
  if (vars && typeof vars.n === 'number') {
    const form = new Intl.PluralRules(LOCALES.find((l) => l.id === locale)!.intl).select(vars.n);
    msg = dict[`${key}_${form}` as MessageKey] ?? dict[`${key}_other` as MessageKey];
  }
  msg ??= dict[key as MessageKey] ?? en[key as MessageKey] ?? key;
  if (!vars) return msg;
  return msg.replace(/\{(\w+)\}/g, (m, name: string) => (name in vars ? String(vars[name]) : m));
}

/**
 * Display label for a stored enum value (status, role, event kind). Unknown
 * values — user-entered or from a newer backend — fall through untranslated.
 */
export function label(value: string): string {
  const key = `value.${value}` as MessageKey;
  return key in en ? t(key) : value;
}

export type { MessageKey };
