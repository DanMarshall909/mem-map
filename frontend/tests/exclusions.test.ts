import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { fixture } from './fixture.ts';

test('global exclusions can be managed from expanded overview with keyboard form submission and reversible categories', t => {
  const window = new Window(); t.after(() => window.happyDOM.abort());
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const calls: unknown[] = [], dashboard = new Dashboard(action => calls.push(action), () => {});
  document.body.append(dashboard.element); const view = fixture(); dashboard.update(view);
  dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Expand overview"]')!.click();
  const open = dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Global exclusions"]');
  assert(open, 'Global exclusions must remain available in the header'); open.click();
  const dialog = dashboard.element.querySelector<HTMLDialogElement>('[aria-label="Global exclusions"]dialog')!;
  assert(dialog.open);
  const input = dialog.querySelector<HTMLInputElement>('input[type="search"]')!;
  const path = dialog.querySelector<HTMLInputElement>('input[aria-label="Executable path"]')!;
  path.checked = true; path.dispatchEvent(new window.Event('change', { bubbles: true }));
  input.value = 'C:\\TOOLS'; dialog.querySelector('form')!.dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
  assert.deepEqual(calls.pop(), { type: 'add_exclusion', kind: 'path', value: 'C:\\TOOLS' });
  const toggle = dialog.querySelector<HTMLInputElement>('[aria-label="Show excluded memory as a band"]')!;
  toggle.checked = false; toggle.dispatchEvent(new window.Event('change'));
  assert.deepEqual(calls.pop(), { type: 'set_show_excluded', value: false });
  view.settings.exclusions.rules = [{kind:'path', value:'c:\\tools'}];
  input.value = 'typing'; input.focus(); dashboard.update(view);
  assert.equal(input.value, 'typing'); assert.equal(document.activeElement, input);
  [...dialog.querySelectorAll('button')].find(b => b.getAttribute('aria-label') === 'Remove path exclusion: c:\\tools')!.click();
  assert.deepEqual(calls.pop(), { type: 'remove_exclusion', kind: 'path', value: 'c:\\tools' });
  for (const [label, type] of [['Exclude Other used RAM', 'set_exclude_other_ram'], ['Exclude Other processes', 'set_exclude_other_processes']] as const) {
    const control = dialog.querySelector<HTMLInputElement>(`[aria-label="${label}"]`)!;
    control.checked = true; control.dispatchEvent(new window.Event('change'));
    assert.deepEqual(calls.pop(), { type, value: true });
  }
  [...dialog.querySelectorAll('button')].find(b => b.textContent === 'Clear all exclusions')!.click();
  assert.deepEqual(calls.pop(), { type:'clear_exclusions' });
  const close = dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Close exclusions"]')!;
  close.focus();
  const escape = new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }); close.dispatchEvent(escape);
  assert(!escape.defaultPrevented, 'Native modal dismissal must not be intercepted by dashboard shortcuts');
  assert(dashboard.element.classList.contains('overview-expanded'));
  dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Close exclusions"]')!.click();
  assert(!dialog.open); assert.equal(document.activeElement, open);
});
