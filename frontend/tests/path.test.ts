import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { fixture } from './fixture.ts';

test('the path preview protects the complete executable tail independently of a shortened directory', () => {
  const window = new Window({ width: 980, height: 740 });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const style = document.createElement('style'); style.textContent = readFileSync(new URL('../src/styles.css', import.meta.url), 'utf8'); document.head.append(style);
  const native: string[] = []; const dashboard = new Dashboard(() => {}, action => native.push(action)); document.body.append(dashboard.element);
  const view = fixture(); const filename = 'TimeMon.CaptureService.Windows.exe';
  const original = `C:\\Program Files\\${'Original directory\\'.repeat(9)}${filename}`;
  view.selected!.paths = [original]; view.selected!.path_count = 1;
  for (const zoom of [100, 200]) {
    view.settings.ui_zoom = zoom; dashboard.update(view);
    const preview = dashboard.inspector.querySelector<HTMLElement>('.path-preview')!; preview.style.width = '180px';
    const tail = preview.querySelector<HTMLElement>('.path-filename');
    assert.ok(tail, 'The executable tail must have a protected layout region, outside directory ellipsis');
    assert.equal(tail.textContent, `\\${filename}`);
    assert.equal(preview.querySelector('.path-prefix')!.textContent, original.slice(0, -filename.length - 1));
    assert.equal(window.getComputedStyle(tail).overflowWrap, 'anywhere', 'A long executable name wraps instead of being cut off');
    assert.equal(window.getComputedStyle(tail).flexShrink, '0', 'Directory text yields before a filename that already fits needs to wrap');
    assert.notEqual(window.getComputedStyle(tail).textOverflow, 'ellipsis');
    assert(!dashboard.inspector.querySelector('input[readonly], textarea, [contenteditable=true]'), 'Path is static text, not a textbox');
    assert.equal(preview.getAttribute('aria-description'), original);
    assert.equal(preview.textContent, original);
    assert.equal(window.getComputedStyle(preview).userSelect, 'text');
    preview.focus(); assert.equal(document.activeElement === preview, true);

  }
  [...dashboard.inspector.querySelectorAll('button')].find(button => button.getAttribute('aria-label') === 'Copy path')!.click();
  assert.deepEqual(native, ['copy']); window.happyDOM.abort();
});
