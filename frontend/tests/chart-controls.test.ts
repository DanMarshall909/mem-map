import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { MemoryChart } from '../src/charts.ts';
import { fixture } from './fixture.ts';
function setup() {
  const window = new Window({ width: 1440, height: 1000 });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const view = fixture(); const calls: unknown[] = [];
  const dashboard = new Dashboard(a => calls.push(a), () => {}); document.body.append(dashboard.element);
  const panel = dashboard.element.querySelector<HTMLElement>('.overview-panel')!;
  Object.defineProperty(panel, 'clientHeight', { value: 600 });
  dashboard.update(view);
  return { window, view, calls, dashboard, panel };
}

test('change interval toggles send their duration and follow toggle stays synchronized with navigation', () => {
  const { window, view, dashboard, calls } = setup();
  const interval = [...dashboard.element.querySelectorAll('fieldset')].find(el => el.querySelector('legend')?.textContent === 'Change over');
  assert(interval, 'Change over exposes the available durations together');
  for (const seconds of [60, 300, 900, 1800]) {
    const choice = interval.querySelector<HTMLInputElement>(`input[value="${seconds}"]`)!;
    choice.click(); assert.deepEqual(calls.at(-1), { type: 'set_change_seconds', value: seconds });
    view.settings.change_seconds = seconds; dashboard.update(view);
    assert.equal(interval.querySelector<HTMLInputElement>('input:checked')!.value, String(seconds));
    assert.equal(interval.querySelectorAll('input:checked').length, 1);
  }
  const follow = dashboard.inspector.querySelector<HTMLInputElement>('input[aria-label="Follow latest"]')!;
  assert(follow); assert.equal(follow.checked, true);
  follow.click(); assert.deepEqual(calls.at(-1), { type: 'set_follow', value: false });
  view.history_viewport.follow = false; dashboard.update(view); assert.equal(follow.checked, false);
  follow.click(); assert.deepEqual(calls.at(-1), { type: 'set_follow', value: true });
  view.history_viewport.follow = true; dashboard.update(view); assert.equal(follow.checked, true);
  window.happyDOM.abort();
});
test('commit overview uses commit scale and shares while the header remains physical', () => {
  const { window, view, dashboard, panel } = setup();
  view.settings.overview_metric = 'private_commit'; view.overview.capacity = '0';
  view.overview.layers = [{ name: 'Other processes', id: null }, { name: 'worker.exe', id: 'name:worker' }];
  view.overview.samples = view.overview.samples.map((sample, i) => ({ ...sample, capacity: '0', available: null,
    used: String(BigInt(40 + i * 5) * 1073741824n), layers: ['10737418240', String(BigInt(30 + i * 5) * 1073741824n)], reason: 'Measured process private commit; not system commit or physical RAM' }));
  dashboard.update(view);
  panel.querySelector<HTMLButtonElement>('[aria-label="Expand overview"]')!.click();
  assert.match(panel.querySelector('h2')!.textContent!, /Measured private commit/);
  assert.match(dashboard.element.querySelector('.system-summary')!.textContent!, /24.00 GiB \/ 32.00 GiB/);
  const data = panel.querySelector('output')!.textContent!;
  assert.match(data, /Measured private commit 50.00 GiB/);
  assert.doesNotMatch(data, /of physical RAM|Available|Capacity/);
  const labels = [...panel.querySelectorAll('.legend-share')].map(el => el.textContent);
  assert.deepEqual(labels, ['20.0%', '80.0%']);
  const ticks = [...panel.querySelectorAll('.chart-annotations text')].map(el => el.textContent ?? '').filter(s => /^\d+\.\d$/.test(s)).map(Number);
  assert(Math.max(...ticks) > 50, 'Commit may exceed physical capacity');
  window.happyDOM.abort();
});

for (const metric of ['private_working_set', 'private_commit'] as const) test(`overview ${metric} fit spans zero to the visible peak and clips bands and cursor`, () => {
  const { window, view, dashboard, panel } = setup();
  view.settings.overview_metric = metric;
  view.settings.overview_from_zero = false;
  view.overview.samples[0].used = '21474836480';
  view.overview.samples[0].layers = ['20401094656', '1073741824'];
  dashboard.update(view); panel.querySelector<HTMLButtonElement>('[aria-label="Expand overview"]')!.click();
  const drawing = panel.querySelector('.memory-chart > svg')!;
  const ticks = () => [...drawing.querySelectorAll('.chart-annotations text')].map(el => el.textContent ?? '').filter(s => /^\d+\.\d+$/.test(s)).map(Number);
  assert(ticks().length > 1);
  assert.equal(Math.min(...ticks()), 0, 'The stacked overview always starts at zero');
  assert.equal(Math.max(...ticks()), 24, 'Fit values ends at the highest visible total, not RAM capacity');
  const clip = drawing.querySelector('clipPath.chart-plot-clip rect'); assert(clip, 'Fitted areas must be clipped');
  const areas = drawing.querySelector('.chart-areas'); assert(areas?.getAttribute('clip-path'));
  panel.querySelector('.memory-chart')!.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  const cursor = drawing.querySelector('.chart-cursor circle')!;
  const y = Number(cursor.getAttribute('cy')), top = Number(clip.getAttribute('y')), bottom = top + Number(clip.getAttribute('height'));
  assert(y >= top && y <= bottom, 'Cursor uses the fitted scale too');
  assert.equal(drawing.querySelector('.chart-cursor')!.getAttribute('clip-path'), areas!.getAttribute('clip-path'));
  view.settings.overview_from_zero = true; dashboard.update(view); assert.equal(Math.min(...ticks()), 0);
  window.happyDOM.abort();
});

test('both charts have independent named metric and axis icon toggles', () => {
  const { window, dashboard, calls, view } = setup();
  const group = (name: string) => [...dashboard.element.querySelectorAll('fieldset')].find(el => el.querySelector('legend')?.textContent === name);
  for (const name of ['Overall memory metric', 'Overall memory Y-axis', 'Selected history metric', 'Selected history Y-axis']) {
    const fieldset = group(name); assert(fieldset, `${name} must be available`);
    assert.equal(fieldset.querySelectorAll('input:checked').length, 1);
    const paths = [...fieldset.querySelectorAll('svg path')].map(path => path.getAttribute('d')); assert.equal(new Set(paths).size, 2);
    for (const label of fieldset.querySelectorAll('label')) {
      assert(label.title); assert(label.querySelector('input')!.getAttribute('aria-label'));
      assert.equal(label.querySelector('svg')!.getAttribute('aria-hidden'), 'true');
    }
  }
  group('Overall memory metric')!.querySelector<HTMLInputElement>('input[value=private_commit]')!.click();
  group('Overall memory Y-axis')!.querySelector<HTMLInputElement>('input[value=fit]')!.click();
  group('Selected history metric')!.querySelector<HTMLInputElement>('input[value=private_commit]')!.click();
  group('Selected history Y-axis')!.querySelector<HTMLInputElement>('input[value=fit]')!.click();
  assert.deepEqual(calls, [{ type: 'set_overview_metric', value: 'private_commit' }, { type: 'set_overview_from_zero', value: false }, { type: 'set_metric', value: 'private_commit' }, { type: 'set_from_zero', value: false }]);
  view.settings.overview_metric = 'private_commit'; view.settings.overview_from_zero = false; dashboard.update(view);
  assert.equal(group('Overall memory metric')!.querySelector<HTMLInputElement>('input:checked')!.value, 'private_commit');
  assert.equal(group('Selected history metric')!.querySelector<HTMLInputElement>('input:checked')!.value, 'private_working_set');
  assert.equal(group('Overall memory Y-axis')!.querySelector<HTMLInputElement>('input:checked')!.value, 'fit');
  assert.equal(group('Selected history Y-axis')!.querySelector<HTMLInputElement>('input:checked')!.value, 'zero');
  window.happyDOM.abort();
});
