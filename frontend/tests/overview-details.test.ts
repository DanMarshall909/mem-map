import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { MemoryChart } from '../src/charts.ts';
import { fixture } from './fixture.ts';
function setup(width = 1000, height = 650) {
  const window = new Window({ width: 1440, height: 900 });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const panel = document.createElement('section'), chart = new MemoryChart('overview', () => {});
  Object.defineProperty(panel, 'clientHeight', { get: () => height });
  Object.defineProperty(chart.element, 'clientWidth', { get: () => width });
  panel.append(chart.element); document.body.append(panel); const view = fixture(); chart.update(view);
  return { window, chart, view, drawing: chart.element.querySelector('svg')! };
}
test('expanded plots add time and memory divisions while sidebar density stays restrained', () => {
  const { window, chart, drawing } = setup();
  const timeLabels = () => [...drawing.querySelectorAll('text')].filter(text => /^\d+[smh](?: \d+[sm])?$/.test(text.textContent ?? ''));
  const memoryLabels = () => [...drawing.querySelectorAll('text')].filter(text => /^\d+\.\d$/.test(text.textContent ?? ''));
  assert.equal(timeLabels().length, 2); assert.equal(memoryLabels().length, 4);
  chart.setExpanded(true);
  assert(timeLabels().length >= 5, 'The wide main plot needs intermediate time labels');
  assert(memoryLabels().length >= 6, 'The tall main plot needs finer memory divisions');
  assert.equal(new Set(timeLabels().map(text => text.textContent)).size, timeLabels().length);
  assert.equal(timeLabels()[0].textContent, '10m'); assert.equal(timeLabels().at(-1)!.textContent, '0s');
  assert.equal(memoryLabels()[0].textContent, '0.0'); assert.equal(memoryLabels().at(-1)!.textContent, '32.0');
  chart.setExpanded(false); assert.equal(timeLabels().length, 2); assert.equal(memoryLabels().length, 4);
  window.happyDOM.abort();
});
test('compact and very short timelines avoid crowded or duplicate tick labels', () => {
  const narrow = setup(300); narrow.chart.setExpanded(true);
  const times = (drawing: SVGSVGElement) => [...drawing.querySelectorAll('text')].map(text => text.textContent).filter(value => /^\d+[smh](?: \d+[sm])?$/.test(value ?? ''));
  assert.equal(narrow.chart.element.classList.contains('chart-compact'), true); assert.equal(times(narrow.drawing).length, 2);
  narrow.window.happyDOM.abort();
  const { window, chart, view, drawing } = setup(); chart.setExpanded(true);
  view.overview.start = 600; view.overview.end = 600; chart.update(view);
  assert.deepEqual(times(drawing), ['0s']);
  view.overview.start = 599; chart.update(view); assert.deepEqual(times(drawing), ['1s', '0s']);
  window.happyDOM.abort();
});
test('expanded readings and cursor inspect one actual sample by keyboard or pointer and return to latest', () => {
  const { window, chart, view, drawing } = setup();
  view.overview.samples[1] = { ...view.overview.samples[1], capacity: '17179869184', used: '8589934592', available: '8589934592', layers: ['6442450944', '2147483648'], measured: 7, observed: 9, reason: 'Historical coverage note' };
  chart.setExpanded(true); chart.update(view);
  const worker = chart.element.querySelectorAll<SVGGElement>('.band-label')[1];
  const data = chart.element.querySelector('output')!; chart.element.focus();
  const box = drawing.getAttribute('viewBox');
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  const readout = data;
  assert.match(readout.textContent!, /capacity 16.00 GiB/i); assert.match(readout.textContent!, /Used 8.00 GiB/); assert.match(readout.textContent!, /Available 8.00 GiB/);
  assert.match(worker.textContent!, /worker.exe.*2.00 GiB.*12.5%/); assert.match(worker.querySelector('title')!.textContent!, /2147483648 raw bytes/);
  assert.match(chart.element.querySelector('.chart-description')!.textContent!, /7 of 9 processes measured/);
  assert.match(chart.element.querySelector('.chart-description')!.textContent!, /processes measured/i);
  assert.match(data.textContent!, /Historical coverage note/); assert.match(worker.textContent!, /12.5%/);
  const horizontal = drawing.querySelector('line')!, cursor = drawing.querySelector('.chart-cursor line');
  assert(cursor, 'The inspected time must be marked on the plot');
  assert.equal(Number(cursor.getAttribute('x1')), (Number(horizontal.getAttribute('x1')) + Number(horizontal.getAttribute('x2'))) / 2);
  assert.equal(drawing.querySelectorAll('.chart-cursor circle').length, 1);
  assert.equal(drawing.getAttribute('viewBox'), box, 'Inspecting a sample cannot reflow the plot');
  const outside = document.createElement('button'); document.body.append(outside); outside.focus();
  Object.defineProperty(drawing, 'getBoundingClientRect', { value: () => ({ left: 0, top: 0, width: 1000, height: 650 }) });
  drawing.dispatchEvent(new window.PointerEvent('pointermove', { clientX: Number(cursor.getAttribute('x1')), clientY: 300, bubbles: true }));
  assert.match(worker.textContent!, /12.5%/);
  drawing.dispatchEvent(new window.PointerEvent('pointermove', { clientX: Number(cursor.getAttribute('x1')) + 10, clientY: 300, bubbles: true }));
  chart.element.focus(); chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.equal(Number(drawing.querySelector('.chart-cursor line')!.getAttribute('x1')), Number(horizontal.getAttribute('x1')), 'ArrowLeft moves before the inspected sample even when the pointer is slightly after its time');
  outside.focus();
  drawing.dispatchEvent(new window.PointerEvent('pointerleave'));
  assert.match(readout.textContent!, /Used 24.00 GiB/); assert.match(worker.textContent!, /1.00 GiB.*3.1%/);
  assert.equal(drawing.querySelectorAll('.chart-cursor line').length, 0, 'Latest view has no stale inspection cursor');
  chart.setExpanded(false); assert.equal(chart.element.querySelector('.band-labels')!.getAttribute('display'), 'none'); window.happyDOM.abort();
});
test('expanded unknown samples retain reasons without zero shares or invented band attribution', () => {
  const { window, chart, view, drawing } = setup(); chart.setExpanded(true);
  view.overview.samples[2] = { ...view.overview.samples[2], used: null, available: null, layers: null, measured: 1, observed: 4, reason: 'System reading unavailable' };
  chart.update(view);
  assert.match(chart.element.querySelector('output')!.textContent!, /Used —/);
  assert.match(chart.element.querySelectorAll('.band-label')[1].textContent!, /worker.exe.*—/);
  assert.doesNotMatch(chart.element.querySelector('.band-labels')!.textContent!, /0.0%/);
  assert.match(chart.element.querySelector('.chart-description')!.textContent!, /1 of 4 processes measured/);
  assert.match(chart.element.querySelector('output')!.textContent!, /System reading unavailable/);
  const data = chart.element.querySelector('output')!;
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  assert.equal(drawing.querySelectorAll('.chart-cursor circle').length, 0, 'Unavailable used memory has no invented point');
  view.overview.samples[2] = { ...view.overview.samples[2], capacity: '17179869184', used: '8589934592', available: '8589934592', reason: 'Attribution unavailable' }; chart.update(view);
  assert.match(chart.element.querySelector('output')!.textContent!, /Used 8.00 GiB/);
  for (const entry of chart.element.querySelectorAll('.band-label')) { assert.match(entry.textContent!, /—/); assert.doesNotMatch(entry.textContent!, /%/); }
  assert.equal(drawing.querySelectorAll('.chart-cursor circle').length, 1, 'A valid physical total remains inspectable when attribution is unknown');
  view.overview.samples[2] = { ...view.overview.samples[2], capacity: '0', used: '1048576', available: null, layers: ['0', '1048576'] }; chart.update(view);
  assert([...chart.element.querySelectorAll('.legend-share')].every(cell => cell.textContent === '\u2014'));
  view.overview.samples = []; chart.update(view);
  assert.match(chart.element.querySelector('output')!.textContent!, /No system memory samples/);
  assert.match(chart.element.querySelectorAll('.band-label')[1].textContent!, /worker.exe.*—/);
  assert.equal(drawing.querySelectorAll('.chart-cursor line').length, 0); window.happyDOM.abort();
});
test('physical-memory shares round exact bytes beyond floating-point precision', () => {
  const { window, chart, view } = setup(); chart.setExpanded(true);
  const sample = view.overview.samples[2]; sample.capacity = '18014398509481986000';
  sample.used = '9007199254740992'; sample.layers = ['0', sample.used]; chart.update(view);
  const worker = chart.element.querySelectorAll('.band-label')[1]; assert.equal(worker.querySelector('.legend-share')!.textContent, '0.0%');
  sample.used = '9007199254740993'; sample.layers = ['0', sample.used]; chart.update(view);
  assert.equal(worker.querySelector('.legend-share')!.textContent, '0.1%'); window.happyDOM.abort();
});
test('direct labels replace duplicated summaries, legends and visible chart data', () => {
  const { window, chart, drawing } = setup(1000, 400); chart.setExpanded(true);
  assert(!chart.element.querySelector('.legend, .chart-readout, details'));
  assert.equal(chart.element.querySelector('output')!.classList.contains('sr-only'), true);
  assert.doesNotMatch([...drawing.querySelectorAll('text')].map(node => node.textContent).join(' '), /Capacity:|Available|elapsed|recorded/);
  assert.equal(chart.element.tabIndex, 0);
  const labels = [...drawing.querySelectorAll('.band-label')];
  assert.equal(labels.length, 2);
  for (const label of labels) assert.match(label.textContent!, /GiB|MiB/);
  const positions = labels.map(label => Number(label.getAttribute('data-label-y'))).sort((a,b) => a-b);
  assert(positions.every((y,i) => !i || y-positions[i-1] >= 24), 'Thin-band labels do not overlap');
  window.happyDOM.abort();
});
test('legend sits over the top-left plot corner only in expanded overview', () => {
  const { window, chart, drawing } = setup();
  const legend = drawing.querySelector('.band-labels')!;
  assert.equal(legend.getAttribute('display'), 'none', 'Sidebar omits the band legend entirely');
  chart.setExpanded(true);
  assert.equal(legend.getAttribute('display'), null);
  const labels = [...legend.querySelectorAll('.band-label')];
  const line = drawing.querySelector('line')!;
  const left = Number(line.getAttribute('x1')), top = Math.min(...[...drawing.querySelectorAll('line[y1]')].map(line => Number(line.getAttribute('y1'))));
  assert.equal(Number(labels[0].querySelector('rect')!.getAttribute('x')), left + 10);
  assert.equal(Number(labels[0].getAttribute('data-label-y')), top + 10);
  assert.equal(labels.length, fixture().overview.layers.length);
  assert.equal(legend.querySelectorAll('path').length, 0, 'A corner legend needs no band leader lines');
  chart.setExpanded(false); assert.equal(legend.getAttribute('display'), 'none');
  window.happyDOM.abort();
});

test('legend has single aligned rows, 20-character names and sample-relative change percentages', () => {
  const { window, chart, view, drawing } = setup();
  view.overview.layers[1].name = 'a-very-long-program-description.exe';
  view.overview.samples[0].layers![1] = '1073741824';
  view.overview.samples[1].layers![1] = '2147483648';
  view.overview.samples[2].layers![1] = '3221225472';
  view.overview.samples[2].baseline = { at: 300, layers: ['0', '2147483648'] };
  chart.setExpanded(true); chart.update(view);
  const entries = [...drawing.querySelectorAll('.band-label')];
  const names = entries.map(entry => entry.querySelector('.legend-name')!);
  assert(names.every(name => [...name.textContent!].length <= 20));
  for (const entry of entries) {
    const cells = [...entry.querySelectorAll('text')]; assert.equal(cells.length, 4);
    assert.equal(new Set(cells.map(cell => cell.getAttribute('y'))).size, 1);
  }
  for (const field of ['legend-value','legend-share','legend-change']) assert.equal(new Set(entries.map(entry => entry.querySelector('.'+field)!.getAttribute('x'))).size, 1);
  assert.equal(entries[1].querySelector('.legend-change')!.textContent, '+50.0%');
  chart.element.focus(); chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.equal(entries[1].querySelector('.legend-change')!.textContent, '+100.0%');
  drawing.dispatchEvent(new window.PointerEvent('pointerleave'));
  assert.equal(entries[1].querySelector('.legend-change')!.textContent, '+50.0%', 'Pointer departure restores latest even while chart retains focus');
  assert.equal(drawing.querySelectorAll('.chart-cursor line').length, 0);
  view.overview.samples[1].layers = null; view.overview.samples[2].baseline = null; chart.update(view);
  assert.equal(entries[1].querySelector('.legend-change')!.textContent, '—');
  window.happyDOM.abort();
});

test('legend prefers supplied program names and uses valid recent baselines from the selected interval', () => {
  const { window, chart, view, drawing } = setup(); chart.setExpanded(true);
  const layer = view.overview.layers[1]; layer.program_name = 'A friendly program name longer than twenty';
  view.settings.change_seconds = 60;
  const sample = view.overview.samples[2]; sample.layers![1] = '150';
  sample.baseline = { at: 536, layers: ['0', '100'] };
  view.overview.samples[1].at = 536; view.overview.samples[1].layers![1] = '100';
  chart.update(view);
  const entry = drawing.querySelectorAll('.band-label')[1];
  assert.equal([...entry.querySelector('.legend-name')!.textContent!].length, 20);
  assert.match(entry.querySelector('title')!.textContent!, /A friendly program name.*worker.exe/);
  assert.equal(entry.querySelector('.legend-change')!.textContent, '+50.0%', 'Four-second-old baseline is usable');
  view.overview.samples[1].at = 535; sample.baseline = null; chart.update(view);
  assert.equal(entry.querySelector('.legend-change')!.textContent, '\u2014', 'Older baselines cannot fabricate a change');
  layer.program_name = '   '; view.overview.samples[1].at = 540; view.overview.samples[1].layers![1] = '0'; sample.baseline = { at: 540, layers: ['0', '0'] }; chart.update(view);
  assert.equal(entry.querySelector('.legend-name')!.textContent, 'worker.exe');
  assert.equal(entry.querySelector('.legend-change')!.textContent, 'New');
  sample.layers![1] = '0'; chart.update(view); assert.equal(entry.querySelector('.legend-change')!.textContent, '0.0%');
  sample.layers = null; chart.update(view); assert.equal(entry.querySelector('.legend-change')!.textContent, '\u2014');
  window.happyDOM.abort();
});

for (const kind of ['overview', 'history'] as const) test(`the paused first sample anchors its single zero-age tick at the right: ${kind}`, () => {
    const { window, chart, view } = setup();
    view.paused = true; view.overview.start = view.overview.end = 0;
    view.overview.samples = view.overview.samples.slice(0, 1);
    view.history!.start = view.history!.end = 0; view.history!.points = view.history!.points.slice(0, 1);
    const tested = kind === 'overview' ? chart : new MemoryChart('history', () => {});
    if (kind === 'history') { document.body.append(tested.element); Object.defineProperty(tested.element, 'clientWidth', { value: 1000 }); }
    tested.update(view);
    const drawing = tested.element.querySelector('svg')!, grid = drawing.querySelector('line')!;
    const zero = [...drawing.querySelectorAll('.chart-annotations text')].filter(text => text.textContent === '0s');
    assert.equal(zero.length, 1, kind);
    assert.equal(Number(zero[0].getAttribute('x')), Number(grid.getAttribute('x2')), kind + ' starts with 0s on the right');
    assert.equal(zero[0].getAttribute('text-anchor'), 'end', kind + ' tick stays within the panel');
    if (kind === 'overview') {
      tested.setExpanded(true);
      Object.defineProperty(drawing, 'getBoundingClientRect', { value: () => ({ left: 0, width: 1000 }) });
      drawing.dispatchEvent(new window.PointerEvent('pointermove', { clientX: Number(grid.getAttribute('x2')), bubbles: true }));
      assert.equal(Number(drawing.querySelector('.chart-cursor circle')!.getAttribute('cx')), Number(grid.getAttribute('x2')), 'The first overview cursor uses the same zero-age coordinate');
    } else assert.equal(Number(drawing.querySelector('circle')!.getAttribute('cx')), Number(grid.getAttribute('x2')), 'The first history point uses the same zero-age coordinate');
    window.happyDOM.abort();
});


test('legend uses supplied retained baseline outside visible samples without widening keyboard history', () => {
  const { window, chart, view, drawing } = setup();
  view.settings.change_seconds = 300; view.overview.start = 301; view.overview.end = 601;
  const sample = (at: number, value: string, before: number) => ({ ...view.overview.samples[0], at, layers: ['0', value], baseline: { at: before, layers: ['0', '100'] } });
  view.overview.samples = [sample(303, '125', 2), sample(601, '150', 300)];
  chart.setExpanded(true); chart.update(view);
  const label = () => drawing.querySelectorAll('.legend-change')[1].textContent;
  assert.equal(label(), '+50.0%');
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.equal(label(), '+25.0%');
  chart.element.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.equal(label(), '+25.0%', 'No extra keyboard sample is exposed');
  drawing.dispatchEvent(new window.PointerEvent('pointerleave')); assert.equal(label(), '+50.0%');
  window.happyDOM.abort();
});
