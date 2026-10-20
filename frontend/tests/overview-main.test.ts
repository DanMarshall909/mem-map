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
  const range = panel.querySelector('select')!; range.value = '300'; range.dispatchEvent(new window.Event('change', { bubbles: true }));
  assert.deepEqual(calls, [{ type: 'set_overview_range', seconds: 300 }]);
  panelHeight = 420; view.settings.overview_range = 300; view.settings.ui_zoom = 125; dashboard.update(view);
  assert(parseFloat(drawing.style.height) > 300, 'Main-view sizing follows the current available panel and text scale');
  assert(control('Back to processes'));
  control('Back to processes')!.click(); assert(parseFloat(drawing.style.height) <= 250, 'The restored sidebar uses its normal height budget');
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
  for (const name of ['Pause display', 'Export CSV', 'Expand overview', 'Show processes', 'Copy path', 'Reset view', 'Details', 'Close details']) {
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
