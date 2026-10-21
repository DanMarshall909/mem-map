import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { fixture } from './fixture.ts';

test('collection details filter explained Windows limitations without losing coverage or raw reasons', t => {
  const window = new Window();
  t.after(() => window.happyDOM.abort());
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element);
  const expected = [
    '0s: csrss.exe (PID 12): OpenProcess: Access is denied. (os error 5)',
    '2s: csrss.exe (PID 12): OpenProcess: Zugriff verweigert. (os error 5)',
    '4s: dllhost.exe (PID 20): OpenProcess: The parameter is incorrect. (os error 87)',
    ...['System', 'Secure System', 'Registry', 'Memory Compression'].map((name, i) => `0s: ${name} (PID ${100 + i}): Executable path: A device attached to the system is not functioning. (os error 31)`),
  ];
  const unexpected = [
    '4s: ordinary.exe (PID 30): Executable path: Failure (os error 31)',
    '4s: worker.exe (PID 31): Private working set counters: Failure (os error 5)',
    '4s: worker.exe (PID 32): OpenProcess: Failure (os error 6)',
    '4s: worker.exe (PID 0): OpenProcess: Failure (os error 87)',
    '4s: Process enumeration: Failure (os error 5)',
    '4s: Physical memory collection failed',
    'A new error format that is not classified',
  ];
  const view = fixture(); view.diagnostics.current_measured = 578; view.diagnostics.current_observed = 620; view.diagnostics.current_unavailable = 42;
  view.diagnostics.failures = [...expected, ...unexpected]; const original = structuredClone(view);
  dashboard.update(view); dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Details"]')!.click();
  const dialog = dashboard.element.querySelector('dialog')!;
  for (const reason of expected) assert(!dialog.textContent!.includes(reason), reason);
  for (const reason of unexpected) assert(dialog.textContent!.includes(reason), reason);
  assert.match(dialog.textContent!, /578 of 620 current processes measured/);
  assert.match(dialog.textContent!, /42 current unavailable readings/);
  assert.match(dialog.textContent!, /7 explained Windows limitations hidden/);
  const toggle = dialog.querySelector<HTMLInputElement>('[aria-label="Show explained Windows limitations"]')!;
  assert(toggle); toggle.click(); toggle.focus();
  for (const reason of view.diagnostics.failures) assert(dialog.textContent!.includes(reason));
  assert.match(dialog.textContent!, /7 explained Windows limitations shown/);
  dashboard.update(structuredClone(view)); assert.equal(document.activeElement, toggle); assert(toggle.checked);
  toggle.click(); for (const reason of expected) assert(!dialog.textContent!.includes(reason));
  assert.deepEqual(view, original, 'Display filtering must not mutate measurement or raw history');
  view.diagnostics.failures = expected; dashboard.update(view);
  assert.match(dialog.textContent!, /No unexpected collection failures/);
  view.diagnostics.failures = []; dashboard.update(view);
  assert.match(dialog.textContent!, /No recorded collection failures/);
  window.happyDOM.abort();
});
