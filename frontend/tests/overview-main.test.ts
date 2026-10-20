import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { fixture } from './fixture.ts';

function setup() {
  const window = new Window({ width: 1440, height: 900 });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const style = document.createElement('style'); style.textContent = readFileSync(new URL('../src/styles.css', import.meta.url), 'utf8'); document.head.append(style);
  const calls: unknown[] = [], dashboard = new Dashboard(action => calls.push(action), () => {});
  document.body.append(dashboard.element); const view = fixture(); dashboard.update(view);
  const control = (label: string) => [...dashboard.element.querySelectorAll('button')].find(button => (button.getAttribute('aria-label') ?? button.textContent) === label);
  return { window, calls, dashboard, view, control };
}

test('overall memory can become the main workspace and return with process state and latest data', () => {
  const { window, calls, dashboard, view, control } = setup();
  view.settings.filter = 'worker'; view.settings.grouping = 'family'; view.paused = true; dashboard.update(view);
  const chart = dashboard.element.querySelector('.overview-panel .memory-chart')!;
  const row = dashboard.table.element.querySelector('[data-row-id="name:worker"]')!;
  const expand = control('Expand overview'); assert(expand, 'Overall memory must offer a main-view action');
  expand.focus(); expand.click();
  assert.equal(dashboard.element.querySelector<HTMLElement>('.process-panel')!.hidden, true);
  assert.equal(dashboard.inspector.hidden, true);
  assert.equal(dashboard.element.querySelector<HTMLElement>('.splitter')!.hidden, true);
  assert.equal(control('Back to processes') === expand, true, 'The same focused control provides the way back');
  assert.equal(document.activeElement === expand, true);
  assert.equal(window.getComputedStyle(dashboard.element.querySelector('.workspace')!).gridTemplateColumns, 'minmax(0, 1fr)');
  view.revision++; view.table.rows[0].private = { ...view.table.rows[0].private, value: '536870912' }; dashboard.update(view);
  assert.equal(dashboard.inspector.hidden, true, 'Collector updates must keep the selected main view');
  assert.equal(dashboard.element.querySelector('.overview-panel .memory-chart') === chart, true);
  control('Back to processes')!.click();
  assert.equal(dashboard.element.querySelector<HTMLElement>('.process-panel')!.hidden, false);
  assert.equal(dashboard.inspector.hidden, false);
  assert.equal(dashboard.element.querySelector<HTMLElement>('.splitter')!.hidden, false);
  assert.equal(dashboard.table.element.querySelector('[data-row-id="name:worker"]') === row, true);
  assert.equal(row.querySelector('.cell-2')!.textContent, '512.0 MiB');
  assert.equal((dashboard.element.querySelector('#process-filter') as HTMLInputElement).value, 'worker');
  assert.equal(dashboard.table.element.querySelector('[aria-selected="true"]')!.getAttribute('data-row-id'), 'name:worker');
  assert(control('Resume display'), 'Pause state survives the layout choice');
  assert.deepEqual(calls, [], 'A presentation switch must not change controller data or preferences');
  window.happyDOM.abort();
});

test('main overview uses the available panel height and preserves its range control', () => {
  const { window, calls, dashboard, view, control } = setup();
  const panel = dashboard.element.querySelector<HTMLElement>('.overview-panel')!;
  let panelHeight = 600;
  Object.defineProperty(panel, 'clientHeight', { get: () => panelHeight });
  Object.defineProperty(panel.querySelector('.memory-chart')!, 'clientWidth', { value: 860 });
  const drawing = panel.querySelector('.memory-chart > svg')!;
  control('Expand overview')?.click();
  assert(parseFloat(drawing.style.height) > 450, 'The main plot must use available height rather than the sidebar cap');
  const range = panel.querySelector<HTMLInputElement>('fieldset input[value="300"]'); assert(range, 'Overview range must be a toggle group'); range.click();
  assert.deepEqual(calls, [{ type: 'set_overview_range', seconds: 300 }]);
  panelHeight = 420; view.settings.overview_range = 300; view.settings.ui_zoom = 125; dashboard.update(view);
  assert(parseFloat(drawing.style.height) > 300, 'Main-view sizing follows the current available panel and text scale');
  assert(control('Back to processes'));
  control('Back to processes')!.click(); assert(parseFloat(drawing.style.height) <= panelHeight, 'The restored sidebar plot fits its allocated panel');
  window.happyDOM.abort();
});

test('keyboard commands return to visible process controls and Escape respects chart labels and dialogs', () => {
  const { window, calls, dashboard, control } = setup();
  const key = (target: Element, value: string, ctrlKey = false) => target.dispatchEvent(new window.KeyboardEvent('keydown', { key: value, ctrlKey, bubbles: true, cancelable: true }));
  const expand = control('Expand overview'); assert(expand, 'A keyboard-operable main-view action is required');
  expand.focus(); key(expand, 'Enter'); expand.click();
  key(expand, 'f', true);
  assert.equal(dashboard.inspector.hidden, false);
  assert.equal((document.activeElement as HTMLElement).id, 'process-filter');
  control('Expand overview')!.click(); key(expand, 'Escape');
  assert.equal(dashboard.inspector.hidden, false); assert.equal(document.activeElement === expand, true);
  control('Expand overview')!.click(); key(expand, 'F8');
  assert.equal(dashboard.inspector.hidden, false); assert.deepEqual(calls, [{ type: 'select_process_parent' }]);
  const chart = dashboard.element.querySelector<HTMLElement>('.overview-panel .memory-chart')!;
  Object.defineProperty(chart, 'clientWidth', { value: 300 });
  control('Expand overview')!.click(); chart.focus();
  const labels = chart.querySelector<HTMLElement>('.chart-labels')!;
  assert.equal(labels.hidden, false);
  key(chart, 'Escape'); assert.equal(labels.hidden, true); assert.equal(dashboard.inspector.hidden, true, 'Chart labels own the first Escape');
  key(chart, 'Escape'); assert.equal(dashboard.inspector.hidden, false, 'The next Escape can return to processes');
  control('Expand overview')!.click(); control('Details')!.click();
  const dialog = dashboard.element.querySelector('dialog')!;
  key(dialog, 'Escape'); assert.equal(dashboard.inspector.hidden, true, 'A modal owns its Escape');
  control('Close details')!.click(); key(expand, 'Escape'); assert.equal(dashboard.inspector.hidden, false);
  window.happyDOM.abort();
});

test('icon actions keep accessible names, tooltips and state changes', () => {
  const { window, calls, dashboard, view, control } = setup();
  for (const name of ['Pause display', 'Copy snapshot to clipboard', 'Expand overview', 'Show processes', 'Copy path', 'Reset view', 'Details', 'Close details']) {
    const action = control(name); assert(action, name);
    assert(action.title.length > 0, name + ' has a tooltip');
    assert.equal(action.querySelector('svg')!.getAttribute('aria-hidden'), 'true');
    assert.equal(action.textContent, '', name + ' uses an icon');
  }
  control('Pause display')!.click(); assert.deepEqual(calls.at(-1), { type: 'set_paused', value: true });
  view.paused = true; dashboard.update(view);
  assert(control('Resume display')); assert.equal(control('Resume display')!.title, 'Resume display');
  control('Expand overview')!.click(); assert(control('Back to processes'));
  window.happyDOM.abort();
});


test('overview chrome retains useful status without routine clutter or sample arrow buttons', () => {
  const { window, dashboard, view, control } = setup();
  assert(!dashboard.element.querySelector('h1'), 'App title is omitted');
  assert(!dashboard.element.querySelector('.footer-counts'), 'Session counts are omitted');
  assert.equal(control('Previous sample'), undefined); assert.equal(control('Next sample'), undefined);
  assert.equal(dashboard.element.querySelector<HTMLElement>('.live-status')!.hidden, true);
  assert.equal(control('Copy snapshot to clipboard')!.classList.contains('accent'), true);
  assert.equal(control('Copy snapshot to clipboard')!.classList.contains('primary'), false);
  view.paused = true; dashboard.update(view);
  assert.equal(dashboard.element.querySelector<HTMLElement>('.live-status')!.hidden, false);
  assert.match(dashboard.element.querySelector('.live-status')!.textContent!, /paused/i);
  view.paused = false; view.collection.status = 'failed'; view.collection.error = 'Sample unavailable'; dashboard.update(view);
  assert.match(dashboard.element.querySelector('.live-status')!.textContent!, /Sample unavailable/);
  window.happyDOM.abort();
});


test('details close stays in a separate header while long content scrolls', () => {
  const { window, dashboard, view, control } = setup();
  view.diagnostics.failures = Array.from({ length: 100 }, (_, i) => `Failure ${i}`); dashboard.update(view);
  control('Details')!.click();
  const dialog = dashboard.element.querySelector('dialog')!;
  const close = control('Close details')!;
  assert.equal(dialog.open, true);
  assert.equal(close.parentElement?.className, 'diagnostic-heading');
  assert.equal(close.parentElement?.nextElementSibling?.className, 'diagnostic-content');
  assert.equal(window.getComputedStyle(dialog).overflow, 'hidden');
  assert.equal(window.getComputedStyle(dialog.querySelector('.diagnostic-content')!).overflow, 'auto');
  close.click(); assert.equal(dialog.open, false);
  control('Details')!.click(); assert.equal(dialog.open, true);
  dialog.dispatchEvent(new window.Event('cancel', { cancelable: true })); assert.equal(dialog.open, false);
  window.happyDOM.abort();
});


test('50 percent scale is selectable and is the shortcut and wheel lower bound', () => {
  const { window, calls, dashboard, view } = setup();
  const zoom = dashboard.element.querySelector<HTMLSelectElement>('select[aria-label="View scale"]')!;
  assert([...zoom.options].some(option => option.value === '50'));
  zoom.value = '50'; zoom.dispatchEvent(new window.Event('change', { bubbles: true }));
  assert.deepEqual(calls.at(-1), { type: 'set_ui_zoom', value: 50 });
  for (const scale of [75, 50]) {
    view.settings.ui_zoom = scale; dashboard.update(view);
    dashboard.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: '-', ctrlKey: true, bubbles: true }));
    assert.deepEqual(calls.at(-1), { type: 'set_ui_zoom', value: 50 });
    dashboard.element.dispatchEvent(new window.WheelEvent('wheel', { deltaY: 1, ctrlKey: true, bubbles: true }));
    assert.deepEqual(calls.at(-1), { type: 'set_ui_zoom', value: 50 });
  }
  assert.equal(document.documentElement.style.fontSize, '7px');
  window.happyDOM.abort();
});
