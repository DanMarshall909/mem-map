import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';
import { Dashboard } from '../src/dashboard.ts';
import { fixture } from './fixture.ts';

function dom(width: number, height: number) {
  const window = new Window({ width, height });
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, Element: window.Element });
  const style = document.createElement('style'); style.textContent = readFileSync(new URL('../src/styles.css', import.meta.url), 'utf8');
  document.head.append(style); return window;
}
function rule(selector: string): CSSStyleDeclaration {
  const rule = [...document.styleSheets[0].cssRules].find(entry => 'selectorText' in entry && entry.selectorText === selector);
  assert.ok(rule, `Layout rule ${selector} exists`); return (rule as CSSStyleRule).style;
}

test('enlarged text reflows all dashboard regions instead of squeezing the table and inspector', () => {
  const window = dom(1280, 800); const calls: unknown[] = [];
  const dashboard = new Dashboard(action => calls.push(action), () => {}); document.body.append(dashboard.element);
  const view = fixture(); dashboard.update(view);
  const workspace = dashboard.element.querySelector<HTMLElement>('.workspace')!;
  assert.equal(window.getComputedStyle(workspace).display, 'grid', 'An ordinary desktop retains the side-by-side workspace');
  const filter = dashboard.element.querySelector<HTMLInputElement>('input[type=search]')!; filter.focus();
  dashboard.table.element.scrollTop = 112; dashboard.inspector.scrollTop = 233;
  view.settings.ui_zoom = 200; view.revision++; dashboard.update(view);
  assert.equal(window.getComputedStyle(workspace).display, 'flex', 'At twice the text size, regions need normal vertical flow');
  assert.equal(window.getComputedStyle(workspace).flexDirection, 'column');
  assert.equal(window.getComputedStyle(dashboard.element).height, 'auto');
  assert.equal(window.getComputedStyle(dashboard.element.querySelector('.process-panel')!).gridTemplateRows, 'auto 16rem', 'Toolbar growth cannot consume the usable process viewport');
  // HappyDOM resolves rem with its own fixed base; actual scaled bounds belong
  // to the desktop layout check. Here assert the applied flow and authored units.
  assert.equal(rule('.reflow .process-grid').minHeight, '16rem');
  assert.ok(parseFloat(window.getComputedStyle(dashboard.table.element).minHeight) > 0);
  assert.equal(rule('.reflow .inspector-scroll').maxHeight, '42rem', 'The normal-flow inspector remains independently scrollable');
  assert.equal(window.getComputedStyle(dashboard.inspector).overflow, 'auto');
  assert.equal(window.getComputedStyle(dashboard.element.querySelector('.splitter')!).display, 'none');
  assert.equal(document.activeElement, filter); assert.equal(dashboard.table.element.scrollTop, 112); assert.equal(dashboard.inspector.scrollTop, 233);
  assert.equal(calls.length, 0, 'Responsive presentation cannot dispatch a domain action or change selection');
  window.happyDOM.abort();
});

test('resizing between minimum and roomy windows adapts flow using the current text scale', () => {
  const window = dom(1280, 800); const calls: unknown[] = [];
  const dashboard = new Dashboard(action => calls.push(action), () => {}); document.body.append(dashboard.element);
  const view = fixture(); dashboard.update(view);
  const workspace = dashboard.element.querySelector<HTMLElement>('.workspace')!;
  const filter = dashboard.element.querySelector<HTMLInputElement>('input[type=search]')!; filter.focus();
  const resize = (width: number, height: number) => { window.happyDOM.setViewport({ width, height }); window.dispatchEvent(new window.Event('resize')); };
  resize(760, 520);
  assert.equal(window.getComputedStyle(workspace).display, 'flex', 'The minimum native client uses a vertical workspace without waiting for collection');
  resize(740, 1000); assert.equal(window.getComputedStyle(workspace).display, 'flex', 'A narrow client also needs reflow');
  resize(1280, 800); assert.equal(window.getComputedStyle(workspace).display, 'grid');
  view.settings.ui_zoom = 200; dashboard.update(view); assert.equal(window.getComputedStyle(workspace).display, 'flex');
  resize(1800, 1300); assert.equal(window.getComputedStyle(workspace).display, 'grid', 'A genuinely roomy scaled viewport returns to side-by-side panels');
  assert.equal(window.getComputedStyle(dashboard.element).gridTemplateRows, 'auto minmax(0, 1fr) auto');
  assert.equal(rule('.inspector-scroll').minHeight, '11rem', 'Normal layout reserves a useful inspector viewport including padding');
  resize(1800, 1100); assert.equal(window.getComputedStyle(workspace).display, 'flex', 'Short scaled clients must not squeeze the inspector');
  view.settings.ui_zoom = 100; dashboard.update(view); assert.equal(window.getComputedStyle(workspace).display, 'grid');
  assert.equal(document.activeElement, filter); assert.equal(calls.length, 0);
  window.happyDOM.abort();
});

test('the overview honors its explicit plot height while selected history retains its minimum', () => {
  const window = dom(1280, 800); const dashboard = new Dashboard(() => {}, () => {}); document.body.append(dashboard.element);
  Object.defineProperty(dashboard.element.querySelector('.overview-panel'), 'clientHeight', { value: 100 });
  dashboard.update(fixture());
  const overview = dashboard.element.querySelector<SVGSVGElement>('.overview-panel .memory-chart > svg')!;
  const history = dashboard.inspector.querySelector<SVGSVGElement>('.memory-chart > svg')!;
  assert.equal(overview.style.height, '112px', 'The chart chooses its supported overview floor');
  assert.equal(parseFloat(window.getComputedStyle(overview).minHeight), 0, 'CSS cannot enlarge the viewport beyond the explicit SVG viewBox height');
  assert.ok(parseFloat(window.getComputedStyle(history).minHeight) > 0, 'Selected history retains a practical minimum');
  window.happyDOM.abort();
});
