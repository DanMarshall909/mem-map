import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';
import { fixture } from './fixture.ts';
import { ProcessTreeGrid } from '../src/treegrid.ts';
import { Dashboard } from '../src/dashboard.ts';
import { MemoryChart } from '../src/charts.ts';

function dom() {
  const window = new Window({ width: 1440, height: 900 });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const style = document.createElement('style'); style.textContent = readFileSync(new URL('../src/styles.css', import.meta.url), 'utf8'); document.head.append(style);
  return window;
}

test('memory proportion decorates the whole keyed row while exact values remain unobstructed', () => {
  const window = dom(); const grid = new ProcessTreeGrid(() => {}); document.body.append(grid.element);
  const view = fixture(); view.table.rows[0].private = { ...view.table.rows[0].private, value: '536870912' }; grid.update(view);
  const row = grid.element.querySelector<HTMLElement>('[data-row-id="name:worker"]')!;
  assert.equal(row.style.getPropertyValue('--memory-fill'), '50%', 'Half the largest working set paints half the row');
  assert.match(window.getComputedStyle(row).backgroundImage, /linear-gradient/, 'The fill belongs to the full row background');
  assert.equal(row.querySelectorAll('.memory-bar').length, 0, 'The old in-cell line must not consume numeric space');
  assert.equal(row.querySelector('.cell-2')!.textContent, '512.0 MiB');
  row.focus(); view.table.rows[0].private = { ...view.table.rows[0].private, value: null }; grid.update(view);
  assert.equal(grid.element.querySelector('[data-row-id="name:worker"]') === row, true, 'Updates retain the same focused row');
  assert.equal(row.style.getPropertyValue('--memory-fill'), '0%', 'Unavailable values leave no stale memory fill');
  assert.equal(row.querySelector('.cell-2')!.textContent, '—');
  window.happyDOM.abort();
});

test('table and inspector percentages use the earlier complete reading without fabricating a baseline', async t => {
  const window = dom(); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element);
  const cases = [
    { name: 'growth', current: '125', delta: '25', label: '+25.0%' },
    { name: 'decline', current: '75', delta: '-25', label: '−25.0%' },
    { name: 'exact zero', current: '100', delta: '0', label: '0.0%' },
    { name: 'zero remains zero', current: '0', delta: '0', label: '0.0%' },
    { name: 'growth from zero', current: '100', delta: '100', label: 'New' },
    { name: 'unavailable delta', current: '100', delta: null, label: '—' },
    { name: 'unknown current', current: null, delta: '25', label: '—' },
    { name: 'partial current', current: '125', delta: '25', label: '—', partial: true },
    { name: 'confirmed exit', current: null, delta: '-100', label: '−100.0%', exited: true },
    { name: 'inconsistent baseline', current: '25', delta: '50', label: '—' },
    { name: 'above JavaScript integer precision', current: '112589990684262400', delta: '22517998136852480', label: '+25.0%' },
    { name: 'tiny real increase', current: '1000001', delta: '1', label: '+<0.1%' },
  ];
  for (const example of cases) await t.test(example.name, () => {
    const view = fixture(), row = view.table.rows[0];
    row.private = { ...row.private, value: example.current, measured: example.partial ? 1 : 2 };
    row.change = { value: example.delta, reason: example.delta === null ? 'Need history' : 'Original measurement reason' };
    row.lifecycle = example.exited ? 'exited' : 'live';
    view.selected = { ...view.selected!, private: row.private, change: row.change, lifecycle: row.lifecycle };
    dashboard.update(view);
    const cell = dashboard.table.element.querySelector<HTMLElement>('[data-row-id="name:worker"] .cell-5')!;
    const inspectorValue = dashboard.inspector.querySelectorAll<HTMLElement>('.summary-values dd')[2];
    assert.equal(cell.textContent, example.label); assert.equal(inspectorValue.textContent, example.label);
    assert.equal(cell.querySelectorAll('.change-bar').length, 0);
    if (example.delta !== null) assert.match(cell.title, new RegExp(`${example.delta} raw bytes`));
    assert.match(cell.title, /Original measurement reason|Need history/);
    if (example.delta === '0') {
      assert.match(window.getComputedStyle(cell).color, /^(?:#ffffff|rgb\(255, 255, 255\))$/i);
      assert.match(window.getComputedStyle(inspectorValue).color, /^(?:#ffffff|rgb\(255, 255, 255\))$/i);
    }
  });
  window.happyDOM.abort();
});

test('percentage color grows from white toward red for growth and green for decline', () => {
  const window = dom(); const grid = new ProcessTreeGrid(() => {}); document.body.append(grid.element);
  const color = (current: string, delta: string) => {
    const view = fixture(); view.table.rows[0].private = { ...view.table.rows[0].private, value: current };
    view.table.rows[0].change = { value: delta, reason: null }; grid.update(view);
    return window.getComputedStyle(grid.element.querySelector('[data-row-id="name:worker"] .cell-5')!).color.match(/\d+/g)!.map(Number);
  };
  const smallUp = color('105', '5'), bigUp = color('150', '50');
  assert(bigUp[0] > bigUp[1], 'Memory growth is red'); assert(bigUp[1] < smallUp[1], 'Larger growth is more strongly red');
  const smallDown = color('95', '-5'), bigDown = color('50', '-50');
  assert(bigDown[1] > bigDown[0], 'Memory decline is green'); assert(bigDown[0] < smallDown[0], 'Larger decline is more strongly green');
  const luminance = (channels: number[]) => channels.map(channel => { const c = channel / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4; }).reduce((sum, value, i) => sum + value * [.2126, .7152, .0722][i], 0);
  // Conservative lightest supported row tone: hover plus a full green memory fill.
  const background = luminance([60, 78, 75]);
  for (const ink of [color('200', '100'), color('1', '-99')]) assert((luminance(ink) + .05) / (background + .05) >= 4.5, 'Percentage text must remain readable over the memory background');
  window.happyDOM.abort();
});

test('compact charts devote width to the plot and reveal stable labels by hover, focus and tap', async () => {
  const window = dom(); const calls: unknown[] = [];
  const chart = new MemoryChart('overview', action => calls.push(action)); document.body.append(chart.element);
  let width = 225; Object.defineProperty(chart.element, 'clientWidth', { get: () => width });
  const view = fixture(); view.settings.ui_zoom = 75; chart.update(view);
  const drawing = chart.element.querySelector('svg')!;
  assert.equal(chart.element.classList.contains('chart-compact'), true, 'Compactness follows available chart width divided by text scale');
  assert(parseFloat(drawing.style.height) > 150, 'The compact plot uses the space released by its labels instead of keeping the old height cap');
  const labels = chart.element.querySelector<HTMLElement>('.chart-labels')!;
  assert.equal(labels.hidden, true, 'Legend and readings consume no normal-flow space while idle');
  const firstGridline = drawing.querySelector('line')!;
  const viewBox = drawing.getAttribute('viewBox')!;
  const plotFraction = (Number(firstGridline.getAttribute('x2')) - Number(firstGridline.getAttribute('x1'))) / Number(viewBox.split(' ')[2]);
  assert(plotFraction > .85, 'The plot gets almost all of a narrow chart, without an annotation gutter');
  chart.element.dispatchEvent(new window.PointerEvent('pointerenter', { pointerType: 'mouse' }));
  assert.equal(labels.hidden, false); assert.equal(window.getComputedStyle(labels).position, 'fixed');
  assert.equal(drawing.getAttribute('viewBox'), viewBox, 'Hover must not move the pointer sampling coordinates');
  chart.element.dispatchEvent(new window.PointerEvent('pointerleave', { pointerType: 'mouse' })); assert.equal(labels.hidden, true);
  chart.element.focus(); assert.equal(labels.hidden, false, 'Keyboard focus also exposes the legend');
  const data = chart.element.querySelector<HTMLElement>('.chart-data')!;
  data.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.match(data.textContent!, /5m ago/, 'Keyboard sample navigation remains available in the revealed labels');
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert.equal(labels.hidden, true); assert.equal(document.activeElement === chart.element, true, 'Escape keeps a useful keyboard focus target');
  const outside = document.createElement('button'); document.body.append(outside); outside.focus(); await Promise.resolve();
  const toggle = chart.element.querySelector<HTMLButtonElement>('.chart-label-toggle')!;
  toggle.click(); assert.equal(labels.hidden, false); assert.equal(toggle.getAttribute('aria-expanded'), 'true');
  toggle.click(); assert.equal(labels.hidden, true); assert.equal(toggle.getAttribute('aria-expanded'), 'false');
  assert.equal(drawing.getAttribute('viewBox'), viewBox, 'Tapping labels must not resize the plot');
  toggle.focus(); toggle.click(); assert.equal(labels.hidden, false);
  outside.focus(); await Promise.resolve(); assert.equal(labels.hidden, true, 'Leaving the chart by keyboard dismisses pinned labels');
  width = 600; chart.update(view);
  assert.equal(chart.element.classList.contains('chart-compact'), false); assert.equal(labels.hidden, false, 'Roomy charts retain their inline labels');
  assert.equal(toggle.hidden, true); assert.equal(calls.length, 0, 'Inspecting chart labels changes no domain state');
  window.happyDOM.abort();
});

test('flat panels and controls retain input, selection and keyboard focus affordances', () => {
  const window = dom(); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element); dashboard.update(fixture());
  for (const selector of ['.panel', '.process-grid', '.legend-entry', '.segments span', '.header-actions button', '.inspector-heading button', 'select']) {
    for (const element of dashboard.element.querySelectorAll(selector)) {
      const style = window.getComputedStyle(element);
      assert.equal(parseFloat(style.borderTopWidth) || 0, 0, `${selector} has no enclosing box border`);
      assert.equal(parseFloat(style.borderRightWidth) || 0, 0);
      assert.equal(parseFloat(style.borderLeftWidth) || 0, 0);
    }
  }
  const filter = dashboard.element.querySelector('input[type=search]')!;
  assert(parseFloat(window.getComputedStyle(filter).borderBottomWidth) > 0, 'A restrained underline still identifies editable input');
  const row = dashboard.table.element.querySelector<HTMLElement>('.process-row.selected')!; row.focus();
  assert.equal(parseFloat(window.getComputedStyle(row).borderBottomWidth) || 0, 0, 'Rows avoid a boxed spreadsheet appearance');
  assert(parseFloat(window.getComputedStyle(row).borderLeftWidth) > 0, 'Selection retains its accent stripe');
  assert(parseFloat(window.getComputedStyle(row).outlineWidth) > 0, 'Keyboard focus remains visible');
  window.happyDOM.abort();
});
