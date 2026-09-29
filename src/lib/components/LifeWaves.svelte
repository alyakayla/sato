<script lang="ts">
  import { onMount } from 'svelte';

  /**
   * The empty state for a case's connection graph: Conway's Game of Life on a
   * white field, fed by a sine-shaped front that sweeps across and leaves
   * black waves of cells behind, which break up into gliders, blinkers and
   * still lifes. It never settles, so the box never looks frozen.
   *
   * Drawn on a canvas at ~12 frames a second, paused when off screen or when
   * the window is hidden, and a single still frame for reduced motion. Colours
   * come from the theme (`--surface`, `--ink`): white and black in light.
   */

  let { label = '' }: { label?: string } = $props();

  const CELL = 5;
  const FRAME_MS = 85;

  let canvas = $state<HTMLCanvasElement | null>(null);

  onMount(() => {
    const el = canvas!;
    const ctx = el.getContext('2d')!;
    let cols = 0;
    let rows = 0;
    let grid = new Uint8Array(0);
    let next = new Uint8Array(0);
    let tick = 0;
    let fg = 'black';
    let bg = 'white';
    let timer = 0;
    let visible = true;

    function readColours(): void {
      const cs = getComputedStyle(el);
      fg = cs.getPropertyValue('--ink').trim() || fg;
      bg = cs.getPropertyValue('--surface').trim() || bg;
    }

    function resize(): void {
      const dpr = window.devicePixelRatio || 1;
      const w = el.clientWidth;
      const h = el.clientHeight;
      el.width = Math.max(1, Math.round(w * dpr));
      el.height = Math.max(1, Math.round(h * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const nc = Math.max(8, Math.ceil(w / CELL));
      const nr = Math.max(8, Math.ceil(h / CELL));
      if (nc === cols && nr === rows) return;
      cols = nc;
      rows = nr;
      grid = new Uint8Array(cols * rows);
      next = new Uint8Array(cols * rows);
      seed();
    }

    /** Diagonal bands of live cells, thinned at random: the first waves. */
    function seed(): void {
      for (let y = 0; y < rows; y++) {
        for (let x = 0; x < cols; x++) {
          const band = Math.sin((x + y * 0.6) * 0.22);
          grid[y * cols + x] = band > 0.55 && Math.random() < 0.55 ? 1 : 0;
        }
      }
    }

    /** One generation of Life on a torus, then the sweeping wave front. */
    function step(): void {
      for (let y = 0; y < rows; y++) {
        const up = ((y - 1 + rows) % rows) * cols;
        const mid = y * cols;
        const down = ((y + 1) % rows) * cols;
        for (let x = 0; x < cols; x++) {
          const l = (x - 1 + cols) % cols;
          const r = (x + 1) % cols;
          const n =
            grid[up + l] + grid[up + x] + grid[up + r] +
            grid[mid + l] + grid[mid + r] +
            grid[down + l] + grid[down + x] + grid[down + r];
          const alive = grid[mid + x];
          next[mid + x] = n === 3 || (alive && n === 2) ? 1 : 0;
        }
      }
      [grid, next] = [next, grid];

      // The front: a column moving right, alive along a travelling sine, so
      // each pass lays down a fresh wave for Life to break apart.
      const x = tick % cols;
      for (let y = 0; y < rows; y++) {
        const wave = Math.sin(y * 0.3 + tick * 0.12) + Math.sin(y * 0.07 - tick * 0.05);
        if (wave > 0.9 && Math.random() < 0.7) {
          grid[y * cols + x] = 1;
          grid[y * cols + ((x + 1) % cols)] = 1;
        }
      }
      tick++;
    }

    function draw(): void {
      ctx.fillStyle = bg;
      ctx.fillRect(0, 0, el.clientWidth, el.clientHeight);
      ctx.fillStyle = fg;
      for (let y = 0; y < rows; y++) {
        for (let x = 0; x < cols; x++) {
          if (grid[y * cols + x]) ctx.fillRect(x * CELL, y * CELL, CELL - 1, CELL - 1);
        }
      }
    }

    function loop(): void {
      timer = 0;
      if (!visible || document.hidden) return;
      step();
      // Theme can change at any time; re-read now and then, not every frame.
      if (tick % 24 === 0) readColours();
      draw();
      timer = window.setTimeout(() => requestAnimationFrame(loop), FRAME_MS);
    }

    function start(): void {
      if (!timer && visible && !document.hidden) timer = window.setTimeout(loop, 0);
    }

    readColours();
    resize();
    const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduce) {
      // A still frame that still reads as Life: seed, then let it evolve a bit.
      for (let i = 0; i < 12; i++) step();
      draw();
    } else {
      draw();
      start();
    }

    const ro = new ResizeObserver(() => {
      resize();
      draw();
    });
    ro.observe(el);
    const io = new IntersectionObserver(([e]) => {
      visible = e.isIntersecting;
      if (visible && !reduce) start();
    });
    io.observe(el);
    const onVisibility = () => !reduce && start();
    document.addEventListener('visibilitychange', onVisibility);

    return () => {
      clearTimeout(timer);
      ro.disconnect();
      io.disconnect();
      document.removeEventListener('visibilitychange', onVisibility);
    };
  });
</script>

<div class="life" role="img" aria-label={label}>
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
</div>

<style>
  .life {
    width: 100%;
    height: 100%;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
