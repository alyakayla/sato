import { describe, expect, it } from 'bun:test';
import { en } from '../src/lib/i18n/en';
import { pt } from '../src/lib/i18n/pt';
import {
  DEFAULT_LAYOUT,
  isDefaultLayout,
  keyLayout,
  nearestEdge,
  normalizeLayout,
  proposeLayout,
  sameLayout,
} from '../src/lib/layout';

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe('dictionaries', () => {
  const keys = Object.keys(en) as (keyof typeof en)[];

  it('Portuguese has exactly the English keys', () => {
    expect(Object.keys(pt).sort()).toEqual([...keys].sort());
  });

  it('every translation uses the same placeholders as the English', () => {
    const mismatched = keys.filter((k) => placeholders(en[k]).join() !== placeholders(pt[k]).join());
    expect(mismatched).toEqual([]);
  });

  it('no translation is empty', () => {
    expect(keys.filter((k) => !pt[k].trim() || !en[k].trim())).toEqual([]);
  });

  it('plural keys come in complete _one/_other pairs', () => {
    const ones = keys.filter((k) => k.endsWith('_one')).map((k) => k.slice(0, -4));
    const others = keys.filter((k) => k.endsWith('_other')).map((k) => k.slice(0, -6));
    expect(ones.sort()).toEqual(others.sort());
  });

  it('every stored enum value has a display label', () => {
    const stored = [
      'Open', 'Pending', 'On Hold', 'Closed', 'Ready', 'Indexing', 'Failed',
      'Client', 'Opposing Party', 'Witness', 'Judge', 'Expert', 'Other',
      'Hearing', 'Filing', 'Meeting', 'Deadline',
    ];
    expect(stored.filter((v) => !(`value.${v}` in en))).toEqual([]);
  });

  it('every slash command has a hint and a template in both languages', () => {
    for (const name of ['summarize', 'timeline', 'deadlines', 'compare', 'draft']) {
      for (const part of ['hint', 'template']) {
        expect(`slash.${name}.${part}` in en).toBe(true);
      }
      expect(pt[`slash.${name}.template` as keyof typeof pt]).toContain('{caret}');
    }
  });
});

describe('panel layout', () => {
  const shell = { left: 0, top: 0, width: 1400, height: 900 };
  const content = { left: 44, width: 1356 };

  it('normalizes bad persisted layouts field by field', () => {
    expect(normalizeLayout(null)).toEqual(DEFAULT_LAYOUT);
    expect(normalizeLayout({ rail: 'middle', chat: 'right' })).toEqual({ rail: 'left', chat: 'right' });
    expect(normalizeLayout({ rail: 'bottom', chat: 42 })).toEqual({ rail: 'bottom', chat: 'left' });
    expect(normalizeLayout({ rail: 'top', chat: 'right' })).toEqual({ rail: 'top', chat: 'right' });
  });

  it('docks the rail to the nearest window edge', () => {
    expect(nearestEdge(shell, 10, 450)).toBe('left');
    expect(nearestEdge(shell, 1390, 450)).toBe('right');
    expect(nearestEdge(shell, 700, 20)).toBe('top');
    expect(nearestEdge(shell, 700, 880)).toBe('bottom');
  });

  it('weighs edges by window size, so a wide window still reaches top and bottom', () => {
    // 100px from the left is 7% across; 100px from the top is 11% down.
    expect(nearestEdge(shell, 100, 100)).toBe('left');
    // 300px from the left is 21% across; 150px from the top is 17% down.
    expect(nearestEdge(shell, 300, 150)).toBe('top');
  });

  it('places the assistant on the half it is dropped on', () => {
    expect(proposeLayout(DEFAULT_LAYOUT, 'chat', shell, content, 1200, 400)).toEqual({ rail: 'left', chat: 'right' });
    expect(proposeLayout(DEFAULT_LAYOUT, 'chat', shell, content, 200, 400)).toEqual(DEFAULT_LAYOUT);
  });

  it('dropping the workspace on the left puts the assistant on the right', () => {
    expect(proposeLayout(DEFAULT_LAYOUT, 'workspace', shell, content, 200, 400).chat).toBe('right');
  });

  it('moving the rail leaves the assistant side alone', () => {
    const l = { rail: 'left' as const, chat: 'right' as const };
    expect(proposeLayout(l, 'rail', shell, content, 700, 10)).toEqual({ rail: 'top', chat: 'right' });
  });

  it('moves with the keyboard, ignoring keys that change nothing', () => {
    expect(keyLayout(DEFAULT_LAYOUT, 'rail', 'ArrowDown')).toEqual({ rail: 'bottom', chat: 'left' });
    expect(keyLayout(DEFAULT_LAYOUT, 'rail', 'ArrowLeft')).toBeNull();
    expect(keyLayout(DEFAULT_LAYOUT, 'chat', 'ArrowRight')).toEqual({ rail: 'left', chat: 'right' });
    expect(keyLayout(DEFAULT_LAYOUT, 'chat', 'ArrowUp')).toBeNull();
    expect(keyLayout(DEFAULT_LAYOUT, 'workspace', 'ArrowLeft')).toEqual({ rail: 'left', chat: 'right' });
    expect(keyLayout(DEFAULT_LAYOUT, 'chat', 'Enter')).toBeNull();
  });

  it('compares layouts by value', () => {
    expect(sameLayout({ rail: 'top', chat: 'left' }, { rail: 'top', chat: 'left' })).toBe(true);
    expect(isDefaultLayout({ rail: 'left', chat: 'left' })).toBe(true);
    expect(isDefaultLayout({ rail: 'top', chat: 'left' })).toBe(false);
  });
});
