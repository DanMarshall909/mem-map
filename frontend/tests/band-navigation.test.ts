import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { MemoryChart } from '../src/charts.ts';
import { fixture } from './fixture.ts';

function setup(width = 1100, height = 900) {
  const window = new Window({ width: 1600, height: 1000 });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const actions: unknown[] = [], view = fixture();
  view.overview.layers = [{ name: 'Other used RAM', id: null }, { name: 'Other processes', id: null }, ...Array.from({ length: 12 }, (_, i) => ({ name: `program-${i}.exe`, id: `program:${i}` }))];
  const amounts = ['9007199254740993', '17', ...Array.from({ length: 12 }, (_, i) => String((i + 1) * 11))];
  view.overview.samples = view.overview.samples.map(sample => ({ ...sample, used: amounts.reduce((sum, v) => sum + BigInt(v), 0n).toString(), layers: [...amounts], baseline: { at: sample.at - 300, layers: amounts.map(v => (BigInt(v) + 3n).toString()) } }));
  const chart = new MemoryChart('overview', a => actions.push(a));
  const panel = document.createElement('section'); panel.append(chart.element); document.body.append(panel);
  Object.defineProperty(chart.element, 'clientWidth', { get: () => width, configurable: true });
  Object.defineProperty(panel, 'clientHeight', { get: () => height, configurable: true });
  chart.update(view);
  return { window, actions, view, chart, resize: (w: number, h: number) => { width = w; height = h; chart.update(view); } };
}

test('named bands and keyboard selection open a program; residuals have no process command', () => {
  const { window, actions, chart } = setup();
  const bands = chart.element.querySelectorAll('.chart-areas path');
  bands[3].dispatchEvent(new window.MouseEvent('dblclick', { bubbles: true }));
  assert.deepEqual(actions, [{ type: 'open_overview_process', id: 'program:0' }]);
  bands[1].dispatchEvent(new window.MouseEvent('dblclick', { bubbles: true }));
  bands[2].dispatchEvent(new window.MouseEvent('dblclick', { bubbles: true }));
  assert.equal(actions.length, 1);
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  assert.deepEqual(actions.at(-1), { type: 'open_overview_process', id: 'program:1' });
  window.happyDOM.abort();
});

test('normal overview shows only names and sizes on hover/focus without changing plot size', () => {
  const { window, chart } = setup(600, 600);
  const legend = chart.element.querySelector('.band-labels')!, drawing = chart.element.querySelector('svg')!;
  const box = drawing.getAttribute('viewBox'); assert.equal(legend.getAttribute('display'), 'none');
  chart.element.dispatchEvent(new window.PointerEvent('pointerenter', { pointerType: 'mouse' }));
  assert.notEqual(legend.getAttribute('display'), 'none');
  assert(legend.querySelector('.legend-name')); assert(legend.querySelector('.legend-value'));
  assert.equal(legend.querySelectorAll('.legend-share,.legend-change').length, 0);
  assert.equal(drawing.getAttribute('viewBox'), box);
  chart.element.dispatchEvent(new window.PointerEvent('pointerleave'));
  assert.equal(legend.getAttribute('display'), 'none');
  chart.element.focus(); assert.notEqual(legend.getAttribute('display'), 'none');
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert.equal(legend.getAttribute('display'), 'none');
  chart.setExpanded(true); assert(legend.querySelector('.legend-share')); assert(legend.querySelector('.legend-change'));
  window.happyDOM.abort();
});

test('commit folding conserves measured totals and unavailable attribution stays unknown', () => {
  const { window, chart, view, resize } = setup(300, 200);
  view.settings.overview_metric = 'private_commit'; view.overview.capacity = '0';
  view.overview.layers = view.overview.layers.slice(1);
  view.overview.samples = view.overview.samples.map(sample => ({ ...sample, capacity: '0', available: null,
    layers: sample.layers!.slice(1), used: sample.layers!.slice(1).reduce((sum, n) => sum + BigInt(n), 0n).toString(),
    baseline: { ...sample.baseline!, layers: sample.baseline!.layers.slice(1) } }));
  resize(300, 200); chart.setExpanded(true);
  const labels = [...chart.element.querySelectorAll('.band-label title')];
  assert(labels.length < view.overview.layers.length);
  const sum = labels.reduce((n, label) => n + BigInt(label.textContent!.match(/; (\d+) raw bytes/)![1]), 0n);
  assert.equal(sum.toString(), view.overview.samples.at(-1)!.used);
  assert.doesNotMatch(chart.element.querySelector('output')!.textContent!, /Available|Capacity|of physical RAM/);
  view.overview.samples.at(-1)!.layers = null; chart.update(view);
  assert([...chart.element.querySelectorAll('.legend-value')].every(label => label.textContent === '—'));
  window.happyDOM.abort();
});

test('responsive bands fold exact hidden allocations and baselines into Other while preserving identities and colors', () => {
  const { window, chart, view, resize } = setup(); chart.setExpanded(true);
  const large = [...chart.element.querySelectorAll('.band-label')]; assert.equal(large.length, 14);
  const color = chart.element.querySelectorAll('.chart-areas path')[3].getAttribute('fill');
  resize(300, 200);
  const small = [...chart.element.querySelectorAll('.band-label')]; assert(small.length < large.length && small.length >= 3);
  const kept = small.length - 2;
  const expected = 17n + Array.from({ length: 12 - kept }, (_, j) => BigInt((kept + j + 1) * 11)).reduce((a, b) => a + b, 0n);
  assert.match(small[1].querySelector('title')!.textContent!, new RegExp(`${expected} raw bytes`));
  assert.equal(chart.element.querySelectorAll('.chart-areas path')[3].getAttribute('fill'), color);
  const foldedBefore = expected + BigInt(13 - kept) * 3n;
  assert.match(small[1].querySelector('.legend-change')!.textContent!, /^−/);
  assert.match(small[1].querySelector('title')!.textContent!, new RegExp(`${foldedBefore} baseline bytes`));
  assert.equal(view.overview.layers.length, 14, 'Projection does not mutate the backend view');
  resize(1100, 900); assert.equal(chart.element.querySelectorAll('.band-label').length, 14);
  window.happyDOM.abort();
});

test('excluding Other processes keeps short legends bounded without putting visible programs into Excluded', () => {
  const { window, chart, view, resize } = setup();
  view.overview.layers.splice(1, 1); view.overview.layers.push({name:'Excluded', id:null});
  for (const sample of view.overview.samples) {
    const removed = sample.layers!.splice(1, 1)[0]; sample.layers!.push(removed);
    if (sample.baseline) { const before = sample.baseline.layers.splice(1, 1)[0]; sample.baseline.layers.push(before); }
  }
  chart.update(view); chart.setExpanded(true); resize(300, 200);
  const labels = [...chart.element.querySelectorAll('.band-label')];
  assert(labels.length <= 5, 'All legend rows must remain reachable in a short chart');
  const excluded = labels.find(label => label.querySelector('.legend-name')?.textContent === 'Excluded')!;
  assert.match(excluded.querySelector('title')!.textContent!, /17 raw bytes/);
  const more = labels.find(label => label.querySelector('.legend-name')?.textContent === 'More programs');
  assert(more, 'Responsive overflow remains a distinct visible allocation');
  assert.match(more.querySelector('title')!.textContent!, /baseline bytes/);
  resize(300, 100);
  const drawing = chart.element.querySelector('svg')!;
  const height = Number(drawing.getAttribute('viewBox')!.split(' ')[3]);
  assert([...drawing.querySelectorAll('.band-label rect')].every(rect => Number(rect.getAttribute('y')) + Number(rect.getAttribute('height')) <= height), 'Every legend rectangle fits even at minimum chart height');
  window.happyDOM.abort();
});

test('band navigation leaves expanded overview and processes explicit row reveal responses', () => {
  const { window, view } = setup(); const actions: unknown[] = [];
  const dashboard = new Dashboard(a => actions.push(a), () => {}); document.body.append(dashboard.element); dashboard.update(view);
  dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Expand overview"]')!.click();
  const named = dashboard.element.querySelectorAll('.overview-panel .chart-areas path')[3];
  named.dispatchEvent(new window.MouseEvent('dblclick', { bubbles: true }));
  assert(!dashboard.element.classList.contains('overview-expanded'));
  assert.deepEqual(actions.at(-1), { type: 'open_overview_process', id: 'program:0' });
  window.happyDOM.abort();
});

test('a double-click targeted at the persistent SVG after live redraw opens the band under the pointer', () => {
  const { window, chart, view, actions } = setup();
  chart.update({ ...view, revision: view.revision + 1 });
  const drawing = chart.element.querySelector('svg')!, band = chart.element.querySelector('.chart-areas path[data-process-id]')!;
  Object.defineProperty(document, 'elementFromPoint', { value: () => band, configurable: true });
  drawing.dispatchEvent(new window.MouseEvent('dblclick', { clientX: 200, clientY: 200, bubbles: true }));
  assert.deepEqual(actions, [{ type: 'open_overview_process', id: 'program:0' }]);
  window.happyDOM.abort();
});

test('expanded legend navigation survives its text being refreshed between clicks', () => {
  const { window, chart, actions } = setup(); chart.setExpanded(true);
  const legend = chart.element.querySelector('.band-labels')!, entry = legend.querySelector('[data-process-id]')!;
  Object.defineProperty(document, 'elementFromPoint', { value: () => entry, configurable: true });
  legend.dispatchEvent(new window.MouseEvent('dblclick', { clientX: 200, clientY: 200, bubbles: true }));
  assert.deepEqual(actions, [{ type: 'open_overview_process', id: 'program:0' }]);
  window.happyDOM.abort();
});

test('Enter on the chart info button retains native activation rather than opening a program', () => {
  const { window, chart, actions } = setup(300, 200);
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
  const info = chart.element.querySelector<HTMLButtonElement>('.chart-label-toggle')!; info.focus();
  const enter = new window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
  info.dispatchEvent(enter);
  assert.equal(enter.defaultPrevented, false, 'Chart must not suppress the focused button default action');
  assert.deepEqual(actions, []);
  window.happyDOM.abort();
});
