import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { ViewClient } from '../src/transport.ts';
import { fixture } from './fixture.ts';

test('header copies the displayed snapshot instead of opening export', () => {
  const window = new Window();
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const calls: string[] = [], dashboard = new Dashboard(() => {}, action => calls.push(action));
  document.body.append(dashboard.element); dashboard.update(fixture());
  const action = dashboard.element.querySelector<HTMLButtonElement>('[aria-label="Copy snapshot to clipboard"]');
  assert(action, 'The header needs a snapshot clipboard action'); action.click();
  assert.deepEqual(calls, ['snapshot']);
  assert(!dashboard.element.querySelector('[aria-label="Export CSV"]'));
  window.happyDOM.abort();
});

test('snapshot clipboard requests wait for preceding actions and report copy failures', async () => {
  const calls: string[] = [], errors: string[] = []; let release!: () => void;
  const client = new ViewClient({
    getView: async () => ({ revision: 1 }),
    dispatch: async (_action: string) => { await new Promise<void>(resolve => { release = resolve; }); calls.push('paused'); return { revision: 2 }; },
    copyPaths: async () => 'Copied paths',
    copySnapshot: async () => { calls.push('snapshot'); throw Error('Clipboard is busy'); },
  }, () => {}, error => errors.push(error));
  const pause = client.send('pause'), copied = client.native('snapshot');
  await new Promise(resolve => setImmediate(resolve)); assert.deepEqual(calls, []);
  release(); await Promise.all([pause, copied]);
  assert.deepEqual(calls, ['paused', 'snapshot']); assert.match(errors[0], /Clipboard is busy/);
});

test('grouping icons preserve named radios, tooltips and grouping commands', () => {
  const window = new Window();
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const calls: unknown[] = [], dashboard = new Dashboard(action => calls.push(action), () => {});
  document.body.append(dashboard.element); dashboard.update(fixture());
  const group = dashboard.element.querySelector('fieldset')!;
  const shapes: string[] = [];
  for (const [value, name] of [['path', 'Path'], ['tree', 'Process tree'], ['process', 'Individual processes'], ['family', 'Family'], ['name', 'Name']]) {
    const radio = group.querySelector<HTMLInputElement>(`input[value=${value}]`)!;
    assert.equal(radio.getAttribute('aria-label'), name);
    assert.equal(radio.parentElement!.title, name);
    assert.equal(radio.parentElement!.textContent, '', 'Only the icon is visible');
    const icon = radio.parentElement!.querySelector('svg'); assert(icon);
    assert.equal(icon.getAttribute('aria-hidden'), 'true'); shapes.push(icon.querySelector('path')!.getAttribute('d')!);
    radio.click(); assert.deepEqual(calls.at(-1), { type: 'set_grouping', value });
  }
  assert.equal(new Set(shapes).size, 5, 'Each grouping has a distinct icon');
  assert.equal(group.querySelectorAll('input:checked').length, 1);
  window.happyDOM.abort();
});
