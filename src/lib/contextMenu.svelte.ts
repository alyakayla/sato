/**
 * The app's one context menu. Any surface opens it with a list of entries at
 * a point; `ContextMenu.svelte`, mounted once in the shell, renders it.
 */

export interface MenuItem {
  id: string;
  label: string;
  glyph?: string;
  /** Right-aligned hint, e.g. a shortcut. */
  hint?: string;
  danger?: boolean;
  disabled?: boolean;
  run: () => void | Promise<void>;
}

export type MenuEntry = MenuItem | 'separator';

interface MenuState {
  open: boolean;
  x: number;
  y: number;
  /** What the menu acts on, shown as a muted header. */
  title: string | null;
  items: MenuEntry[];
  /** Where focus returns when the menu closes. */
  returnFocus: HTMLElement | null;
}

export const menu = $state<MenuState>({ open: false, x: 0, y: 0, title: null, items: [], returnFocus: null });

/**
 * Opens the menu at a pointer event, or at an element (for the keyboard's
 * ContextMenu key / Shift+F10, which have no pointer position).
 */
export function openContextMenu(at: MouseEvent | HTMLElement, items: MenuEntry[], title: string | null = null): void {
  if (at instanceof MouseEvent) {
    at.preventDefault();
    at.stopPropagation();
    menu.x = at.clientX;
    menu.y = at.clientY;
  } else {
    const r = at.getBoundingClientRect();
    menu.x = r.left + 16;
    menu.y = r.bottom;
  }
  menu.returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  menu.title = title;
  menu.items = items;
  menu.open = true;
}

export function closeContextMenu(restoreFocus = false): void {
  if (!menu.open) return;
  menu.open = false;
  if (restoreFocus) menu.returnFocus?.focus();
  menu.returnFocus = null;
}

/** True for the keys that open a context menu without a mouse. */
export function isContextMenuKey(e: KeyboardEvent): boolean {
  return e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10');
}
