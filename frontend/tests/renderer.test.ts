import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { fixture } from './fixture.ts';
import { ProcessTreeGrid } from '../src/treegrid.ts';
import { Dashboard } from '../src/dashboard.ts';
import { MemoryChart } from '../src/charts.ts';
import { bytes, change } from '../src/format.ts';
function dom() { const window = new Window(); Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element }); return window; }
test('column dragging at enlarged scale produces valid whole-pixel desktop preferences', () => {
  const window = dom(); const calls: { type: string; value?: number[] }[] = [];
  const grid = new ProcessTreeGrid(action => calls.push(action as { type: string; value?: number[] }));
  document.body.append(grid.element); const view = fixture(); view.settings.ui_zoom = 125; grid.update(view);
  const resize = grid.element.querySelector('[aria-label="Resize PID column"]')!;
  resize.dispatchEvent(new window.PointerEvent('pointerdown', { clientX: 100, bubbles: true }));
  document.dispatchEvent(new window.PointerEvent('pointermove', { clientX: 101, bubbles: true }));
  document.dispatchEvent(new window.PointerEvent('pointerup', { clientX: 101, bubbles: true }));
  const widths = calls.find(action => action.type === 'set_column_widths')?.value;
  assert.ok(widths, 'Pointer release publishes remembered widths');
  assert.ok(widths.every(Number.isInteger), 'Rust desktop preferences require exact whole logical pixels');
  assert.ok(widths[1] > view.settings.column_widths[1]);
  window.happyDOM.abort();
});
test('physical chart labels its measured endpoint, capacity and available bracket inside the plot panel', () => {
  const window = dom(); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element);
  const view = fixture(); dashboard.update(view);
  const plot = dashboard.element.querySelector('.overview-panel svg')!;
  const labels = [...plot.querySelectorAll('text')].map(label => label.textContent);
  const latest = view.overview.samples.at(-1)!;
  assert.ok(labels.includes(`Capacity: ${bytes(view.overview.capacity)}`));
  assert.ok(labels.includes(bytes(latest.used)));
  assert.ok(labels.includes(`Available ${bytes(latest.available)}`));
  window.happyDOM.abort();
});
test('unavailable physical capacity remains unknown instead of a zero capacity or zero-percent meter', () => {
  const window = dom(); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element);
  const view = fixture(); view.overview.capacity = '0'; view.overview.samples = [];
  view.collection = { status: 'unavailable', age_seconds: 0, error: 'System reading failed' }; dashboard.update(view);
  assert.match(dashboard.element.querySelector('.system-summary')!.textContent!, /used — \/ —/);
  assert.equal(dashboard.element.querySelector('progress')!.hasAttribute('value'), false);
  assert.match(dashboard.element.querySelector('.overview-panel svg')!.textContent!, /Capacity: —/);
  assert.doesNotMatch(dashboard.element.querySelector('.overview-panel .chart-readout')!.textContent!, /0 B/);
  window.happyDOM.abort();
});
test('each explicit Show processes response can reveal the same child while polling never scrolls it', () => {
  const window = dom(); const grid = new ProcessTreeGrid(() => {}); document.body.append(grid.element);
  const view = fixture(); const leaf = view.table.rows[1];
  view.table.rows = Array.from({ length: 40 }, (_, i) => ({ ...leaf, id: `leaf-${i}`, parent_id: null }));
  view.reveal_row_id = 'leaf-39'; grid.update(view, true);
  assert.ok(grid.element.scrollTop > 0);
  grid.element.scrollTop = 0; view.revision++; grid.update(view, true);
  assert.ok(grid.element.scrollTop > 0, 'A repeated explicit request reveals the same child again');
  grid.element.scrollTop = 0; view.revision++; grid.update(view);
  assert.equal(grid.element.scrollTop, 0, 'A refreshed projection cannot repeat a scroll effect');
  window.happyDOM.abort();
});
test('pointer sample inspection and history dragging use the rendered plot at every interface scale', () => {
  const window = dom();
  for (const scale of [100, 125, 200]) for (const kind of ['overview', 'history'] as const) {
    const actions: unknown[] = []; const chart = new MemoryChart(kind, action => actions.push(action)); document.body.append(chart.element);
    const view = fixture(); view.settings.ui_zoom = scale;
    view.overview.samples = Array.from({ length: 61 }, (_, i) => ({ ...view.overview.samples[0], at: i * 10 }));
    view.history!.points = Array.from({ length: 61 }, (_, i) => ({ ...view.history!.points[0], at: i * 10 }));
    chart.update(view); const plot = chart.element.querySelector('svg')!;
    Object.assign(plot, { getBoundingClientRect: () => ({ left: 100, width: 900 }), setPointerCapture: () => {}, releasePointerCapture: () => {} });
    const width = Number(plot.getAttribute('viewBox')!.split(' ')[2]); const line = plot.querySelector('line')!;
    const left = Number(line.getAttribute('x1')), right = Number(line.getAttribute('x2'));
    const pixel = (x: number) => 100 + x / width * 900;
    for (const [position, expected] of [[left, '0:00'], [(left + right) / 2, '5:00'], [right, '10:00']] as const) {
      plot.dispatchEvent(new window.PointerEvent('pointermove', { clientX: pixel(position), bubbles: true }));
      assert.ok(chart.element.querySelector('.chart-data')!.textContent!.includes(`· ${expected} elapsed`), `${kind} ${scale}% at ${expected}`);
    }
    if (kind === 'history') {
      plot.dispatchEvent(new window.PointerEvent('pointerdown', { clientX: pixel(left), button: 0, pointerId: 1, bubbles: true }));
      plot.dispatchEvent(new window.PointerEvent('pointermove', { clientX: pixel((left + right) / 2), pointerId: 1, bubbles: true }));
      assert.deepEqual(actions.at(-1), { type: 'set_history_viewport', end: 300, width: 600 });
      plot.dispatchEvent(new window.PointerEvent('pointerup', { pointerId: 1, bubbles: true }));
    }
    chart.element.remove();
  }
  window.happyDOM.abort();
});
test('live values update keyed rows without replacing focus, and disclose hierarchy and raw unavailable values', () => {
  const window = dom(); const grid = new ProcessTreeGrid(() => {}); document.body.append(grid.element);
  const view = fixture(); grid.update(view);
  const row = grid.element.querySelector<HTMLElement>('[data-row-id="name:worker"]');
  assert.ok(row, 'selected group is rendered'); row.focus(); grid.element.scrollTop = 112;
  view.revision++; view.table.rows[0].private.value = '2147483648'; grid.update(view);
  assert.equal(grid.element.querySelector('[data-row-id="name:worker"]'), row);
  assert.equal(document.activeElement, row); assert.equal(grid.element.scrollTop, 112);
  assert.equal(row.getAttribute('aria-expanded'), 'true'); assert.equal(row.getAttribute('aria-selected'), 'true');
  assert.match(row.textContent!, /2\.00 GiB/);
  const child = grid.element.querySelector('[data-row-id="process:123:18000000000000000"]')!;
  assert.equal(child.getAttribute('aria-level'), '2');
  view.table.rows[1].private = { value: null, reason: 'Access denied', measured: 0, total: 1, enumeration_complete: true }; grid.update(view);
  assert.match(child.textContent!, /—/); assert.match(child.getAttribute('aria-label')!, /Access denied/);
  window.happyDOM.abort();
});
test('treegrid keys select stable IDs and collapse or enter children', () => {
  const window = dom(); const calls: unknown[] = []; const grid = new ProcessTreeGrid(a => calls.push(a)); document.body.append(grid.element);
  grid.update(fixture()); const row = grid.element.querySelector<HTMLElement>('[data-row-id="name:worker"]'); assert.ok(row); row.focus();
  row.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  row.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.deepEqual(calls, [{ type: 'set_table_interaction_held', value: true }, { type: 'select_row', id: 'process:123:18000000000000000' }, { type: 'toggle_expanded', id: 'name:worker' }]);
  window.happyDOM.abort();
});

test('repeated live frames retain inspector scroll, focused control and selected history choices', () => {
  const window = dom(); const actions: unknown[] = []; const dashboard = new Dashboard(a => actions.push(a), () => {}); document.body.append(dashboard.element);
  const view = fixture(); dashboard.update(view); dashboard.inspector.scrollTop = 233;
  const metric = [...dashboard.element.querySelectorAll('select')].find(input => input.parentElement?.textContent?.startsWith('Metric:'))!;
  metric.focus(); const inspector = dashboard.inspector; const ranges = dashboard.inspector.querySelector('fieldset')!;
  for (let i = 1; i <= 20; i++) { const frame = structuredClone(view); frame.revision = i + 1; frame.live_at += i * 2; frame.history_viewport.follow = false; frame.history_viewport.manual_end = 600; frame.history_viewport.reset_enabled = true; dashboard.update(frame); }
  assert.equal(dashboard.inspector, inspector); assert.equal(inspector.scrollTop, 233); assert.equal(document.activeElement, metric);
  assert.equal(inspector.querySelector('fieldset'), ranges); assert.equal(dashboard.element.querySelectorAll('.inspector-scroll').length, 1);
  const follow = [...inspector.querySelectorAll('input')].find(input => input.parentElement?.textContent === 'Follow latest')!; assert.equal(follow.checked, false);
  assert.match(inspector.textContent!, /Requested: 30m · Recorded: 10m 00s/);
  const chartData = inspector.querySelector('.chart-data')!; chartData.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true })); assert.match(chartData.textContent!, /5:00 elapsed/);
  window.happyDOM.abort();
});

test('path aggregates retain original identity, clipboard action and accessible full paths', () => {
  const window = dom(); const native: string[] = []; const dashboard = new Dashboard(() => {}, action => native.push(action)); document.body.append(dashboard.element);
  const view = fixture(); view.settings.grouping = 'path';
  const long = 'C:\\Program Files\\' + 'original long directory\\'.repeat(8) + 'worker.exe';
  view.table.rows[0].detail = long; view.selected!.paths = [long, 'D:\\Other\\worker.exe']; view.selected!.path_count = 2; dashboard.update(view);
  assert.match(dashboard.table.element.querySelector('[data-row-id="name:worker"]')!.textContent!, /original long directory/);
  const copy = [...dashboard.inspector.querySelectorAll('button')].find(b => b.textContent === 'Copy all paths')!; copy.click(); assert.deepEqual(native, ['copy']);
  assert.equal(dashboard.inspector.querySelector('input[readonly]')!.getAttribute('aria-description'), `${long}; D:\\Other\\worker.exe`);
  assert.ok(dashboard.inspector.textContent!.includes(long));
  window.happyDOM.abort();
});

test('a partial current history endpoint stays unknown and all overview totals remain textual', () => {
  const window = dom(); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element);
  const view = fixture(); view.history!.points.at(-1)!.reading = { value: '1073741824', measured: 1, total: 2, reason: 'Access denied', enumeration_complete: true }; view.history!.current = view.history!.points.at(-1)!.reading;
  view.selected!.private = view.history!.current; dashboard.update(view);
  assert.match(dashboard.inspector.textContent!, /Partial/); assert.match(dashboard.inspector.querySelector('.chart-data')!.textContent!, /Access denied/);
  assert.equal(dashboard.inspector.querySelectorAll('svg circle').length, 0, 'partial endpoint has no falsely complete chart marker');
  assert.match(dashboard.element.querySelector('.overview-panel')!.textContent!, /Used 24\.00 GiB/); assert.match(dashboard.element.querySelector('.overview-panel')!.textContent!, /Available 8\.00 GiB/);
  window.happyDOM.abort();
});

test('byte formatting keeps integer precision and removes signed rounded zero', () => {
  assert.equal(bytes('9007199254740993'), '8388608.00 GiB');
  assert.equal(change({ value: '-1', reason: null }), '0.0 MiB');
  assert.equal(change({ value: '1048576', reason: null }), '+1.0 MiB');
});

test('Ctrl+F and Escape address the filter, and F8 requests actual process parent navigation', () => {
  const window = dom(); const actions: unknown[] = []; const dashboard = new Dashboard(a => actions.push(a), () => {}); document.body.append(dashboard.element); dashboard.update(fixture());
  document.body.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'f', ctrlKey: true, bubbles: true }));
  const filter = dashboard.element.querySelector<HTMLInputElement>('input[type=search]')!; assert.ok(document.activeElement === filter, 'Global Ctrl+F focuses the filter from the document body');
  filter.value = 'worker'; filter.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert.equal(filter.value, ''); assert.deepEqual(actions, [{ type: 'set_filter', value: '' }]);
  filter.blur(); document.body.dispatchEvent(new window.KeyboardEvent('keydown', { key: '+', ctrlKey: true, bubbles: true }));
  assert.deepEqual(actions.at(-1), { type: 'set_ui_zoom', value: 125 }, 'Global zoom works after focus leaves a dashboard control');
  document.body.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'F8', bubbles: true })); assert.deepEqual(actions.at(-1), { type: 'select_process_parent' });
  window.happyDOM.abort();
});

test('semantic controls dispatch the agreed domain actions and preserve in-progress filter text', t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const window = dom(); const actions: unknown[] = []; const dashboard = new Dashboard(a => actions.push(a), () => {}); document.body.append(dashboard.element); const view = fixture(); dashboard.update(view);
  const group = dashboard.element.querySelector('fieldset')!;
  assert.equal(group.querySelectorAll('input:checked').length, 1); assert.equal(group.querySelector<HTMLInputElement>('input:checked')!.value, 'name');
  group.querySelector<HTMLInputElement>('input[value=family]')!.click();
  dashboard.element.querySelector<HTMLSelectElement>('select[aria-label="Metric"]')!.value = 'private_commit';
  dashboard.element.querySelector('select[aria-label="Metric"]')!.dispatchEvent(new window.Event('change'));
  dashboard.inspector.querySelector<HTMLInputElement>('input[value="900"]')!.click();
  dashboard.inspector.querySelector<HTMLInputElement>('input[value=fit]')!.click();
  [...dashboard.element.querySelectorAll('button')].find(b => b.textContent === 'Pause display')!.click();
  assert.deepEqual(actions, [{ type: 'set_grouping', value: 'family' }, { type: 'set_metric', value: 'private_commit' }, { type: 'set_history_range', seconds: 900 }, { type: 'set_from_zero', value: false }, { type: 'set_paused', value: true }]);
  const filter = dashboard.element.querySelector<HTMLInputElement>('input[type=search]')!; filter.focus(); filter.value = 'hidden original directory'; filter.dispatchEvent(new window.Event('input'));
  dashboard.update(structuredClone(view)); assert.equal(filter.value, 'hidden original directory'); assert.equal(actions.length, 5);
  t.mock.timers.tick(200); assert.deepEqual(actions.at(-1), { type: 'set_filter', value: 'hidden original directory' });
  window.happyDOM.abort();
});

test('overview keeps sample navigation accessible through an explicit disclosure instead of consuming legend space', () => {
  const window = dom(); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element); dashboard.update(fixture());
  const panel = dashboard.element.querySelector('.overview-panel')!;
  const inspection = panel.querySelector<HTMLDetailsElement>('.chart-inspection'); assert.ok(inspection, 'chart data has a keyboard-accessible disclosure');
  assert.equal(inspection.open, false); assert.equal(inspection.querySelector('summary')!.textContent, 'Chart data');
  assert.ok(inspection.querySelector('.chart-data')); assert.equal(panel.querySelectorAll('.legend-entry').length, 2);
  inspection.open = true; assert.match(inspection.textContent!, /Used 24\.00 GiB/); window.happyDOM.abort();
});
