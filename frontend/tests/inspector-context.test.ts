import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { fixture } from './fixture.ts';
import type { Inspection } from '../src/contract.ts';

function setup() {
  const window = new Window({ width: 1440, height: 1000 });
  Object.assign(globalThis, { window, document: window.document, HTMLElement: window.HTMLElement, Element: window.Element });
  const view = fixture(), dashboard = new Dashboard(() => {}, () => {});
  document.body.append(dashboard.element);
  const sample = (at: number, ws: number, commit: number, peak: number, delta: number): Inspection => ({
    at, private: { ...view.selected!.private, value: String(ws * 1048576) },
    commit: { ...view.selected!.commit, value: String(commit * 1048576) },
    peak: { ...view.selected!.peak, value: String(peak * 1048576) },
    change: { value: String(delta * 1048576), reason: 'Retained baseline' }, recorded: at,
  });
  view.settings.metric = 'private_commit';
  view.history!.latest_inspection = sample(600, 50, 120, 150, 20);
  view.history!.points = [sample(0, 10, 40, 45, 10), sample(300, 20, 60, 80, -20)].map(inspection => ({ at: inspection.at, captured_unix_ms: null, reading: inspection.commit, inspection }));
  // Panned view deliberately excludes the latest sample.
  view.history!.end = 300; view.history_viewport.follow = false;
  dashboard.update(view);
  const chart = dashboard.inspector.querySelector<HTMLElement>('.memory-chart')!;
  const values = () => [...dashboard.inspector.querySelectorAll('.summary-values dd')].map(el => el.textContent);
  return { window, view, dashboard, chart, values, sample };
}

test('inspector follows chart sample and metric, then restores latest outside a panned chart', () => {
  const { window, dashboard, chart, values } = setup();
  assert.deepEqual(values(), ['50.0 MiB', '120.0 MiB', '+20.0%', '150.0 MiB']);
  assert.match(dashboard.inspector.querySelector('.summary-values dd:last-child')!.getAttribute('aria-label')!, /Peak private commit/);
  chart.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  assert.deepEqual(values(), ['10.0 MiB', '40.0 MiB', '+33.3%', '45.0 MiB']);
  assert.match(dashboard.inspector.querySelector('.coverage')!.textContent!, /10m 00s ago/);
  chart.dispatchEvent(new window.PointerEvent('pointerleave'));
  assert.deepEqual(values(), ['50.0 MiB', '120.0 MiB', '+20.0%', '150.0 MiB']);
  assert.match(dashboard.inspector.querySelector('.coverage')!.textContent!, /Latest/);
  window.happyDOM.abort();
});

test('context refreshes for live data, metric changes, selection and unavailable samples', () => {
  const { window, view, dashboard, chart, values, sample } = setup();
  chart.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  view.settings.metric = 'private_working_set';
  view.history!.points[0].inspection = sample(0, 12, 40, 15, 2);
  dashboard.update(view);
  assert.deepEqual(values(), ['12.0 MiB', '40.0 MiB', '+20.0%', '15.0 MiB']);
  const missing = sample(0, 0, 0, 15, 2); missing.private.value = null; missing.private.reason = 'Access denied'; missing.change.value = null;
  view.history!.points[0].inspection = missing; dashboard.update(view);
  assert.equal(values()[0], '—'); assert.equal(values()[2], '—');
  assert.match(dashboard.inspector.querySelector('.coverage')!.textContent!, /Access denied/);
  view.selected!.id = 'another'; view.selected!.name = 'another.exe';
  view.history!.latest_inspection = sample(600, 75, 175, 100, 25); dashboard.update(view);
  assert.deepEqual(values(), ['75.0 MiB', '175.0 MiB', '+50.0%', '100.0 MiB']);
  assert.match(dashboard.inspector.querySelector('.coverage')!.textContent!, /Latest/);
  window.happyDOM.abort();
});

test('pointer inspection and keyboard dismissal update summary without sending native actions', () => {
  const { window, chart, values } = setup();
  const plot = chart.querySelector('svg')!;
  Object.assign(plot, { getBoundingClientRect: () => ({ left: 100, width: 900 }) });
  plot.dispatchEvent(new window.PointerEvent('pointermove', { clientX: 1000, bubbles: true }));
  assert.deepEqual(values(), ['20.0 MiB', '60.0 MiB', '−25.0%', '80.0 MiB']);
  chart.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert.deepEqual(values(), ['50.0 MiB', '120.0 MiB', '+20.0%', '150.0 MiB']);
  window.happyDOM.abort();
});
