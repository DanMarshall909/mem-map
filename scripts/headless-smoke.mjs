// Slow lane: built frontend + real Rust controller/sampler, no desktop UI.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launchHeadless, requireHeadless } from './headless-browser.mjs';
const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const require = createRequire(path.join(root, 'frontend', 'package.json'));
const { chromium } = require('playwright-core');
const executable = process.argv[2] && path.resolve(process.argv[2]);
assert(executable, 'Use scripts/smoke.ps1 to build and supply the headless Rust host');
const output = path.resolve(process.argv[3] || path.join(process.env.LOCALAPPDATA || root, 'mem-map-build', 'evidence', `headless-${Date.now()}`));
await mkdir(output, { recursive: true });
const hash = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const beforeHash = await hash(executable);
const started = new Date();
const checks = [], effects = [], pending = new Map();
let browser, page, server, child, failure, stderr = '', sequence = 0, hostExit, browserUrl;
let frontendAssets = {};
const check = (name, details = true) => { checks.push({ name, details }); console.log(`PASS ${name}`); };
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const until = async (predicate, label, timeout = 15000) => {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await predicate()) return;
    if (child.exitCode !== null) throw Error(`Rust host exited ${child.exitCode}: ${label}\n${stderr}`);
    await sleep(100);
  }
  throw Error(`Timed out: ${label}`);
};
function request(command, args) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(`Rust command timed out: ${command}`)); }, 10000);
    pending.set(id, { resolve, reject, timer });
    child.stdin.write(JSON.stringify({ id, command, args }) + '\n', error => {
      if (error) { clearTimeout(timer); pending.delete(id); reject(error); }
    });
  });
}
async function invoke(command, args = {}) {
  const result = await request(command, args);
  if (result.effect) {
    // These private adapters deliberately never touch clipboard or native dialogs.
    if (result.effect.kind === 'snapshot-copy') await writeFile(path.join(output, 'paused.csv'), result.effect.text);
    else assert.equal(result.effect.kind, 'copy');
    effects.push(result.effect);
  }
  return result.value;
}
// Independent CSV consumer, including quoted commas/newlines and escaped quotes.
function parseCsv(csv) {
  const rows = []; let row = [], field = '', quoted = false;
  for (let i = 0; i < csv.length; i++) {
    const c = csv[i];
    if (c === '"') { if (quoted && csv[i + 1] === '"') { field += '"'; i++; } else quoted = !quoted; }
    else if (!quoted && c === ',') { row.push(field); field = ''; }
    else if (!quoted && c === '\n') { row.push(field.replace(/\r$/, '')); rows.push(row); row = []; field = ''; }
    else field += c;
  }
  assert(!quoted, 'CSV has an unterminated quoted field');
  if (field || row.length) { row.push(field); rows.push(row); }
  const headers = rows.shift(); assert(headers?.length > 1);
  return rows.map(values => { assert.equal(values.length, headers.length); return Object.fromEntries(headers.map((name, i) => [name, values[i]])); });
}
async function loadAssets(directory, prefix = '') {
  const assets = new Map();
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const relative = `${prefix}/${entry.name}`, full = path.join(directory, entry.name);
    if (entry.isDirectory()) for (const [key, value] of await loadAssets(full, relative)) assets.set(key, value);
    else if (entry.isFile()) assets.set(relative, await readFile(full));
    else throw Error(`Unexpected built asset type: ${relative}`);
  }
  return assets;
}
try {
  const assets = await loadAssets(path.join(root, 'frontend', 'dist'));
  assert(assets.has('/index.html'), 'Build the frontend before the headless smoke');
  frontendAssets = Object.fromEntries([...assets].map(([name, bytes]) => [name, createHash('sha256').update(bytes).digest('hex')]));
  child = spawn(executable, [path.join(output, 'preferences.ini')], { cwd: path.dirname(executable), windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  hostExit = new Promise(resolve => { child.once('exit', (code, signal) => resolve({ code, signal })); child.once('error', error => resolve({ error: String(error) })); });
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-65536); });
  child.stdin.on('error', () => {});
  const responses = createInterface({ input: child.stdout });
  responses.on('line', line => {
    try {
      const response = JSON.parse(line), item = pending.get(response.id);
      if (!item) return;
      pending.delete(response.id); clearTimeout(item.timer);
      if (response.error) item.reject(Error(response.error)); else item.resolve(response.result);
    } catch (error) { for (const item of pending.values()) { clearTimeout(item.timer); item.reject(error); } pending.clear(); }
  });
  child.once('close', () => { for (const item of pending.values()) { clearTimeout(item.timer); item.reject(Error(`Rust host closed: ${stderr}`)); } pending.clear(); });
  server = createServer((req, res) => {
    const pathname = new URL(req.url, 'http://localhost').pathname;
    const key = pathname === '/' ? '/index.html' : pathname;
    const bytes = req.method === 'GET' && assets.get(key);
    if (!bytes) { res.writeHead(404); res.end(); return; }
    const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml' };
    res.writeHead(200, { 'Content-Type': types[path.extname(key)] || 'application/octet-stream', 'Cache-Control': 'no-store' }); res.end(bytes);
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  browser = await launchHeadless(chromium);
  check('Browser is actually headless', await requireHeadless(browser));
  page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, hasTouch: true });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.exposeFunction('__memmapTestInvoke', invoke);
  await page.addInitScript(() => { window.__TAURI_INTERNALS__ = { invoke: (command, args = {}) => window.__memmapTestInvoke(command, args) }; });
  browserUrl = `http://127.0.0.1:${server.address().port}/`;
  await page.goto(browserUrl);
  await page.locator('.dashboard[data-ready="true"]').waitFor();
  const view = () => invoke('get_view');
  await until(async () => (await view()).table.rows.length > 0, 'real Rust first frame');
  const first = await view();
  assert(first.overview.capacity !== '0'); assert(first.selected); assert.equal(typeof first.selected.private.value, 'string');
  check('Real collector and precision-safe stdio boundary', { rows: first.table.rows.length, status: first.collection.status });
  const grid = page.locator('.process-grid'); const inspector = page.locator('.inspector-scroll');
  await inspector.waitFor();
  const ownedRow = first.table.rows.find(row => row.name.toLowerCase() === 'headless-test-host.exe');
  assert(ownedRow, 'Live sampler must observe the owned headless Rust host');
  await invoke('dispatch', { action: { type: 'select_row', id: ownedRow.id } });
  await until(async () => (await page.getByRole('group', { name: 'Full executable path', exact: true }).getAttribute('aria-description')).toLowerCase().endsWith('headless-test-host.exe'), 'owned host selection rendered');
  await page.screenshot({ path: path.join(output, 'initial.png') });
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
  assert(await page.evaluate(() => document.documentElement.scrollHeight <= innerHeight + 1));
  check('Root fits the actual window');
  await page.getByRole('button', { name: 'Pause display', exact: true }).click();
  await until(async () => (await view()).paused, 'pause');
  const paused = await view();
  await writeFile(path.join(output, 'paused-view.json'), JSON.stringify(paused, null, 2));
  await until(async () => (await view()).live_at > paused.live_at, 'recording advances while display is paused');
  const afterPause = await view();
  assert.equal(afterPause.displayed_at, paused.displayed_at);
  assert.equal(afterPause.displayed_unix_ms, paused.displayed_unix_ms);
  assert.deepEqual(afterPause.history_viewport, paused.history_viewport);
  check('Paused display remains frozen while recording continues');
  // Only the adapter changes: data still comes from the real Rust controller.
  await page.getByRole('button', { name: /^Copy (all )?paths?$/i }).first().click();
  await until(() => effects.some(effect => effect.kind === 'copy'), 'headless copy result');
  assert.equal(effects.find(effect => effect.kind === 'copy').text, paused.selected.paths.join('\r\n'));
  check('Copy button preserves full original paths through an isolated adapter');
  await page.getByRole('button', { name: 'Copy snapshot to clipboard', exact: true }).click();
  await until(() => effects.some(effect => effect.kind === 'snapshot-copy'), 'headless snapshot clipboard');
  await page.getByRole('status').filter({hasText:'Copied snapshot as CSV'}).waitFor({state:'visible'});
  await assert.rejects(invoke('copy_snapshot', {text:'untrusted payload'}), /Expected empty command arguments/);
  await assert.rejects(invoke('export_csv'), /Unsupported test command/);
  const csv = await readFile(path.join(output, 'paused.csv'), 'utf8');
  const records = parseCsv(csv);
  assert(records.length > 0);
  assert(records.every(row => row.elapsed_seconds === String(paused.displayed_at) && row.snapshot_unix_ms === paused.displayed_unix_ms));
  assert.equal(new Set(records.map(row => [row.pid,row.creation_filetime,row.provisional_generation].join('/'))).size, records.length);
  assert(csv.includes('private_working_set_bytes'));
  check('Clipboard CSV preserves frozen timestamp and distinct identities', { bytes: csv.length, records: records.length });
  await page.getByRole('button', { name: /Resume display/, exact: true }).click();
  await until(async () => !(await view()).paused, 'resume');
  const filter = page.getByRole('searchbox', { name: /Filter/ });
  await page.keyboard.press('Control+f'); assert(await filter.evaluate(el => el === document.activeElement));
  await filter.fill('definitely-no-memmap-process-match');
  await until(async () => (await view()).table.rows.length === 0, 'filter real model');
  assert((await view()).table.selection_hidden); await filter.press('Escape');
  await until(async () => (await view()).settings.filter === '', 'clear filter');
  check('Keyboard filter and hidden selected identity');
  for (const [grouping, value] of [['Path', 'path'], ['Process tree', 'tree'], ['Individual processes', 'process'], ['Family', 'family'], ['Name', 'name']]) {
    const radio = page.getByRole('radio', { name: grouping, exact: true });
    await radio.click();
    await until(async () => { const frame = await view(); return frame.settings.grouping === value && frame.table.rows.length > 0 && await radio.isChecked(); }, `group ${grouping}`);
  }
  const nameGrouping = page.getByRole('radio', { name: 'Name', exact: true });
  await nameGrouping.focus(); await nameGrouping.press('ArrowRight');
  await until(async () => (await view()).settings.grouping === 'path' && await page.getByRole('radio', {name:'Path',exact:true}).isChecked(), 'native arrow-key grouping');
  assert(await page.getByRole('group', {name:'Group by',exact:true}).locator('label').evaluateAll(labels => labels.every(label => label.title && !label.textContent && label.querySelector('svg[aria-hidden=true]'))));
  await nameGrouping.click(); await until(async () => (await view()).settings.grouping === 'name', 'restore name grouping');
  check('All five icon groupings retain tooltips, radio semantics and native arrow-key commands');
  const intervals = page.getByRole('group', {name:'Change over',exact:true});
  for (const seconds of [60,300,900,1800]) {
    await intervals.getByRole('radio', {name:`${seconds / 60}m`,exact:true}).check();
    await until(async () => (await view()).settings.change_seconds === seconds, 'change interval');
  }
  await intervals.getByRole('radio', {name:'5m',exact:true}).press('ArrowRight');
  await until(async () => (await view()).settings.change_seconds === 900, 'native interval arrow key');
  await intervals.getByRole('radio', {name:'5m',exact:true}).check();
  await until(async () => (await view()).settings.change_seconds === 300, 'restore interval');
  const follow = page.getByRole('checkbox', {name:'Follow latest',exact:true});
  await follow.uncheck(); await until(async () => !(await view()).history_viewport.follow, 'follow off');
  await follow.press('Space'); await until(async () => (await view()).history_viewport.follow, 'follow native keyboard on');
  check('Change interval segments and Follow latest icon preserve native keyboard commands and backend state');
  const dpr = await page.evaluate(() => devicePixelRatio);
  const layout = async () => page.evaluate(() => {
    const rect = selector => { const el = document.querySelector(selector), box = el.getBoundingClientRect(); return { top: box.top, bottom: box.bottom, height: el.clientHeight, width: el.clientWidth }; };
    return { reflow: document.querySelector('.dashboard').classList.contains('reflow'), width: innerWidth, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, scrollHeight: document.documentElement.scrollHeight, process: rect('.process-grid'), overview: rect('.overview-panel'), inspector: rect('.inspector-scroll'), footer: rect('.app-footer') };
  });
  const layoutProofs = [], pathLayoutProofs = [];
  const usableLayout = async label => {
    const metrics = await layout(); layoutProofs.push({ label, ...metrics });
    await writeFile(path.join(output, 'layout.json'), JSON.stringify(layoutProofs, null, 2));
    assert(metrics.scrollWidth <= metrics.width + 1, `${label}: no horizontal root overflow`);
    assert(metrics.process.height >= 140, `${label}: a usable process table viewport`);
    assert(metrics.inspector.height >= 140, `${label}: a usable inspector viewport`);
    assert(metrics.inspector.top >= metrics.overview.bottom - 1, `${label}: inspector follows overview without overlap`);
    assert(metrics.footer.top >= metrics.inspector.bottom - 1, `${label}: footer follows the workspace without overlap`);
    if (!metrics.reflow) assert(metrics.scrollHeight <= metrics.height + 1, `${label}: desktop root fits the window`);
    const selectedPath = (await view()).selected?.paths;
    assert.equal(selectedPath?.length, 1, 'Rendered path check needs one actual full executable path');
    const pathProof = await page.locator('.path-preview').evaluate((preview, original) => {
      const tail = preview.querySelector('.path-filename'), prefix = preview.querySelector('.path-prefix');
      if (!tail || !prefix) return { protectedTail: false };
      const box = preview.getBoundingClientRect(); const range = document.createRange(); range.selectNodeContents(tail);
      const rects = [...range.getClientRects()].map(r => ({left:r.left,right:r.right,top:r.top,bottom:r.bottom}));
      const leading = document.createRange(); leading.setStart(prefix.firstChild, 0); leading.setEnd(prefix.firstChild, Math.min(3, prefix.textContent.length));
      return { protectedTail: true, value: preview.textContent, tail:tail.textContent, original,
        visible: getComputedStyle(preview).visibility !== 'hidden' && rects.length > 0 && rects.every(r => r.left >= box.left - 1 && r.right <= box.right + 1 && r.top >= box.top - 1 && r.bottom <= box.bottom + 1),
        beginningVisible: leading.getBoundingClientRect().width <= prefix.clientWidth + 1 };
    }, selectedPath[0]);
    assert(pathProof.protectedTail, `${label}: executable filename must have a protected visible region`);
    assert.equal(pathProof.value, selectedPath[0]);
    assert.equal(pathProof.tail, selectedPath[0].slice(Math.max(selectedPath[0].lastIndexOf('\\'), selectedPath[0].lastIndexOf('/'))));
    assert(pathProof.visible && pathProof.beginningVisible, `${label}: original beginning and complete executable filename must remain painted inside the path field`);
    assert.equal(await inspector.locator('input[readonly], textarea, [contenteditable=true]').count(), 0, 'Executable path is static text');
    const pathText = page.getByRole('group', { name: 'Full executable path', exact: true });
    const pathHeight = await pathText.evaluate(el => el.clientHeight);
    await pathText.focus(); assert.equal(await pathText.textContent(), selectedPath[0]);
    assert.equal(await pathText.getAttribute('aria-description'), selectedPath[0]);
    assert.equal(await pathText.evaluate(el => getComputedStyle(el).userSelect), 'text');
    assert.equal(await pathText.evaluate(el => el.clientHeight), pathHeight, 'Focus preserves the static path layout');
    pathLayoutProofs.push({ label, ...pathProof });
    await writeFile(path.join(output, 'path-layout.json'), JSON.stringify(pathLayoutProofs, null, 2));
  };
  for (const [width, height] of [[1280, 800], [980, 740], [760, 540], [1440, 900]]) {
    await page.setViewportSize({ width, height });
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await usableLayout(`viewport ${width}x${height}`);
    await page.screenshot({ path: path.join(output, `window-${width}x${height}.png`) });
  }
  check('Headless viewport resizing retains root containment', { dpr });
  await page.keyboard.press('Control++');
  await until(async () => (await view()).settings.ui_zoom === 125, 'Global zoom after path blur');
  const beforeColumns = (await view()).settings.column_widths;
  const resizeColumn = await page.getByRole('separator', { name: 'Resize PID column', exact: true }).boundingBox(); assert(resizeColumn);
  await page.mouse.move(resizeColumn.x + resizeColumn.width / 2, resizeColumn.y + resizeColumn.height / 2);
  await page.mouse.down(); await page.mouse.move(resizeColumn.x + resizeColumn.width / 2 + 7, resizeColumn.y + resizeColumn.height / 2); await page.mouse.up();
  await until(async () => (await view()).settings.column_widths[1] !== beforeColumns[1], '125% scaled column resize');
  assert((await view()).settings.column_widths.every(Number.isInteger));
  check('125% pointer column resize preserves integral backend preferences');
  await page.screenshot({ path: path.join(output, 'zoomed.png') });
  await page.getByRole('combobox', { name: /View scale/ }).selectOption('200');
  await until(async () => (await view()).settings.ui_zoom === 200, '200 percent application zoom');
  await page.waitForFunction(() => parseFloat(getComputedStyle(document.documentElement).fontSize) === 28);
  await page.screenshot({ path: path.join(output, 'zoom200.png') });
  await usableLayout('200% application zoom');
  check('Executable path keeps its visible beginning and filename across window sizes and zoom');
  await page.keyboard.press('Control+0');
  await until(async () => (await view()).settings.ui_zoom === 100, 'Global zoom reset after path blur');
  check('Application zoom shortcuts');
  const accessibleRow = grid.locator('[data-row-id][aria-expanded]').first();
  await accessibleRow.click();
  if (await accessibleRow.getAttribute('aria-expanded') === 'false') await accessibleRow.press('ArrowRight');
  await until(async () => await accessibleRow.getAttribute('aria-expanded') === 'true' && await accessibleRow.getAttribute('aria-selected') === 'true', 'accessible selected and expanded process row');
  const accessibility = await page.context().newCDPSession(page);
  const { nodes } = await accessibility.send('Accessibility.getFullAXTree');
  const rowNodes = nodes.filter(node => node.role?.value === 'row' && !node.ignored);
  assert(nodes.some(node => node.role?.value === 'treegrid' && node.name?.value === 'Processes'));
  assert(rowNodes.some(node => node.properties?.some(p => p.name === 'selected' && p.value.value === true)
    && node.properties.some(p => p.name === 'expanded' && p.value.value === true)));
  check('Chromium accessibility exposes selected and expanded process rows', { rows: rowNodes.length });
  await accessibility.detach();
  await assert.rejects(invoke('not_a_command', {}), /Unsupported test command/);
  const beforeInvalid = (await view()).settings.ui_zoom;
  await assert.rejects(invoke('dispatch', {action:{type:'set_ui_zoom',value:150,unexpected:true}}), /unknown field/);
  assert.equal((await view()).settings.ui_zoom, beforeInvalid);
  check('Invalid test-boundary commands fail without changing the controller');
  await page.setViewportSize({ width: 585, height: 774 });
  await page.getByRole('combobox', { name: /View scale/ }).selectOption('75');
  await until(async () => (await view()).settings.ui_zoom === 75, 'compact browser text scale');
  const compactChart = page.locator('.overview-panel .memory-chart');
  await compactChart.locator('.chart-label-toggle').waitFor({ state: 'visible' });
  await page.mouse.move(0, 0); await page.locator('.system-summary').click();
  await compactChart.locator('.chart-labels').waitFor({ state: 'hidden' });
  assert.equal(await compactChart.locator('.band-labels').isVisible(), false, 'Idle overview displays no legend');
  const compactDrawing = compactChart.locator('svg[role=img]');
  const plotBefore = await compactDrawing.getAttribute('viewBox');
  const compactProof = await compactDrawing.evaluate(svg => {
    const line = svg.querySelector('line'); const box = svg.viewBox.baseVal;
    return { fraction: (Number(line.getAttribute('x2')) - Number(line.getAttribute('x1'))) / box.width };
  });
  assert(compactProof.fraction > .85, 'Compact graph must use almost all available width');
  await compactDrawing.hover();
  await compactChart.locator('.band-labels').waitFor({ state: 'visible' });
  assert.equal(await compactChart.locator('.legend-share,.legend-change').count(), 0, 'Normal hover legend shows only names and sizes');
  await compactChart.locator('.chart-labels').waitFor({ state: 'visible' });
  const overlayProof = await compactChart.evaluate(chart => {
    const plot = chart.querySelector('svg[role=img]').getBoundingClientRect(), labels = chart.querySelector('.chart-labels').getBoundingClientRect();
    return { overlap: Math.max(0, Math.min(plot.right, labels.right) - Math.max(plot.left, labels.left)) * Math.max(0, Math.min(plot.bottom, labels.bottom) - Math.max(plot.top, labels.top)),
      insideViewport: labels.left >= 0 && labels.right <= innerWidth && labels.top >= 0 && labels.bottom <= innerHeight };
  });
  assert.equal(overlayProof.overlap, 0, 'When space is available, revealed labels must not block the plot pointer target');
  assert(overlayProof.insideViewport, 'Revealed chart labels remain readable inside the viewport');
  assert.equal(await compactDrawing.getAttribute('viewBox'), plotBefore, 'Hover cannot change chart pointer coordinates');
  await page.screenshot({ path: path.join(output, 'compact-labels.png') });
  await page.mouse.move(0, 0); await page.locator('.system-summary').click();
  await compactChart.locator('.chart-labels').waitFor({ state: 'hidden' });
  await page.screenshot({ path: path.join(output, 'compact-idle.png') });
  await compactChart.focus(); await compactChart.locator('.chart-labels').waitFor({ state: 'visible' });
  await compactChart.press('Escape'); await compactChart.locator('.chart-labels').waitFor({ state: 'hidden' });
  const labelToggleBounds = await compactChart.locator('.chart-label-toggle').boundingBox(); assert(labelToggleBounds);
  await page.touchscreen.tap(labelToggleBounds.x + labelToggleBounds.width / 2, labelToggleBounds.y + labelToggleBounds.height / 2);
  await compactChart.locator('.chart-labels').waitFor({ state: 'visible' });
  await page.touchscreen.tap(labelToggleBounds.x + labelToggleBounds.width / 2, labelToggleBounds.y + labelToggleBounds.height / 2);
  await compactChart.locator('.chart-labels').waitFor({ state: 'hidden' });
  const selectedBeforeInfoEnter = (await view()).selected?.id;
  await compactChart.locator('.chart-label-toggle').focus(); await page.keyboard.press('Enter');
  await compactChart.locator('.chart-labels').waitFor({ state: 'visible' });
  assert.equal((await view()).selected?.id, selectedBeforeInfoEnter, 'Enter activates the focused info button instead of process navigation');
  await page.keyboard.press('Enter'); await compactChart.locator('.chart-labels').waitFor({ state: 'hidden' });
  check('585x774 charts reveal labels by hover, keyboard or tap without shrinking or obstructing the plot', { ...compactProof, ...overlayProof });
  const historyDrawing = page.locator('.inspector-scroll .memory-chart > svg[role=img]');
  await historyDrawing.scrollIntoViewIfNeeded(); await historyDrawing.hover();
  const historyBounds = await historyDrawing.boundingBox(); assert(historyBounds);
  await page.mouse.move(historyBounds.x + historyBounds.width * .45, historyBounds.y + historyBounds.height * .7);
  await page.mouse.down(); await page.mouse.move(historyBounds.x + historyBounds.width * .65, historyBounds.y + historyBounds.height * .7); await page.mouse.up();
  await until(async () => (await view()).history_viewport.manual_end !== null && !(await view()).history_viewport.follow, 'compact history drag');
  await page.getByRole('button', { name: 'Reset view', exact: true }).click();
  await until(async () => (await view()).history_viewport.manual_end === null && (await view()).history_viewport.follow, 'reset compact history drag');
  check('Compact history remains draggable while labels are revealed');
  const focusedRow = grid.locator('[data-row-id]').first(); await focusedRow.focus();
  const flatProof = await focusedRow.evaluate(row => ({
    background: getComputedStyle(row).backgroundImage, focus: document.activeElement === row,
    outline: parseFloat(getComputedStyle(row).outlineWidth), rowLine: parseFloat(getComputedStyle(row).borderBottomWidth),
    obsoleteBars: document.querySelectorAll('.memory-bar, .change-bar').length,
    panels: [...document.querySelectorAll('.panel, .process-grid, .legend-entry, .segments span')].every(element => parseFloat(getComputedStyle(element).borderTopWidth) === 0)
  }));
  assert(flatProof.background.includes('linear-gradient') && flatProof.focus && flatProof.outline >= 1);
  assert.equal(flatProof.rowLine, 0); assert.equal(flatProof.obsoleteBars, 0); assert(flatProof.panels);
  check('Flat panels and row-wide fills preserve visible focus and exact numeric cells', flatProof);
  const processState = await view();
  const expandOverview = page.getByRole('button', { name: 'Expand overview', exact: true });
  await expandOverview.focus(); await page.keyboard.press('Enter');
  const backToProcesses = page.getByRole('button', { name: 'Back to processes', exact: true });
  await backToProcesses.waitFor({ state: 'visible' });
  assert(await backToProcesses.evaluate(button => document.activeElement === button), 'View switch keeps a visible keyboard focus target');
  assert.equal(await page.locator('.process-panel').isVisible(), false);
  assert.equal(await page.locator('.inspector-scroll').isVisible(), false);
  const mainLayouts = [];
  for (const [width, height, scale] of [[585, 774, 75], [585, 774, 100], [585, 774, 125], [1440, 900, 100]]) {
    await page.setViewportSize({ width, height });
    await page.getByRole('combobox', { name: /View scale/ }).selectOption(String(scale));
    await until(async () => (await view()).settings.ui_zoom === scale && await page.evaluate(target => parseFloat(getComputedStyle(document.documentElement).fontSize) === 14 * target / 100, scale), 'main overview scale');
    await until(async () => page.locator('.dashboard').evaluate(root => {
      const workspace = root.querySelector('.workspace').getBoundingClientRect(), panel = root.querySelector('.overview-panel').getBoundingClientRect();
      return root.classList.contains('overview-expanded') && panel.width >= workspace.width * .98 && panel.height >= workspace.height * .95 && panel.height <= workspace.height + 2;
    }), 'main overview occupies the available workspace at each layout breakpoint');
    const geometry = await page.locator('.dashboard').evaluate(root => {
      const plot = root.querySelector('.overview-panel .memory-chart > svg').getBoundingClientRect();
      return { width: innerWidth, height: innerHeight, plotWidth: plot.width, plotHeight: plot.height, contained: document.documentElement.scrollWidth <= innerWidth + 1 && document.documentElement.scrollHeight <= innerHeight + 1 };
    });
    assert(geometry.contained, 'Main overview remains within the root viewport');
    assert(geometry.plotWidth > width * .85 && geometry.plotHeight > 200, 'The main plot uses expanded width and height');
    mainLayouts.push({ ...geometry, scale });
  }
  if (!(await view()).paused) {
    await page.getByRole('button', { name: 'Pause display', exact: true }).click();
    await until(async () => (await view()).paused, 'freeze detailed inspection');
  }
  const detailFrame = await view(), detailChart = page.locator('.overview-panel .memory-chart');
  await until(async () => detailChart.evaluate(chart => chart.classList.contains('chart-detailed')), 'roomy overview details');
  const targetIndex = Math.floor(detailFrame.overview.samples.length / 2); assert(targetIndex > 0);
  const targetSample = detailFrame.overview.samples[targetIndex];
  const detailGeometry = await detailChart.locator('svg[role=img]').evaluate(svg => {
    const line = svg.querySelector('line'), box = svg.getBoundingClientRect();
    return { left: Number(line.getAttribute('x1')), right: Number(line.getAttribute('x2')), viewBox: svg.getAttribute('viewBox'), width: svg.viewBox.baseVal.width, screenLeft: box.left, screenTop: box.top, screenWidth: box.width, screenHeight: box.height };
  });
  const sampleX = sample => detailGeometry.left + (sample.at - detailFrame.overview.start) / Math.max(1, detailFrame.overview.end - detailFrame.overview.start) * (detailGeometry.right - detailGeometry.left);
  await page.mouse.move(detailGeometry.screenLeft + sampleX(targetSample) / detailGeometry.width * detailGeometry.screenWidth, detailGeometry.screenTop + detailGeometry.screenHeight * .55);
  await until(async () => (await detailChart.locator('.chart-description').innerText()).startsWith('Inspecting'), 'pointer sample details');
  const assertSampleDetails = async sample => {
    for (let index = 0; index < detailFrame.overview.layers.length; index++) {
      const amount = sample.layers?.[index] ?? null, title = await detailChart.locator('.band-label title').nth(index).textContent();
      assert(title.includes(`${amount ?? 'Unavailable'} raw bytes`), 'Direct label reads the inspected Rust sample, not the latest frame');
      if (amount !== null && Number(sample.capacity) > 0) assert((await detailChart.locator('.band-label').nth(index).textContent()).includes(`${(Math.round(Number(amount) / Number(sample.capacity) * 1000) / 10).toFixed(1)}%`));
    }
    assert((await detailChart.locator('.chart-description').innerText()).includes(`${sample.measured} of ${sample.observed} processes measured`));
    assert(Math.abs(Number(await detailChart.locator('.chart-cursor line').getAttribute('x1')) - sampleX(sample)) < .01, 'The cursor snaps to the actual inspected capture time');
    assert.equal(await detailChart.locator('.chart-cursor circle').count(), sample.used === null ? 0 : 1);
  };
  const cornerProof = await detailChart.locator('svg[role=img]').evaluate(svg => {
    const grid = svg.querySelector('line'), labels = [...svg.querySelectorAll('.band-label')].map(label => {
      const rect = label.querySelector('rect');
      return { x: Number(rect.getAttribute('x')), y: Number(rect.getAttribute('y')), width: Number(rect.getAttribute('width')), height: Number(rect.getAttribute('height')), textRight: Math.max(...[...label.querySelectorAll('text')].map(text => text.getBBox().x + text.getBBox().width)), cells: [...label.querySelectorAll('text')].map(text => ({ kind: text.getAttribute('class'), x: Number(text.getAttribute('x')), y: Number(text.getAttribute('y')), text: text.textContent })) };
    });
    const ages = [...svg.querySelectorAll('.chart-annotations text')].map(text => text.textContent).filter(text => /^\d+[smh](?: \d+[sm])?$/.test(text));
    return { labels, left: Number(grid.getAttribute('x1')), top: Math.min(...[...svg.querySelectorAll('line[y1]')].map(line => Number(line.getAttribute('y1')))), ages, width: svg.viewBox.baseVal.width };
  });
  assert.equal(cornerProof.ages.at(-1), '0s', 'Displayed right endpoint is zero seconds, including paused views');
  assert.equal(cornerProof.labels.length, detailFrame.overview.layers.length);
  cornerProof.labels.forEach((label, index, labels) => {
    assert(Math.abs(label.x - cornerProof.left - 10) < .1, 'Legend overlays the top-left corner');
    assert(label.textRight <= label.x + label.width + 1, 'Legend text fits its background');
    assert.equal(label.cells.length, 4); assert.equal(new Set(label.cells.map(cell => cell.y)).size, 1, 'Each band is one line');
    assert([...label.cells[0].text].length <= 20, 'Displayed names are at most twenty characters');
    assert.equal(label.cells[3].text, '\u2014', 'A newly started real host has no five-minute baseline');
    if (index) { assert(label.y >= labels[index-1].y + labels[index-1].height, 'Legend rows do not overlap'); assert.deepEqual(label.cells.map(cell => cell.x), labels[0].cells.map(cell => cell.x), 'Legend figures align in columns'); }
  });
  assert(Math.abs(cornerProof.labels[0].y - cornerProof.top - 10) < .1);
  assert.equal(await detailChart.locator('details, .legend, .chart-readout').count(), 0, 'No duplicated summaries or Chart data disclosure');
  const iconProof = await page.locator('.icon-button').evaluateAll(buttons => buttons.every(button => button.getAttribute('aria-label') && button.title && !button.textContent && button.querySelector('svg[aria-hidden=true]')));
  assert(iconProof, 'Icon actions retain accessible names and tooltips');
  await assertSampleDetails(targetSample);
  assert.equal(await detailChart.locator('svg[role=img]').getAttribute('viewBox'), detailGeometry.viewBox, 'Pointer inspection must not resize the plot');
  await page.screenshot({ path: path.join(output, 'expanded-inspection.png') });
  await detailChart.focus(); await page.keyboard.press('ArrowLeft');
  await assertSampleDetails(detailFrame.overview.samples[targetIndex - 1]);
  await page.mouse.move(0, 0);
  await until(async () => (await detailChart.locator('.chart-description').innerText()).startsWith('Latest'), 'pointer departure restores latest details despite retained chart focus');
  assert.equal(await detailChart.locator('.chart-cursor line').count(), 0);
  await writeFile(path.join(output, 'detail-proof.json'), JSON.stringify({ source: targetSample, geometry: detailGeometry, inspectedX: sampleX(targetSample), keyboardSample: detailFrame.overview.samples[targetIndex - 1], cornerProof, iconProof }, null, 2));
  check('Expanded corner legend, age axis and accessible icons retain coherent pointer and keyboard sample inspection');
  const overviewChoice = page.getByRole('group', { name: 'Overall memory view last', exact: true });
  await overviewChoice.getByRole('radio', { name: '5m', exact: true }).check();
  await until(async () => (await view()).settings.overview_range === 300, 'main overview range uses real controller');
  if ((await view()).paused) {
    await page.getByRole('button', { name: 'Resume display', exact: true }).click();
    await until(async () => !(await view()).paused, 'resume in main overview');
  }
  const mainDisplayedAt = (await view()).displayed_at;
  await until(async () => (await view()).displayed_at > mainDisplayedAt, 'main overview keeps receiving collected snapshots');
  assert(await backToProcesses.isVisible(), 'Live updates preserve the main overview');
  await page.getByRole('button', { name: 'Pause display', exact: true }).click();
  await until(async () => (await view()).paused, 'pause in main overview');
  await page.screenshot({ path: path.join(output, 'main-overview-wide.png') });
  await page.setViewportSize({ width: 585, height: 774 });
  await page.getByRole('combobox', { name: /View scale/ }).selectOption('75');
  await until(async () => (await view()).settings.ui_zoom === 75, 'main overview reference scale');
  await page.screenshot({ path: path.join(output, 'main-overview-compact.png') });
  check('Overall memory can fill the main workspace across window and text sizes while range, live updates and pause work', mainLayouts);
  await backToProcesses.focus(); await page.keyboard.press('Escape');
  await expandOverview.waitFor({ state: 'visible' }); assert(await page.locator('.process-panel').isVisible());
  const restoredState = await view();
  for (const key of ['filter', 'grouping', 'sort']) assert.deepEqual(restoredState.settings[key], processState.settings[key], `Restore preserves ${key}`);
  assert.equal(restoredState.table.selected_id, processState.table.selected_id, 'Restore preserves selected process');
  await expandOverview.click(); await page.keyboard.press('Control+f');
  assert(await page.locator('#process-filter').evaluate(input => document.activeElement === input), 'Ctrl+F reveals and focuses the process filter');
  assert(await page.locator('.inspector-scroll').isVisible());
  await expandOverview.click(); await backToProcesses.click(); await expandOverview.waitFor({ state: 'visible' });
  await overviewChoice.getByRole('radio', { name: processState.settings.overview_range === null ? 'All' : `${processState.settings.overview_range / 60}m`, exact: true }).check();
  await until(async () => (await view()).settings.overview_range === processState.settings.overview_range, 'restore overview range');
  check('Main overview returns by Escape, button or filter shortcut without changing process state');
  for (const [name, seconds] of [['15m', 900], ['30m', 1800], ['All', null], ['5m', 300]]) {
    await overviewChoice.getByRole('radio', { name, exact: true }).check();
    await until(async () => (await view()).settings.overview_range === seconds, `Overview toggle ${name}`);
  }
  await overviewChoice.getByRole('radio', { name: '5m', exact: true }).focus();
  await page.keyboard.press('ArrowRight');
  await until(async () => (await view()).settings.overview_range === 900, 'Overview toggle arrow keyboard navigation');
  await page.getByRole('combobox', { name: /View scale/ }).selectOption('50');
  await until(async () => (await view()).settings.ui_zoom === 50, 'Half scale accepted by controller');
  assert.match(await readFile(path.join(output, 'preferences.ini'), 'utf8'), /^ui_zoom=50$/m);
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).fontSize), '7px');
  assert.equal(await page.locator('.app-header h1, .footer-counts, .chart-navigator').count(), 0);
  const copyColor = await page.getByRole('button', { name: 'Copy snapshot to clipboard', exact: true }).evaluate(button => getComputedStyle(button).color);
  await expandOverview.click();
  assert.equal(copyColor, await backToProcesses.evaluate(button => getComputedStyle(button).color));
  await page.getByRole('combobox', { name: /View scale/ }).selectOption('100');
  await until(async () => (await view()).settings.ui_zoom === 100, 'Dialog reference scale');
  await page.setViewportSize({ width: 420, height: 300 });
  const detailsButton = page.getByRole('button', { name: 'Details', exact: true });
  await detailsButton.click();
  const dialog = page.getByRole('dialog', { name: 'Collection details', exact: true });
  await dialog.waitFor({ state: 'visible' });
  const closeDetails = page.getByRole('button', { name: 'Close details', exact: true });
  const dialogLayout = await closeDetails.evaluate(button => {
    const box = button.getBoundingClientRect(), content = document.querySelector('.diagnostic-content');
    return { visible: box.top >= 0 && box.bottom <= innerHeight && box.left >= 0 && box.right <= innerWidth,
      reachable: button.contains(document.elementFromPoint(box.x + box.width/2, box.y + box.height/2)),
      scrollable: content.scrollHeight > content.clientHeight && getComputedStyle(content).overflowY === 'auto' };
  });
  assert(dialogLayout.visible && dialogLayout.reachable && dialogLayout.scrollable, 'Details close stays reachable above scrollable content');
  await page.screenshot({ path: path.join(output, 'details-small-window.png') });
  await closeDetails.click(); await dialog.waitFor({ state: 'hidden' });
  await detailsButton.click(); await dialog.waitFor({ state: 'visible' });
  await page.keyboard.press('Escape'); await dialog.waitFor({ state: 'hidden' });
  assert(await backToProcesses.isVisible(), 'Dialog Escape must preserve expanded overview');
  assert(await detailsButton.evaluate(button => document.activeElement === button), 'Dialog returns focus to its opener');
  await backToProcesses.click();
  await page.setViewportSize({ width: 980, height: 740 });
  await overviewChoice.getByRole('radio', { name: processState.settings.overview_range === null ? 'All' : `${processState.settings.overview_range/60}m`, exact: true }).check();
  await until(async () => (await view()).settings.overview_range === processState.settings.overview_range, 'Restore range after toggle proof');
  check('Overview toggles, half scale, reduced chrome and reachable dialog close work through the real headless browser', dialogLayout);
  const chartBefore = await view();
  const toggle = (group, name) => page.getByRole('group', { name: group, exact: true }).getByRole('radio', { name, exact: true });
  for (const group of ['Overall memory metric', 'Overall memory Y-axis', 'Selected history metric', 'Selected history Y-axis']) {
    const proof = await page.getByRole('group', { name: group, exact: true }).evaluate(fieldset => [...fieldset.querySelectorAll('label')].map(label => ({ title: label.title, name: label.querySelector('input').getAttribute('aria-label'), decorative: label.querySelector('svg').getAttribute('aria-hidden') })));
    assert.equal(proof.length, 2); assert(proof.every(option => option.title && option.name && option.decorative === 'true'));
  }
  await toggle('Overall memory metric', 'Private commit').check();
  await until(async () => (await view()).settings.overview_metric === 'private_commit', 'Overall commit metric');
  let commitView = await view();
  assert.equal(commitView.settings.metric, chartBefore.settings.metric, 'History metric stays independent');
  assert.deepEqual(commitView.system_memory, chartBefore.system_memory, 'Physical header stays on the same paused sample');
  assert.equal(commitView.overview.capacity, '0');
  assert(!commitView.overview.layers.some(layer => layer.name === 'Other used RAM'));
  assert(commitView.overview.samples.every(sample => sample.available === null));
  assert.match(await page.locator('.overview-panel h2').innerText(), /Measured private commit/);
  assert.doesNotMatch(await page.locator('.overview-panel .chart-data').textContent(), /of physical RAM|Available|Capacity/);
  await toggle('Overall memory Y-axis', 'Fit values').check();
  await until(async () => (await view()).settings.overview_from_zero === false, 'Overall fitted scale');
  assert.equal((await view()).settings.from_zero, chartBefore.settings.from_zero);
  await toggle('Selected history metric', 'Private commit').check();
  await until(async () => (await view()).settings.metric === 'private_commit', 'History commit metric');
  await toggle('Selected history Y-axis', 'Fit values').check();
  await until(async () => (await view()).settings.from_zero === false, 'History fitted scale');
  await toggle('Overall memory Y-axis', 'From zero').check();
  await until(async () => (await view()).settings.overview_from_zero === true, 'Overall zero scale');
  assert.equal((await view()).settings.from_zero, false, 'History axis stays independent');
  const metricPrefs = await readFile(path.join(output, 'preferences.ini'), 'utf8');
  assert.match(metricPrefs, /^metric=private_commit$/m); assert.match(metricPrefs, /^overview_metric=private_commit$/m); assert.match(metricPrefs, /^overview_from_zero=true$/m);
  await page.screenshot({ path: path.join(output, 'metric-axis-icons.png') });
  await toggle('Overall memory metric', 'Private working set').focus();
  await page.keyboard.press('ArrowRight');
  await until(async () => (await view()).settings.overview_metric === 'private_commit', 'Metric native keyboard navigation');
  await toggle('Overall memory metric', 'Private working set').check();
  await toggle('Selected history metric', chartBefore.settings.metric === 'private_commit' ? 'Private commit' : 'Private working set').check();
  await toggle('Selected history Y-axis', chartBefore.settings.from_zero ? 'From zero' : 'Fit values').check();
  await toggle('Overall memory Y-axis', chartBefore.settings.overview_from_zero ? 'From zero' : 'Fit values').check();
  await until(async () => {
    const restored = await view();
    return restored.settings.overview_metric === 'private_working_set' && restored.settings.metric === chartBefore.settings.metric
      && restored.settings.from_zero === chartBefore.settings.from_zero && restored.settings.overview_from_zero === chartBefore.settings.overview_from_zero
      && await toggle('Selected history metric', chartBefore.settings.metric === 'private_commit' ? 'Private commit' : 'Private working set').isChecked();
  }, 'Restore chart settings before next journey');
  check('Both charts support independent accessible metric/axis icons, commit semantics and saved preferences');
  await toggle('Selected history metric', 'Private commit').check();
  await until(async () => (await view()).settings.metric === 'private_commit', 'Inspector commit context');
  const inspectionView = await view(), latestInspection = inspectionView.history.latest_inspection;
  const summary = page.locator('.inspector-scroll .summary-values dd');
  const assertInspection = async (sample, metric) => {
    for (const [index, counter] of [[0, sample.private], [1, sample.commit], [3, sample.peak]]) {
      const label = await summary.nth(index).getAttribute('aria-label');
      assert(label.includes(counter.value === null ? 'Unavailable' : `${counter.value} bytes`), `Inspector value ${index} follows inspected sample ${sample.at}`);
    }
    assert((await summary.nth(3).getAttribute('aria-label')).includes(`Peak ${metric}`));
    const delta = await summary.nth(2).getAttribute('aria-label');
    assert(delta.includes(`${sample.change.value ?? 'Unavailable'} raw bytes`));
  };
  await page.mouse.move(10, 10);
  await until(async () => (await summary.nth(3).getAttribute('aria-label')).includes('Peak private commit'), 'Inspector adopts chosen metric');
  await assertInspection(latestInspection, 'private commit');
  const inspectionChart = page.locator('.inspector-scroll .memory-chart');
  await inspectionChart.focus(); await page.keyboard.press('ArrowLeft');
  const inspectedPoint = inspectionView.history.points.at(-2);
  assert(inspectedPoint, 'Real retained history offers a previous sample');
  await assertInspection(inspectedPoint.inspection, 'private commit');
  assert.match(await page.locator('.inspector-scroll .coverage').innerText(), /ago/);
  await page.screenshot({ path: path.join(output, 'inspector-context.png') });
  await page.keyboard.press('Escape'); await assertInspection(latestInspection, 'private commit');
  assert.match(await page.locator('.inspector-scroll .coverage').innerText(), /Latest/);
  await toggle('Selected history metric', chartBefore.settings.metric === 'private_commit' ? 'Private commit' : 'Private working set').check();
  await until(async () => (await view()).settings.metric === chartBefore.settings.metric, 'Restore metric after inspector proof');
  check('Inspector summary follows selected metric and historical chart sample, then restores latest');

  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.getByRole('combobox', { name: /View scale/ }).selectOption('100');
  await until(async () => (await view()).settings.ui_zoom === 100, 'Band navigation reference scale');
  const beforeBand = await view();
  await page.getByRole('group', { name: 'Group by', exact: true }).getByRole('radio', { name: 'Path', exact: true }).check();
  await until(async () => (await view()).settings.grouping === 'path', 'Band navigation begins in another grouping');
  await page.getByRole('searchbox', { name: 'Filter:', exact: true }).fill('mm021-nothing-matches');
  await until(async () => (await view()).settings.filter === 'mm021-nothing-matches', 'Band navigation begins with a hidden target');
  await page.mouse.move(0, 0); await page.locator('.system-summary').click();
  const navigationChart = page.locator('.overview-panel .memory-chart');
  const bandTarget = await navigationChart.locator('.chart-areas path[data-process-id]').evaluateAll(paths => {
    for (const path of paths) {
      const box = path.getBBox(), matrix = path.getScreenCTM();
      for (const fraction of [.8, .6, .4, .95]) for (let row = 1; row < 60; row++) {
        const point = new DOMPoint(box.x + box.width * fraction, box.y + box.height * row / 60);
        if (!path.isPointInFill(point)) continue;
        const screen = point.matrixTransform(matrix);
        if (document.elementFromPoint(screen.x, screen.y) === path) return { x: screen.x, y: screen.y, id: path.getAttribute('data-process-id') };
      }
    }
    return null;
  });
  assert(bandTarget, 'A real rendered program band offers a double-click target');
  await page.mouse.dblclick(bandTarget.x, bandTarget.y);
  await until(async () => { const value = await view(); return value.selected?.id === bandTarget.id && value.settings.grouping === 'name' && !value.settings.filter; }, 'Band selects its program across grouping/filter');
  const openedBand = await view();
  assert.equal(openedBand.paused, beforeBand.paused);
  assert.equal(openedBand.settings.metric, beforeBand.settings.metric); assert.equal(openedBand.settings.overview_range, beforeBand.settings.overview_range);
  await grid.locator(`[data-row-id="${bandTarget.id}"][aria-selected=true]`).waitFor({ state: 'visible' });
  assert.equal(await page.locator('#selected-title').innerText(), openedBand.selected.name);
  await page.getByRole('button', { name: 'Expand overview', exact: true }).click();
  const legendTarget = navigationChart.locator('.band-label[data-process-id]').first();
  const legendId = await legendTarget.getAttribute('data-process-id');
  await legendTarget.dblclick();
  await until(async () => !(await page.locator('.dashboard').evaluate(root => root.classList.contains('overview-expanded'))) && (await view()).selected?.id === legendId, 'Expanded legend returns to process inspector');
  await navigationChart.focus();
  const keyboardIds = await navigationChart.locator('.chart-areas path[data-process-id]').evaluateAll(paths => paths.map(path => path.getAttribute('data-process-id')));
  const keyboardId = keyboardIds[Math.min(1, keyboardIds.length - 1)];
  await navigationChart.press('ArrowUp');
  const revisionBeforeEnter = (await view()).revision;
  await navigationChart.press('Enter');
  await until(async () => { const value = await view(); return value.revision > revisionBeforeEnter && value.selected?.id === keyboardId; }, 'Keyboard opens selected named band');
  await grid.locator(`[data-row-id="${keyboardId}"][aria-selected=true]`).waitFor({ state: 'visible' });
  await page.screenshot({ path: path.join(output, 'band-navigation.png') });
  check('Real band double-click, expanded legend and keyboard navigate to a visible program without changing chart settings');


  await page.getByRole('combobox', { name: /View scale/ }).selectOption('100');
  await until(async () => (await view()).settings.ui_zoom === 100, 'restore test scale');
  const preferences = await readFile(path.join(output, 'preferences.ini'), 'utf8');
  assert.match(preferences, /^ui_zoom=100$/m);
  assert.match(preferences, /^grouping=name$/m);
  assert(!preferences.includes('headless-test-host'), 'Preferences must contain presentation settings only');
  check('Presentation preferences stay in the isolated test directory');
  assert.deepEqual(errors, []); check('No renderer exceptions');
  await page.screenshot({ path: path.join(output, 'final.png') });

} catch (error) {
  failure = error;
  if (page && !page.isClosed()) await page.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
} finally {
  const cleanup = {};
  try { if (browser) await browser.close(); cleanup.browser_closed = true; }
  catch (error) { failure ||= error; cleanup.browser_closed = false; }
  try { if (server) await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())); cleanup.server_closed = true; }
  catch (error) { failure ||= error; cleanup.server_closed = false; }
  if (child) {
    child.stdin.end();
    let result = await Promise.race([hostExit, sleep(3000).then(() => null)]);
    cleanup.forced_cleanup = result === null;
    if (!result) { child.kill(); result = await Promise.race([hostExit, sleep(3000).then(() => null)]); }
    cleanup.host_exit = result;
    if (!result || result.code !== 0 || cleanup.forced_cleanup) failure ||= Error(`Rust host did not exit normally: ${JSON.stringify(cleanup)}`);
  }
  const afterHash = await hash(executable);
  if (afterHash !== beforeHash) failure ||= Error('Headless tests changed the backend executable');
  await writeFile(path.join(output, 'report.json'), JSON.stringify({ result: failure ? 'FAIL' : 'PASS', executable, beforeHash, afterHash, started,
    duration_seconds: (Date.now() - started.getTime()) / 1000, command: process.argv, cwd: process.cwd(), pid: child?.pid,
    checks, cleanup, browserUrl, frontendAssets, error: failure && String(failure.stack || failure), stderr,
    boundary: 'Headless browser and real Rust controller/sampler; private path/snapshot clipboard adapters. No native shell, OS clipboard, save dialog or Windows UIA claim.' }, null, 2));
  console.log(`Evidence: ${output}`);
}
if (failure) throw failure;
