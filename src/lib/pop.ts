/**
 * "Pop!" — shows that something was deleted, right where it was.
 *
 * Every on-screen representation of an artifact carries a `data-artifact` key
 * (`document:<id>`, `case:<id>`, `person:<id>`, `person:<id>:<case>`, …).
 * `popArtifact` swells each visible one slightly and shrinks it away while a
 * small ring and a spray of dots burst from its centre; the caller then
 * refreshes, and the list closes the gap. When nothing is on screen (a
 * document deleted from its own page), the burst plays where the user last
 * clicked.
 *
 * A key also matches everything nested under it: `person:p1` pops the person
 * in the directory and on every case, `person:p1:c2` only on that case.
 */

/** The item's exit. Long enough to read as "it went", not a flicker. */
const DURATION = 560;

/** Where the user last pressed — the fallback origin for a burst. */
let lastPointer: { x: number; y: number } | null = null;
if (typeof window !== 'undefined') {
  window.addEventListener('pointerdown', (e) => (lastPointer = { x: e.clientX, y: e.clientY }), true);
}

function reducedMotion(): boolean {
  return matchMedia('(prefers-reduced-motion: reduce)').matches;
}

function visible(el: Element): DOMRect | null {
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return null;
  if (r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth) return null;
  return r;
}

/** A ring and eight dots flying out from (x, y), removed when done. */
function burst(x: number, y: number, size: number): void {
  const root = document.createElement('div');
  root.className = 'pop-burst';
  root.style.left = `${x}px`;
  root.style.top = `${y}px`;
  root.style.setProperty('--pop-size', `${Math.max(18, Math.min(size, 72))}px`);
  const ring = document.createElement('span');
  ring.className = 'pop-ring';
  root.appendChild(ring);
  for (let i = 0; i < 8; i++) {
    const dot = document.createElement('span');
    dot.className = 'pop-dot';
    dot.style.setProperty('--pop-angle', `${i * 45 + 22.5}deg`);
    root.appendChild(dot);
  }
  document.body.appendChild(root);
  setTimeout(() => root.remove(), 900);
}

function escapeAttr(s: string): string {
  return s.replace(/["\\]/g, '\\$&');
}

/**
 * Pops every visible representation of `key`, and resolves once the
 * animation has played, so the caller can refresh and let the item go.
 */
export async function popArtifact(key: string): Promise<void> {
  if (reducedMotion()) return;
  const k = escapeAttr(key);
  const els = [...document.querySelectorAll<HTMLElement>(`[data-artifact="${k}"], [data-artifact^="${k}:"]`)];
  const shown = els.map((el) => [el, visible(el)] as const).filter((p): p is readonly [HTMLElement, DOMRect] => !!p[1]);

  if (shown.length === 0) {
    if (lastPointer) burst(lastPointer.x, lastPointer.y, 32);
    await new Promise((r) => setTimeout(r, DURATION));
    return;
  }

  const animations = shown.map(([el, r]) => {
    burst(r.left + r.width / 2, r.top + r.height / 2, Math.min(r.width, r.height) * 1.6);
    const anim = el.animate(
      [
        { transform: 'scale(1)', opacity: 1 },
        { transform: 'scale(1.05)', opacity: 1, offset: 0.3 },
        { transform: 'scale(0.6)', opacity: 0 },
      ],
      { duration: DURATION, easing: 'cubic-bezier(.22, 1, .36, 1)', fill: 'forwards' },
    );
    return anim.finished.catch(() => {});
  });
  await Promise.all(animations);
}

/** Clears a finished pop from anything still on screen, without animating. */
export function clearPop(key: string): void {
  const k = escapeAttr(key);
  for (const el of document.querySelectorAll<HTMLElement>(`[data-artifact="${k}"], [data-artifact^="${k}:"]`)) {
    for (const a of el.getAnimations()) a.cancel();
  }
}

/** The reverse, for a rollback: the item settles back into place. */
export function unpopArtifact(key: string): void {
  const k = escapeAttr(key);
  for (const el of document.querySelectorAll<HTMLElement>(`[data-artifact="${k}"], [data-artifact^="${k}:"]`)) {
    for (const a of el.getAnimations()) a.cancel();
    if (reducedMotion()) continue;
    el.animate(
      [
        { transform: 'scale(0.85)', opacity: 0 },
        { transform: 'scale(1.02)', opacity: 1, offset: 0.7 },
        { transform: 'scale(1)', opacity: 1 },
      ],
      { duration: 420, easing: 'cubic-bezier(.22, 1, .36, 1)' },
    );
  }
}
