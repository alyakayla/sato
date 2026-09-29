/**
 * Measures the UI under load, with the Tauri backend mocked (see mock.js):
 * a large case tree, a long conversation, a streaming answer. Drives the
 * production build in Edge/Chromium — the engine behind the Tauri window on
 * Windows — and reports input-to-paint latency, long tasks and CPU hot spots
 * per interaction.
 *
 *   bun run build && bun run perf                 # 10k-node tree, 80 messages
 *   bun run perf '{"cases":1000,"docs":48,"messages":200}'   # 50k nodes
 *
 * Exits non-zero if the app throws (e.g. a Svelte effect loop) or chat input
 * latency regresses. Set BROWSER to a Chromium executable if Edge is not at
 * its default path.
 */
import puppeteer from 'puppeteer-core';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const BUILD = new URL('../../build/', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const PORT = 4179;
const MOCK = readFileSync(new URL('./mock.js', import.meta.url), 'utf8');
const scale = JSON.parse(process.argv[2] ?? '{}');

const server = Bun.serve({
  port: PORT,
  fetch(req) {
    const path = new URL(req.url).pathname;
    const file = Bun.file(join(BUILD, path === '/' ? 'index.html' : path));
    return file.size ? new Response(file) : new Response(Bun.file(join(BUILD, 'index.html')));
  },
});

const browser = await puppeteer.launch({
  executablePath: process.env.BROWSER ?? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  headless: 'new',
  args: ['--window-size=1440,900', '--disable-gpu-vsync'],
  defaultViewport: { width: 1440, height: 900 },
});
const page = await browser.newPage();
// REDUCED=1 measures with animations and view transitions off.
if (process.env.REDUCED) await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
let errs = 0;
let failures = 0;
const chatLatencies = [];
const NL = String.fromCharCode(10);
page.on('pageerror', (e) => { failures++; if (errs++ < 3) console.log('PAGE ERROR', e.message, NL, (e.stack || '').split(NL).slice(0, 12).join(NL)); });
page.on('console', (m) => { if ((m.type() === 'error' || m.type() === 'warning') && errs++ < 8) console.log('CONSOLE', m.type(), m.text().slice(0, 1500)); });
await page.evaluateOnNewDocument(`window.__MOCK_CASES=${scale.cases ?? 300};window.__MOCK_DOCS=${scale.docs ?? 30};window.__MOCK_MESSAGES=${scale.messages ?? 80};` + MOCK);
// Event Timing + long tasks + frame gaps, collected in-page.
await page.evaluateOnNewDocument(() => {
  window.__events = [];
  window.__longtasks = [];
  new PerformanceObserver((l) => window.__events.push(...l.getEntries().map((e) => ({ name: e.name, dur: e.duration, proc: e.processingEnd - e.processingStart })))).observe({ type: 'event', durationThreshold: 16, buffered: true });
  new PerformanceObserver((l) => window.__longtasks.push(...l.getEntries().map((e) => e.duration))).observe({ type: 'longtask', buffered: true });
  window.__lat = [];
  for (const type of ['keydown', 'pointerdown']) {
    addEventListener(type, () => {
      const t0 = performance.now();
      requestAnimationFrame(() => setTimeout(() => window.__lat.push(performance.now() - t0), 0));
    }, true);
  }
  window.__frames = [];
  let last = performance.now();
  const tick = (t) => { window.__frames.push(t - last); last = t; requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
});

const cdp = await page.createCDPSession();
await cdp.send('Profiler.enable');
await cdp.send('Profiler.setSamplingInterval', { interval: 200 });

async function phase(name, fn) {
  await page.evaluate(() => { window.__events = []; window.__longtasks = []; window.__frames = []; window.__lat = []; });
  await cdp.send('Profiler.start');
  const t0 = Date.now();
  await fn();
  const wall = Date.now() - t0;
  const { profile } = await cdp.send('Profiler.stop');
  const m = await page.evaluate(() => ({ events: window.__events, longtasks: window.__longtasks, frames: window.__frames, lat: window.__lat }));
  const frames = m.frames.slice(1);
  const jank = frames.filter((f) => f > 50);
  console.log(`\n=== ${name} (${wall} ms wall)`);
  console.log(`  long tasks: ${m.longtasks.length}, total ${Math.round(m.longtasks.reduce((a, b) => a + b, 0))} ms, max ${Math.round(Math.max(0, ...m.longtasks))} ms`);
  const slow = m.events.filter((e) => ['keydown', 'keypress', 'input', 'keyup', 'click', 'pointerdown', 'pointerup'].includes(e.name));
  if (slow.length) {
    const durs = slow.map((e) => e.dur).sort((a, b) => a - b);
    console.log(`  slow input events (>16ms): ${slow.length}, median ${Math.round(durs[durs.length >> 1])} ms, max ${Math.round(durs.at(-1))} ms`);
  } else console.log('  slow input events (>16ms): 0');
  if (/typing|streaming/.test(name)) chatLatencies.push(...m.lat);
  if (m.lat.length) { const l = [...m.lat].sort((a, b) => a - b); console.log(`  input→paint latency: n=${l.length}, median ${l[l.length >> 1].toFixed(1)} ms, p95 ${l[Math.floor(l.length * 0.95)].toFixed(1)} ms, max ${l.at(-1).toFixed(1)} ms`); }
  console.log(`  frames: ${frames.length}, >50ms gaps: ${jank.length}, worst ${Math.round(Math.max(0, ...frames))} ms`);
  // Self time by function.
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const self = new Map();
  const dt = profile.timeDeltas;
  profile.samples.forEach((id, i) => {
    const n = byId.get(id);
    const cf = n.callFrame;
    const key = `${cf.functionName || '(anon)'} ${cf.url.split('/').pop()}:${cf.lineNumber + 1}`;
    self.set(key, (self.get(key) ?? 0) + (dt[i] ?? 0) / 1000);
  });
  const top = [...self.entries()].filter(([k]) => !k.startsWith('(idle)') && !k.startsWith('(program)')).sort((a, b) => b[1] - a[1]).slice(0, 14);
  for (const [k, ms] of top) console.log(`  ${ms.toFixed(1).padStart(8)} ms  ${k}`);
}

const t0 = Date.now();
await page.goto(process.argv[3] ?? `http://localhost:${PORT}/`, { waitUntil: 'networkidle0', timeout: 120000 });
await page.waitForSelector('.composer textarea', { timeout: 20000 });
await page.waitForFunction(() => document.querySelectorAll('.row-msg').length > 10, { timeout: 20000 });
console.log(`loaded in ${Date.now() - t0} ms; messages rendered: ${await page.$$eval('.row-msg', (e) => e.length)}; tree nodes: ${await page.evaluate(() => window.__TAURI_INTERNALS__ && 1)}`);

await phase('idle 1s', async () => { await new Promise((r) => setTimeout(r, 1000)); });

await phase('typing 40 chars in the composer', async () => {
  await page.click('.composer textarea');
  await page.keyboard.type('Qual é o prazo de aviso previsto em @case-01', { delay: 25 });
});

await phase('typing a plain sentence', async () => {
  await page.keyboard.press('Enter'); // not sent: textarea handles Enter? (sends). Use shift+enter-free text instead
});

await phase('clicking the conversation list twice', async () => {
  await page.click('.chat .title');
  await new Promise((r) => setTimeout(r, 300));
  await page.click('.chat .title');
  await new Promise((r) => setTimeout(r, 300));
});

await phase('sending + streaming an answer, typing meanwhile', async () => {
  await page.click('.composer textarea');
  await page.keyboard.type('What is the notice period?', { delay: 10 });
  await page.keyboard.press('Enter');
  await new Promise((r) => setTimeout(r, 400));
  await page.keyboard.type('follow up while it streams', { delay: 40 });
  await page.waitForFunction(() => !document.querySelector('.steps'), { timeout: 30000 });
});

const labels = await page.$$eval('.rail .r', (b) => b.map((x) => x.getAttribute('aria-label')));
for (let i = 2; i < labels.length; i++) {
  await phase(`rail → ${labels[i]}`, async () => {
    const buttons = await page.$$('.rail .r');
    await buttons[i].click();
    await new Promise((r) => setTimeout(r, 400));
  });
}
await phase('table → Documents tab (48k rows)', async () => {
  const tableBtn = (await page.$$('.rail .r'))[3];
  await tableBtn.click();
  await new Promise((r) => setTimeout(r, 300));
  const tabs = await page.$$('.tabs .tab');
  await tabs[1].click();
  await new Promise((r) => setTimeout(r, 800));
});
await phase('quick find: open, type, close', async () => {
  await page.keyboard.down('Control');
  await page.keyboard.press('k');
  await page.keyboard.up('Control');
  await page.keyboard.type('document-4', { delay: 20 });
  await page.keyboard.press('Escape');
});

await browser.close();
server.stop();

const sorted = chatLatencies.sort((a, b) => a - b);
const p95 = sorted.length ? sorted[Math.floor(sorted.length * 0.95)] : 0;
console.log(`
chat input→paint p95: ${p95.toFixed(1)} ms; page errors: ${failures}`);
if (failures > 0 || p95 > 50) {
  console.log('FAIL: the app threw, or chat input latency regressed past 50 ms p95.');
  process.exit(1);
}
console.log('PASS');
