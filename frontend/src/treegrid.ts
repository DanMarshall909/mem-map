import type { Action, Row, SortColumn, View } from './contract.ts';
import { button, el, text } from './dom.ts';
import { change, elapsed, ratio, reading, readingHelp } from './format.ts';

const columns: [SortColumn, string][] = [['name', 'Executable / processes'], ['pid', 'PID'], ['private_working_set', 'Private working set'], ['peak', 'Peak (last 30m)'], ['private_commit', 'Private commit'], ['change', 'Change (5m)']];
export class ProcessTreeGrid {
  readonly element = el('div', 'process-grid');
  private header = el('div', 'grid-header');
  private body = el('div', 'grid-body');
  private empty = el('p', 'empty', 'No matching processes');
  private nodes = new Map<string, HTMLElement>();
  private identityIds = new Map<string, string>();
  private nodeSerial = 0;
  private view: View | null = null;
  private rowHeight = 34;
  private send: (action: Action) => void;
  private held = false;
  private pointerHeld = false;
  constructor(send: (action: Action) => void) {
    this.send = send;
    this.element.setAttribute('role', 'treegrid'); this.element.setAttribute('aria-label', 'Processes');
    this.element.setAttribute('aria-colcount', '6'); this.header.setAttribute('role', 'row'); this.header.setAttribute('aria-rowindex', '1');
    this.body.setAttribute('role', 'rowgroup');
    for (const [column, name] of columns) {
      const cell = el('div', 'column'); cell.setAttribute('role', 'columnheader');
      const sort = button(name, () => this.send({ type: 'sort_by', column })); cell.append(sort);
      const resize = el('div', 'column-resize'); resize.setAttribute('role', 'separator'); resize.tabIndex = 0;
      resize.setAttribute('aria-label', `Resize ${name} column`); resize.setAttribute('aria-orientation', 'vertical');
      resize.addEventListener('pointerdown', event => {
        if (!this.view) return; event.preventDefault(); event.stopPropagation();
        const index = columns.findIndex(([key]) => key === column); const start = event.clientX;
        const widths = [...this.view.settings.column_widths]; const initial = widths[index];
        const move = (e: PointerEvent) => { widths[index] = Math.round(Math.max(45, Math.min(1600, initial + (e.clientX - start) / (this.view!.settings.ui_zoom / 100)))); this.applyWidths(widths); };
        const end = () => { document.removeEventListener('pointermove', move); document.removeEventListener('pointerup', end); this.send({ type: 'set_column_widths', value: widths }); };
        document.addEventListener('pointermove', move); document.addEventListener('pointerup', end, { once: true });
      });
      resize.addEventListener('keydown', e => {
        if (!this.view || !['ArrowLeft', 'ArrowRight'].includes(e.key)) return; e.preventDefault();
        const widths = [...this.view.settings.column_widths]; const index = columns.findIndex(([key]) => key === column);
        widths[index] = Math.max(45, Math.min(1600, widths[index] + (e.key === 'ArrowRight' ? 10 : -10)));
        this.send({ type: 'set_column_widths', value: widths });
      });
      cell.append(resize); this.header.append(cell);
    }
    this.element.append(this.header, this.body, this.empty);
    this.element.addEventListener('scroll', () => this.renderRows(), { passive: true });
    this.element.addEventListener('keydown', e => this.key(e));
    this.element.addEventListener('focusin', () => this.hold(true));
    this.element.addEventListener('focusout', () => queueMicrotask(() => { if (!this.element.contains(document.activeElement) && !this.pointerHeld) this.hold(false); }));
    this.element.addEventListener('pointerdown', () => { this.pointerHeld = true; this.hold(true); });
    document.addEventListener('pointerup', () => { this.pointerHeld = false; if (!this.element.contains(document.activeElement)) this.hold(false); });
    window.addEventListener('blur', () => { this.pointerHeld = false; this.hold(false); });
  }
  private hold(value: boolean): void { if (value !== this.held) { this.held = value; this.send({ type: 'set_table_interaction_held', value }); } }
  private applyWidths(widths: number[]): void {
    const base = 14 * (this.view?.settings.ui_zoom ?? 100) / 100;
    // Numeric text owns its space before decorative bars or identity. Headers wrap.
    const minima = [160, 55, 130, 100, 105, 110];
    if (this.view) for (const row of this.view.table.rows) {
      const values = ['', row.pid?.toString() ?? '—', reading(row.private), reading(row.peak), reading(row.commit), change(row.change)];
      values.forEach((value, i) => { if (i) minima[i] = Math.max(minima[i], value.length * 8 + 26); });
    }
    const room = this.element.clientWidth / (base / 14);
    const requested = widths.map((width, i) => Math.max(minima[i], width));
    const sum = requested.reduce((a, b) => a + b, 0);
    if (room > 0 && sum > room) {
      let extra = room - minima.reduce((a, b) => a + b, 0);
      for (let i = 0; i < requested.length; i++) { const grant = Math.max(0, Math.min(extra, requested[i] - minima[i])); requested[i] = minima[i] + grant; extra -= grant; }
    }
    this.element.style.setProperty('--columns', requested.map((n, i) => i === 0 ? `minmax(${n / 14}rem,1fr)` : `${n / 14}rem`).join(' '));
    this.element.style.setProperty('--table-min', `${requested.reduce((a, b) => a + b, 0) / 14}rem`);
  }
  update(view: View, explicitReveal = false): void {
    const old = this.view; const top = this.element.scrollTop;
    const anchorIndex = Math.floor(top / this.rowHeight); const anchor = old?.table.rows[anchorIndex]?.id;
    const offset = top - anchorIndex * this.rowHeight;
    this.view = view; this.rowHeight = (view.settings.compact ? 34 : 50) * (view.settings.ui_zoom / 100);
    const retained = new Set(view.table.rows.map(row => row.id));
    for (const id of this.identityIds.keys()) if (!retained.has(id)) this.identityIds.delete(id);
    this.element.classList.toggle('expanded-density', !view.settings.compact);
    this.element.setAttribute('aria-rowcount', String(view.table.rows.length + 1));
    this.element.dataset.snapshot = String(view.displayed_at);
    this.empty.hidden = view.table.rows.length > 0;
    this.element.tabIndex = view.table.rows.length ? -1 : 0;
    this.applyWidths(view.settings.column_widths);
    columns.forEach(([column, label], index) => {
      const cell = this.header.children[index]; const sorted = view.settings.sort.column === column;
      cell.setAttribute('aria-sort', sorted ? (view.settings.sort.descending ? 'descending' : 'ascending') : 'none');
      text(cell.firstChild!, `${column === 'change' ? `Change (${view.settings.change_seconds / 60}m)` : label}${sorted ? (view.settings.sort.descending ? ' ↓' : ' ↑') : ''}`);
      cell.lastElementChild!.setAttribute('aria-valuenow', String(view.settings.column_widths[index]));
    });
    this.body.style.height = `${view.table.rows.length * this.rowHeight}px`;
    if (old && anchor && (old.settings.compact !== view.settings.compact || old.settings.ui_zoom !== view.settings.ui_zoom || old.table.rows.map(r => r.id).join('|') !== view.table.rows.map(r => r.id).join('|'))) {
      const next = view.table.rows.findIndex(r => r.id === anchor);
      if (next >= 0) this.element.scrollTop = next * this.rowHeight + offset;
    }
    if (explicitReveal && view.reveal_row_id) this.revealRow(view.reveal_row_id);
    this.renderRows();
  }
  private renderRows(): void {
    if (!this.view) return;
    const rows = this.view.table.rows;
    const visibleFirst = Math.max(0, Math.min(rows.length - 1, Math.floor(this.element.scrollTop / this.rowHeight)));
    const visibleCount = Math.ceil((this.element.clientHeight || 700) / this.rowHeight);
    const first = Math.max(0, visibleFirst - 5);
    const last = Math.min(rows.length, first + visibleCount + 12);
    const wanted = new Set(rows.slice(first, last).map(row => row.id));
    const active = document.activeElement;
    const activeId = active && this.body.contains(active) ? active.closest<HTMLElement>('[data-row-id]')?.dataset.rowId : undefined;
    if (activeId) wanted.add(activeId);
    // Keyboard entry follows focus or the visible selection; a hidden/offscreen
    // selected identity must not strand the virtualized rows outside Tab order.
    const selectedIndex = rows.findIndex(row => row.id === this.view!.table.selected_id);
    const entryId = activeId && rows.some(row => row.id === activeId) ? activeId
      : selectedIndex >= visibleFirst && selectedIndex < visibleFirst + visibleCount ? rows[selectedIndex].id
      : rows[visibleFirst]?.id;
    for (const [id, node] of this.nodes) if (!wanted.has(id) || !rows.some(row => row.id === id)) { node.remove(); this.nodes.delete(id); }
    for (let index = 0; index < rows.length; index++) {
      const row = rows[index]; if (!wanted.has(row.id)) continue;
      let node = this.nodes.get(row.id);
      if (!node) {
        node = el('div', 'process-row'); node.dataset.rowId = row.id;
        const id = this.identityIds.get(row.id) ?? `process-row-${++this.nodeSerial}`;
        this.identityIds.set(row.id, id); node.id = id; node.setAttribute('role', 'row');
        for (let col = 0; col < 6; col++) { const cell = el('div', `cell cell-${col}`); cell.setAttribute('role', 'gridcell'); node.append(cell); }
        node.addEventListener('click', () => { this.send({ type: 'select_row', id: row.id }); node!.focus({ preventScroll: true }); });
        node.addEventListener('dblclick', () => { if (this.view?.table.rows.find(r => r.id === row.id)?.has_children) this.send({ type: 'toggle_expanded', id: row.id }); });
        node.addEventListener('focusin', () => this.renderRows());
        this.nodes.set(row.id, node); this.body.append(node);
      }
      this.patchRow(node, row, index, entryId);
    }
  }
  private patchRow(node: HTMLElement, row: Row, index: number, entryId: string | undefined): void {
    if (!this.view) return;
    node.style.top = `${index * this.rowHeight}px`; node.style.height = `${this.rowHeight}px`;
    node.setAttribute('aria-rowindex', String(index + 2)); node.setAttribute('aria-level', String(row.depth + 1));
    node.setAttribute('aria-selected', String(row.selected));
    if (row.has_children) node.setAttribute('aria-expanded', String(row.expanded)); else node.removeAttribute('aria-expanded');
    const siblings = this.view.table.rows.filter(r => r.parent_id === row.parent_id);
    node.setAttribute('aria-posinset', String(siblings.findIndex(r => r.id === row.id) + 1)); node.setAttribute('aria-setsize', String(siblings.length));
    node.tabIndex = row.id === entryId ? 0 : -1;
    node.classList.toggle('selected', row.selected);
    const count = row.pid === null ? ` · ${row.count} ${row.count === 1 ? 'process' : 'processes'}` : '';
    const qualifier = row.matching_count < row.count ? ` · ${row.matching_count} matching of ${row.count}` : '';
    const exited = row.lifecycle === 'exited' ? ` · Exited${row.exited_at === null ? '' : ` ${elapsed(row.exited_at)}`}` : '';
    const help = `${row.name}${count}${qualifier}${exited}; ${row.detail}; PID ${row.pid ?? 'aggregate'}; Private working set ${readingHelp(row.private)}; Private commit ${readingHelp(row.commit)}; Peak ${readingHelp(row.peak)}; Change ${row.change.value ?? 'unavailable'} raw bytes; ${row.change.reason ?? ''}`;
    node.setAttribute('aria-label', help); node.title = help;
    const identity = node.children[0] as HTMLElement;
    if (!identity.firstChild) { const disclosure = button('', () => {}, 'disclosure'); disclosure.tabIndex = -1; disclosure.setAttribute('aria-hidden', 'true'); disclosure.addEventListener('click', e => { e.stopPropagation(); this.send({ type: 'toggle_expanded', id: row.id }); }); identity.append(disclosure, el('div', 'identity')); }
    identity.style.paddingInlineStart = `${0.6 + row.depth * 1.1}rem`;
    const disclosure = identity.firstElementChild as HTMLButtonElement; disclosure.disabled = !row.has_children; text(disclosure, row.has_children ? (row.expanded ? '▾' : '▸') : '');
    const pathIdentity = row.pid === null && (this.view.settings.grouping === 'path' || (this.view.settings.grouping === 'name' && row.depth > 0));
    const label = identity.lastElementChild!; text(label, `${pathIdentity ? row.detail : row.name}${count}${qualifier}${exited}`);
    if (!this.view.settings.compact) { const detail = el('small', '', row.detail); label.append(detail); }
    text(node.children[1], row.pid?.toString() ?? '—');
    [row.private, row.peak, row.commit].forEach((r, i) => {
      const cell = node.children[i + 2] as HTMLElement;
      if (i === 0) { if (!cell.firstChild) cell.append(el('span', 'memory-bar'), el('span', 'numeric')); const bar = cell.firstElementChild as HTMLElement; bar.style.setProperty('--fill', `${ratio(r.value, this.view!.table.private_scale_bytes) * 100}%`); text(cell.lastChild!, reading(r)); }
      else text(cell, reading(r)); cell.title = readingHelp(r);
    });
    const delta = node.children[5] as HTMLElement;
    if (!delta.firstChild) delta.append(el('span', 'change-bar'), el('span', 'numeric'));
    const bar = delta.firstElementChild as HTMLElement;
    const n = row.change.value === null ? 0n : BigInt(row.change.value);
    bar.classList.toggle('decrease', n < 0n); bar.style.setProperty('--fill', `${ratio((n < 0n ? -n : n).toString(), this.view.table.change_scale_bytes) * 50}%`);
    text(delta.lastChild!, change(row.change)); delta.title = `${row.change.value ?? 'Unavailable'} raw bytes${row.change.reason ? `; ${row.change.reason}` : ''}`;
  }
  revealRow(id: string): void {
    const index = this.view?.table.rows.findIndex(row => row.id === id) ?? -1; if (index < 0) return;
    const top = index * this.rowHeight; const viewport = this.element.clientHeight || 700;
    if (top < this.element.scrollTop) this.element.scrollTop = top;
    else if (top + this.rowHeight > this.element.scrollTop + viewport - 50) this.element.scrollTop = top - viewport + this.rowHeight + 50;
    this.renderRows();
  }
  private key(event: KeyboardEvent): void {
    if (!this.view || !(event.target as HTMLElement).closest('[data-row-id]')) return;
    const rows = this.view.table.rows; const id = (event.target as HTMLElement).closest<HTMLElement>('[data-row-id]')!.dataset.rowId;
    const index = rows.findIndex(row => row.id === id); const row = rows[index]; if (!row) return;
    let target: Row | undefined;
    switch (event.key) {
      case 'ArrowDown': target = rows[Math.min(rows.length - 1, index + 1)]; break;
      case 'ArrowUp': target = rows[Math.max(0, index - 1)]; break;
      case 'Home': target = rows[0]; break;
      case 'End': target = rows.at(-1); break;
      case 'ArrowRight': if (row.has_children && !row.expanded) this.send({ type: 'toggle_expanded', id: row.id }); else if (rows[index + 1]?.parent_id === row.id) target = rows[index + 1]; break;
      case 'ArrowLeft': if (row.has_children && row.expanded) this.send({ type: 'toggle_expanded', id: row.id }); else target = rows.find(r => r.id === row.parent_id); break;
      case 'Enter': if (row.has_children) this.send({ type: 'toggle_expanded', id: row.id }); break;
      default: return;
    }
    event.preventDefault();
    if (target) { this.send({ type: 'select_row', id: target.id }); this.revealRow(target.id); this.nodes.get(target.id)?.focus({ preventScroll: true }); }
  }
}
