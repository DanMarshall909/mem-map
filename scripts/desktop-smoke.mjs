// Slow lane: the compiled Tauri application and its actual WebView2 compositor.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const require = createRequire(path.join(root, 'frontend', 'package.json'));
const { chromium } = require('playwright-core');
const execute = promisify(execFile);
const executable = path.resolve(process.argv[2] || path.join(root, 'MemMap.exe'));
const output = path.resolve(process.argv[3] || path.join(root, 'artifacts', 'tauri', 'desktop-smoke'));
await mkdir(output, { recursive: true });
const hash = async file => createHash('sha256').update(await readFile(file)).digest('hex');
const beforeHash = await hash(executable);
const started = new Date();
const listener = net.createServer();
await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve));
const port = listener.address().port;
await new Promise(resolve => listener.close(resolve));
const profile = path.join(output, 'webview-profile');
const child = spawn(executable, [], { cwd: path.dirname(executable), windowsHide: true, env: {
  ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --force-renderer-accessibility`,
  MEMMAP_WEBVIEW_DATA_DIR: profile, MEMMAP_PREFERENCES_PATH: path.join(output, 'preferences.ini'),
} });
const native = async (mode, extra = []) => {
  const result = await execute('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'scripts', 'desktop-native-probe.ps1'), '-Mode', mode, '-AppProcessId', String(child.pid), ...extra], { windowsHide: true, timeout: 25000 });
  return JSON.parse(result.stdout.replace(/^\uFEFF/, '').trim());
};
const checks = []; let browser; let page;
const check = (name, details = true) => { checks.push({ name, details }); console.log(`PASS ${name}`); };
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const until = async (predicate, label, timeout = 20000) => {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (await predicate()) return; if (child.exitCode !== null) throw new Error(`Application exited ${child.exitCode}: ${label}`); await sleep(100); }
  throw new Error(`Timed out: ${label}`);
};
try {
  await until(async () => { try { return (await fetch(`http://127.0.0.1:${port}/json/version`)).ok; } catch { return false; } }, 'WebView2 debug endpoint');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  await until(() => browser.contexts().some(c => c.pages().length), 'Tauri page');
  page = browser.contexts().flatMap(c => c.pages())[0];
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  const view = () => page.evaluate(() => window.__TAURI_INTERNALS__.invoke('get_view', {}));
  await until(async () => (await view()).table.rows.length > 0, 'real Rust first frame');
  await page.getByRole('treegrid', { name: 'Processes' }).waitFor();
  const first = await view();
  assert(first.overview.capacity !== '0'); assert(first.selected); assert.equal(typeof first.selected.private.value, 'string');
  check('Real collector and precision-safe IPC', { rows: first.table.rows.length, status: first.collection.status });
  const grid = page.locator('.process-grid'); const inspector = page.locator('.inspector-scroll');
  await inspector.waitFor();
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
  // Actual full-path native clipboard outcome, independent readback.
  const clipboardBefore = await native('ClipboardState');
  const copyButton = page.getByRole('button', { name: /^Copy (all )?paths?$/i }).first();
  await copyButton.click();
  await page.waitForFunction(() => document.querySelector('.notice')?.textContent.startsWith('Copied '));
  assert((await native('Clipboard', ['-AfterSequence', String(clipboardBefore.Sequence)])) === (await view()).selected.paths.join('\r\n'), 'Native clipboard must match full selected paths');
  check('Actual Copy button preserves every full original path');
  const csvPath = path.join(output, 'paused.csv');
  const save = native('SaveCsv', ['-Destination', csvPath]);
  await page.getByRole('button', { name: 'Export CSV', exact: true }).click();
  const saveProof = await save;
  await writeFile(path.join(output, 'native-save.json'), JSON.stringify(saveProof, null, 2));
  assert.equal(saveProof.ChosenPath, csvPath);
  assert.equal(saveProof.NativeEditText, csvPath);
  assert.equal(saveProof.SaveInvoked, true);
  await until(async () => { try { return (await readFile(csvPath)).length > 0; } catch { return false; } }, 'CSV native save');
  const csv = await readFile(csvPath, 'utf8');
  const records = await native('Csv', ['-Destination', csvPath]);
  assert(records.length > 0);
  assert(records.every(row => row.elapsed_seconds === String(paused.displayed_at) && row.snapshot_unix_ms === paused.displayed_unix_ms));
  assert.equal(new Set(records.map(row => `${row.pid}/${row.creation_filetime}/${row.provisional_generation}`)).size, records.length);
  assert(csv.includes('private_working_set_bytes'));
  check('Native CSV save preserves frozen timestamp and distinct process identities', { bytes: csv.length, records: records.length });
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
    await page.getByRole('radio', { name: grouping, exact: true }).check();
    await until(async () => { const frame = await view(); return frame.settings.grouping === value && frame.table.rows.length > 0; }, `group ${grouping}`);
  }
  check('All five groupings through the real command boundary');
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
      return { protectedTail: true, value: preview.previousElementSibling.value, tail:tail.textContent, original,
        visible: getComputedStyle(preview).visibility !== 'hidden' && rects.length > 0 && rects.every(r => r.left >= box.left - 1 && r.right <= box.right + 1 && r.top >= box.top - 1 && r.bottom <= box.bottom + 1),
        beginningVisible: leading.getBoundingClientRect().width <= prefix.clientWidth + 1 };
    }, selectedPath[0]);
    assert(pathProof.protectedTail, `${label}: executable filename must have a protected visible region`);
    assert.equal(pathProof.value, selectedPath[0]);
    assert.equal(pathProof.tail, selectedPath[0].slice(Math.max(selectedPath[0].lastIndexOf('\\'), selectedPath[0].lastIndexOf('/'))));
    assert(pathProof.visible && pathProof.beginningVisible, `${label}: original beginning and complete executable filename must remain painted inside the path field`);
    const pathInput = page.getByRole('textbox', { name: 'Full executable path', exact: true });
    const pathHeight = await page.locator('.path-display').evaluate(el => el.clientHeight);
    await pathInput.focus();
    assert.equal(await pathInput.inputValue(), selectedPath[0]);
    assert.equal(await page.locator('.path-preview').evaluate(el => getComputedStyle(el).visibility), 'hidden');
    await pathInput.evaluate(el => el.blur());
    assert.equal(await page.locator('.path-display').evaluate(el => el.clientHeight), pathHeight, 'Focus must not collapse a wrapped filename');
    pathLayoutProofs.push({ label, ...pathProof });
    await writeFile(path.join(output, 'path-layout.json'), JSON.stringify(pathLayoutProofs, null, 2));
  };
  for (const [width, height] of [[1280, 800], [980, 740], [760, 540], [1440, 900]]) {
    await native('Resize', ['-Width', String(Math.round(width * dpr)), '-Height', String(Math.round(height * dpr))]);
    await sleep(250);
    await usableLayout(`native ${width}x${height}`);
    await page.screenshot({ path: path.join(output, `window-${width}x${height}.png`) });
  }
  check('Actual native window resizing retains root containment', { dpr });
  await page.keyboard.press('Control++');
  await until(async () => (await view()).settings.ui_zoom === 125, 'Global zoom after path blur');
  const beforeColumns = (await view()).settings.column_widths;
  const resizeColumn = await page.getByRole('separator', { name: 'Resize PID column', exact: true }).boundingBox(); assert(resizeColumn);
  await page.mouse.move(resizeColumn.x + resizeColumn.width / 2, resizeColumn.y + resizeColumn.height / 2);
  await page.mouse.down(); await page.mouse.move(resizeColumn.x + resizeColumn.width / 2 + 7, resizeColumn.y + resizeColumn.height / 2); await page.mouse.up();
  await until(async () => (await view()).settings.column_widths[1] !== beforeColumns[1], '125% scaled native column resize');
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
  check('Windows UI Automation', await native('Inspect'));
  assert.deepEqual(errors, []); check('No renderer exceptions');
  await page.screenshot({ path: path.join(output, 'final.png') });
  await writeFile(path.join(output, 'report.json'), JSON.stringify({ result: 'PASS', executable, sha256: beforeHash, started, command: process.argv, cwd: process.cwd(), checks, first, final: await view() }, null, 2));
} catch (error) {
  if (page) await page.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
  await writeFile(path.join(output, 'report.json'), JSON.stringify({ result: 'FAIL', executable, sha256: beforeHash, checks, error: String(error.stack || error) }, null, 2));
  throw error;
} finally {
  if (child.exitCode === null) await native('Close').catch(() => {});
  for (let i = 0; child.exitCode === null && i < 30; i++) await sleep(100);
  const forced = child.exitCode === null && child.signalCode === null;
  if (forced) { child.kill(); for (let i = 0; child.exitCode === null && child.signalCode === null && i < 20; i++) await sleep(100); }
  if (browser) await Promise.race([browser.close().catch(() => {}), sleep(1000)]);
  const afterHash = await hash(executable);
  await writeFile(path.join(output, 'command.json'), JSON.stringify({ command: process.argv, cwd: process.cwd(), started, duration_seconds: (Date.now() - started.getTime()) / 1000, beforeHash, afterHash, pid: child.pid, process_exit: child.exitCode, process_signal: child.signalCode, forced_cleanup: forced, process_closed: child.exitCode !== null || child.signalCode !== null }, null, 2));
  assert.equal(afterHash, beforeHash, 'Desktop checks must not mutate the binary');
  assert(child.exitCode !== null || child.signalCode !== null, 'Owned application must exit before handback');
  console.log(`Evidence: ${output}`);
}
