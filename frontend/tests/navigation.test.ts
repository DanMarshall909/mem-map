import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import type { Action } from '../src/contract.ts';
import { ProcessTreeGrid } from '../src/treegrid.ts';
import { fixture } from './fixture.ts';

function dom() {
  const window = new Window();
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  return window;
}

test('a hidden selection leaves one rendered row in the Tab order without selecting it on entry', () => {
  const window = dom(); const calls: Action[] = [];
  const grid = new ProcessTreeGrid(action => calls.push(action)); document.body.append(grid.element);
  const view = fixture(); const leaf = view.table.rows[1]; const selectedId = view.table.selected_id;
  view.settings.grouping = 'process'; view.table.selection_hidden = true;
  view.table.rows = [1, 2].map(pid => ({ ...leaf, id: `visible-${pid}`, pid, parent_id: null, depth: 0, selected: false }));
  grid.update(view);
  const entries = [...grid.element.querySelectorAll<HTMLElement>('[data-row-id]')].filter(row => row.tabIndex === 0);
  assert.equal(entries.length, 1, 'A row must remain reachable by Tab when the selected entity is hidden');
  const entry = entries[0]; assert.equal(entry.dataset.rowId, 'visible-1');
  assert.equal(entry.getAttribute('aria-selected'), 'false'); assert.equal(grid.element.scrollTop, 0);
  assert.equal(view.table.selected_id, selectedId); assert.equal(calls.length, 0, 'Rendering an entry cannot change selection');
  entry.focus(); assert.equal(document.activeElement, entry, 'The Tab entry accepts keyboard focus');
  assert.deepEqual(calls, [{ type: 'set_table_interaction_held', value: true }], 'Focus is independent of the retained selection');
  entry.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
  assert.deepEqual(calls.at(-1), { type: 'select_row', id: 'visible-2' });
  assert.equal((document.activeElement as HTMLElement).dataset.rowId, 'visible-2');
  window.happyDOM.abort();
});

test('an offscreen selected row leaves a visible Tab entry while live updates preserve external focus and scroll', () => {
  const window = dom(); const calls: Action[] = [];
  const grid = new ProcessTreeGrid(action => calls.push(action));
  const elsewhere = document.createElement('button'); elsewhere.textContent = 'History metric';
  document.body.append(grid.element, elsewhere);
  Object.defineProperty(grid.element, 'clientHeight', { value: 170 });
  const view = fixture(); const leaf = view.table.rows[1];
  view.table.rows = Array.from({ length: 100 }, (_, index) => ({ ...leaf, id: `process-${index}`, pid: index + 1, parent_id: null, depth: 0, selected: index === 0 }));
  view.table.selected_id = 'process-0'; grid.update(view); elsewhere.focus();
  grid.element.scrollTop = 34 * 40;
  grid.element.dispatchEvent(new window.Event('scroll'));
  view.revision++; view.displayed_at += 2; grid.update(view);
  assert.equal(grid.element.querySelector('[data-row-id="process-0"]'), null, 'The selected row is genuinely virtualized away');
  const entries = [...grid.element.querySelectorAll<HTMLElement>('[data-row-id]')].filter(row => row.tabIndex === 0);
  assert.equal(entries.length, 1, 'A selected identity outside the rendered window cannot remove keyboard entry');
  const entry = entries[0]; assert.equal(entry.dataset.rowId, 'process-40', 'Entry belongs to the viewport, not hidden overscan');
  assert.equal(document.activeElement, elsewhere); assert.equal(grid.element.scrollTop, 34 * 40);
  assert.equal(view.table.selected_id, 'process-0'); assert.equal(calls.length, 0);
  entry.focus(); assert.equal(document.activeElement, entry);
  entry.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
  assert.deepEqual(calls.at(-1), { type: 'select_row', id: 'process-41' });
  const focused = document.activeElement as HTMLElement;
  assert.equal(focused.dataset.rowId, 'process-41');
  view.revision++; grid.update(view);
  assert.equal(document.activeElement, focused, 'A sampled frame retains the independently focused keyboard row');
  assert.equal(focused.tabIndex, 0); assert.equal(grid.element.scrollTop, 34 * 40);
  assert.equal(grid.element.querySelectorAll('[data-row-id][tabindex="0"]').length, 1);
  window.happyDOM.abort();
});
