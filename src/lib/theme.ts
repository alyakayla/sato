/**
 * Theme handling.
 *
 * The palette is defined entirely by custom properties in `app.css`, so
 * switching themes is one attribute on `<html>` - no component knows or cares
 * which theme is active.
 *
 * The default is light, deliberately, rather than the OS setting. This app is
 * document-centric and the warm paper palette is the intended look; inheriting
 * `prefers-color-scheme` would silently hand a dark-mode user the dark theme
 * on first launch, which is the opposite of the design. Dark remains one click
 * away and is remembered.
 *
 * The initial value is applied synchronously at module load, before the first
 * paint. Doing it in an `onMount` would flash the wrong theme on every launch.
 */

export type Theme = 'light' | 'dark';

const THEME_KEY = 'sato.theme';

function preferred(): Theme {
  const stored = localStorage.getItem(THEME_KEY);
  return stored === 'dark' ? 'dark' : 'light';
}

function apply(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

let current = preferred();
apply(current);

export function getTheme(): Theme {
  return current;
}

export function setTheme(theme: Theme): void {
  current = theme;
  apply(theme);
  localStorage.setItem(THEME_KEY, theme);
}

export function toggleTheme(): Theme {
  setTheme(current === 'dark' ? 'light' : 'dark');
  return current;
}

/**
 * Kept as a no-op hook so the shell does not need to know that the theme is
 * currently user-owned rather than system-owned. If a "follow system" option is
 * ever added, this is where the media query listener belongs.
 */
export function watchSystemTheme(): () => void {
  return () => {};
}
