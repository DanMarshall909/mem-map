import type { Action, View } from './contract.ts';
import { button, el, text } from './dom.ts';
import { bytes, dateTime, duration, elapsed, plotValue, rangeLabel, readingHelp } from './format.ts';

const NS = 'http://www.w3.org/2000/svg';
const colors = ['#7b858b', '#347758', '#6cbc8b', '#f2a14a', '#d48c36'];
function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, content = ''): SVGElementTagNameMap[K] {
  const node = document.createElementNS(NS, tag);
  Object.entries(attrs).forEach(([key, value]) => node.setAttribute(key, String(value))); if (content) node.textContent = content; return node;
}
function pathSegments(points: { x: number; low: number; high: number }[][]): string {
  return points.filter(segment => segment.length).map(segment => `M${segment.map(p => `${p.x},${p.high}`).join(' L')} L${[...segment].reverse().map(p => `${p.x},${p.low}`).join(' L')} Z`).join(' ');
}
function segments<T>(points: T[], value: (point: T) => { x: number; low: number; high: number } | null): { x: number; low: number; high: number }[][] {
  const out: { x: number; low: number; high: number }[][] = []; let current: { x: number; low: number; high: number }[] = [];
  for (const point of points) { const p = value(point); if (p) current.push(p); else if (current.length) { out.push(current); current = []; } }
  if (current.length) out.push(current); return out;
}
export class MemoryChart {
  readonly element = el('div', 'memory-chart');
  private drawing = svg('svg', { role: 'img', 'aria-hidden': 'true', preserveAspectRatio: 'xMidYMid meet' });
  private description = el('p', 'chart-description');
  private data = el('output', 'chart-data');
  private navigator = el('div', 'chart-navigator');
  private readout = el('div', 'chart-readout');
  private legend = el('div', 'legend');
  private inspection = el('details', 'chart-inspection');
  private labels = el('div', 'chart-labels');
  private labelToggle: HTMLButtonElement;
  private compact = false;
  private expanded = false;
  private pointerInside = false;
  private pinned = false;
  private dismissed = false;
  private entries: HTMLButtonElement[] = [];
  private view: View | null = null;
  private kind: 'overview' | 'history';
  private send: (action: Action) => void;
  private hover: number | null = null;
  private band = 0;
  private drag: { x: number; end: number; width: number } | null = null;
  private plot = { width: 1, left: 0, right: 1 };
  constructor(kind: 'overview' | 'history', send: (action: Action) => void) {
    this.kind = kind; this.send = send;
    this.element.setAttribute('role', 'group');
    this.element.setAttribute('aria-label', `${kind === 'overview' ? 'Overall memory' : 'Selected history'} chart; focus or hover for labels`);
    this.labels.id = `${kind}-chart-labels`;
    this.labelToggle = button('ⓘ', () => { this.pinned = !this.pinned; this.dismissed = !this.pinned; this.updateLabels(); }, 'chart-label-toggle');
    this.labelToggle.setAttribute('aria-controls', this.labels.id);
    this.element.addEventListener('pointerenter', e => { if (e.pointerType !== 'touch') { this.pointerInside = true; this.dismissed = false; this.updateLabels(); } });
    this.element.addEventListener('pointerleave', () => { this.pointerInside = false; this.updateLabels(); });
    this.element.addEventListener('focusin', () => { this.dismissed = false; this.updateLabels(); });
    this.element.addEventListener('focusout', () => queueMicrotask(() => {
      if (!this.element.contains(document.activeElement)) this.dismissLabels(); else this.updateLabels();
    }));
    document.addEventListener('pointerdown', e => {
      if (this.compact && !this.element.contains(e.target as Node)) this.dismissLabels();
    });
    window.addEventListener('scroll', () => { if (this.compact && !this.labels.hidden) this.positionLabels(); }, { capture: true, passive: true });
    this.element.addEventListener('keydown', e => {
      if (e.key === 'Escape' && this.compact && !this.labels.hidden) {
        e.preventDefault(); e.stopPropagation(); this.element.focus({ preventScroll: true });
        this.dismissLabels();
      }
    });
    this.data.tabIndex = 0; this.data.setAttribute('aria-label', `${kind === 'overview' ? 'Overall memory' : 'Selected process history'} chart data; use Left and Right Arrow to inspect samples`);
    this.data.setAttribute('role', 'group');
    this.description.id = `${kind}-chart-description`; this.element.setAttribute('aria-describedby', this.description.id);
    this.inspection.append(el('summary', '', 'Chart data'), this.navigator, this.data);
    this.inspection.addEventListener('toggle', () => this.draw());
    this.labels.append(this.readout, this.legend, this.description);
    if (kind === 'overview') this.labels.append(this.inspection);
    else this.labels.append(this.navigator, this.data);
    this.element.append(this.drawing, this.labelToggle, this.labels);
    this.navigator.append(button('Previous sample', () => this.step(-1), 'quiet'), button('Next sample', () => this.step(1), 'quiet'));
    this.data.addEventListener('keydown', e => { if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') { e.preventDefault(); this.step(e.key === 'ArrowLeft' ? -1 : 1); } });
    this.drawing.addEventListener('pointermove', e => {
      if (!this.view) return;
      const source = this.kind === 'overview' ? this.view.overview : this.view.history; if (!source) return;
      const box = this.drawing.getBoundingClientRect();
      const pixels = Math.max(1, (this.plot.right - this.plot.left) / this.plot.width * box.width);
      const left = box.left + this.plot.left / this.plot.width * box.width;
      if (this.drag) { const delta = (e.clientX - this.drag.x) / pixels * this.drag.width; this.send({ type: 'set_history_viewport', end: Math.round(Math.max(0, Math.min(this.view.displayed_at, this.drag.end - delta))), width: this.drag.width }); }
      else { this.hover = source.start + Math.max(0, Math.min(1, (e.clientX - left) / pixels)) * (source.end - source.start); this.updateReadout(); }
    });
    this.drawing.addEventListener('pointerleave', () => { if (!this.drag && document.activeElement !== this.data && !this.legend.contains(document.activeElement)) { this.hover = null; this.updateReadout(); } });
    if (kind === 'history') {
      this.drawing.addEventListener('wheel', e => { if (e.ctrlKey || !this.view?.history) return; e.preventDefault(); this.zoom(e.deltaY < 0); }, { passive: false });
      this.drawing.addEventListener('pointerdown', e => {
        if (!this.view?.history || e.button !== 0) return; e.preventDefault();
        this.drag = { x: e.clientX, end: this.view.history.end, width: Math.max(30, this.view.history.end - this.view.history.start) };
        this.drawing.setPointerCapture(e.pointerId); this.data.focus({ preventScroll: true });
      });
      this.drawing.addEventListener('pointerup', e => { this.drag = null; this.drawing.releasePointerCapture(e.pointerId); });
      this.drawing.addEventListener('pointercancel', () => { this.drag = null; });
    }
    if (typeof ResizeObserver !== 'undefined') new ResizeObserver(() => this.draw()).observe(this.element);
  }
  update(view: View): void { this.view = view; this.draw(); this.updateReadout(); }
  setExpanded(value: boolean): void { this.expanded = value; this.draw(); }
  private dismissLabels(): void { this.pinned = false; this.dismissed = true; this.updateLabels(); }
  private updateLabels(): void {
    const visible = !this.compact || !this.dismissed && (this.pinned || this.pointerInside || this.element.contains(document.activeElement));
    const popover = typeof this.labels.showPopover === 'function' && this.labels.isConnected;
    if (popover && this.labels.matches(':popover-open') && (!visible || !this.compact)) this.labels.hidePopover();
    if (this.compact) this.labels.setAttribute('popover', 'manual');
    else { this.labels.removeAttribute('popover'); this.labels.style.left = ''; this.labels.style.top = ''; }
    this.labels.hidden = !visible; this.labelToggle.hidden = !this.compact;
    this.element.tabIndex = this.compact ? 0 : -1;
    this.element.classList.toggle('labels-visible', this.compact && visible);
    this.labelToggle.setAttribute('aria-expanded', String(visible));
    this.labelToggle.setAttribute('aria-label', `${visible ? 'Hide' : 'Show'} chart labels`);
    if (this.compact && visible) {
      if (popover && !this.labels.matches(':popover-open')) this.labels.showPopover();
      this.positionLabels();
    }
  }
  private positionLabels(): void {
    const anchor = this.drawing.getBoundingClientRect(), box = this.labels.getBoundingClientRect();
    const clamp = (value: number, maximum: number) => Math.max(8, Math.min(value, Math.max(8, maximum - 8)));
    let left = anchor.left, top = anchor.top;
    // Place beside the plot where possible. The top layer avoids clipping by
    // scrolling panels, and leaves pointer sampling/history dragging reachable.
    if (anchor.left >= box.width + 8) left = anchor.left - box.width;
    else if (window.innerWidth - anchor.right >= box.width + 8) left = anchor.right;
    else top = window.innerHeight - anchor.bottom >= box.height + 8 ? anchor.bottom : anchor.top - box.height;
    this.labels.style.left = `${clamp(left, window.innerWidth - box.width)}px`;
    this.labels.style.top = `${clamp(top, window.innerHeight - box.height)}px`;
  }
  zoom(inward: boolean): void {
    if (!this.view?.history) return;
    const width = this.view.history_viewport.manual_width ?? this.view.settings.history_range ?? Math.max(30, this.view.history.end - this.view.history.start);
    this.send({ type: 'set_history_viewport', end: this.view.history.end, width: Math.max(30, Math.min(1800, Math.round(width * (inward ? 0.8 : 1.25)))) });
  }
  private step(direction: number): void {
    if (!this.view) return;
    const points = this.kind === 'overview' ? this.view.overview.samples : this.view.history?.points ?? [];
    const current = this.hover ?? points.at(-1)?.at ?? 0;
    const next = direction < 0 ? [...points].reverse().find(p => p.at < current) : points.find(p => p.at > current);
    if (next) { this.hover = next.at; this.updateReadout(); }
  }
  private updateReadout(): void {
    if (!this.view) return;
    if (this.kind === 'overview') {
      const overview = this.view.overview;
      const sample = overview.samples.reduce<(typeof overview.samples)[number] | null>((nearest, point) => !nearest || Math.abs(point.at - (this.hover ?? overview.end)) < Math.abs(nearest.at - (this.hover ?? overview.end)) ? point : nearest, null);
      if (!sample) { text(this.data, 'No system memory samples recorded'); return; }
      const time = sample.captured_unix_ms ? dateTime(sample.captured_unix_ms) : `${elapsed(sample.at)} elapsed`;
      const layer = overview.layers[this.band];
      text(this.data, `${time} · ${elapsed(sample.at)} elapsed · ${layer?.name ?? 'System memory'} ${bytes(sample.layers?.[this.band] ?? null)} · Used ${bytes(sample.used)} · Available ${bytes(sample.available)} · ${sample.measured} of ${sample.observed} processes measured${sample.reason ? ` · ${sample.reason}` : ''}`);
    } else {
      const history = this.view.history;
      const point = history?.points.reduce<(NonNullable<View['history']>['points'])[number] | null>((nearest, p) => !nearest || Math.abs(p.at - (this.hover ?? history.end)) < Math.abs(nearest.at - (this.hover ?? history.end)) ? p : nearest, null);
      text(this.data, point ? `${this.view.selected?.name ?? 'Selection'} · ${point.captured_unix_ms ? dateTime(point.captured_unix_ms) : `${elapsed(point.at)} elapsed`} · ${elapsed(point.at)} elapsed · ${this.view.settings.metric === 'private_commit' ? 'Private commit' : 'Private working set'} ${bytes(point.reading.value)} · ${readingHelp(point.reading)}` : 'Select a process or group to inspect history');
    }
    let tooltip = this.drawing.querySelector<SVGTitleElement>('title');
    if (!tooltip) { tooltip = svg('title'); this.drawing.prepend(tooltip); }
    tooltip.textContent = this.data.textContent;
  }
  private draw(): void {
    const view = this.view; if (!view) return;
    const scale = view.settings.ui_zoom / 100;
    const width = Math.max(260 * scale, this.element.clientWidth || 600 * scale);
    this.compact = width / scale < 440;
    this.element.classList.toggle('chart-compact', this.compact); this.updateLabels();
    let height = 220 * scale;
    if (this.kind === 'overview') {
      const panel = this.element.parentElement;
      const pane = panel?.parentElement;
      // Wide plots reserve room for inline labels. Compact labels float outside
      // normal flow, so their plot can use the released width and height.
      const overhead = [...this.element.children].filter(node => node !== this.drawing && node !== this.labelToggle && !this.compact).reduce((sum, node) => {
        const style = window.getComputedStyle(node); return sum + node.getBoundingClientRect().height + (parseFloat(style.marginTop) || 0) + (parseFloat(style.marginBottom) || 0);
      }, 0) + (panel?.querySelector('.panel-heading')?.getBoundingClientRect().height ?? 0) + 28 * scale;
      height = this.expanded ? Math.max(112 * scale, (panel?.clientHeight || 760 * scale) - overhead)
        : this.element.closest('.reflow') ? 200 * scale : Math.max((this.compact ? 168 : 112) * scale, Math.min((this.compact ? 360 : 200) * scale, (pane?.clientHeight || 760 * scale) * .45 - overhead));
    }
    const left = (this.compact ? 12 : 52) * scale, right = width - (this.compact ? 12 : this.kind === 'overview' ? 145 : 18) * scale, top = 25 * scale, bottom = height - 34 * scale;
    this.plot = { width, left, right };
    this.drawing.setAttribute('viewBox', `0 0 ${width} ${height}`); this.drawing.style.height = `${height}px`;
    this.drawing.replaceChildren();
    const source = this.kind === 'overview' ? view.overview : view.history;
    if (!source) { text(this.description, 'No selected history'); return; }
    const x = (at: number) => left + (at - source.start) / Math.max(1, source.end - source.start) * (right - left);
    let min = 0, max = Number(view.overview.capacity) || 1;
    if (this.kind === 'history') {
      const values = view.history!.points.filter(p => p.reading.measured === p.reading.total && p.reading.enumeration_complete).map(p => plotValue(p.reading.value)).filter((v): v is number => v !== null);
      max = Math.max(1, ...values) * 1.1;
      if (!view.settings.from_zero && values.length) { min = Math.max(0, Math.min(...values)); const padding = Math.max(1048576, (max - min) * 0.1); min = Math.max(0, min - padding); max += padding; }
    }
    const y = (value: number) => bottom - (value - min) / Math.max(1, max - min) * (bottom - top);
    const annotations = svg('g', { class: 'chart-annotations' });
    const addText = (px: number, py: number, value: string, anchor = 'start', color = '#b8c0c6') => annotations.append(svg('text', { x: px, y: py, 'font-size': 12 * scale, 'text-anchor': anchor, fill: color }, value));
    const unit = this.kind === 'overview' ? 1073741824 : 1048576;
    const capacity = BigInt(view.overview.capacity) > 0n ? view.overview.capacity : null;
    addText(left, 14 * scale, this.kind === 'overview' ? `Capacity: ${bytes(capacity)}` : 'MiB');
    if (this.kind === 'history' || capacity !== null) for (let i = 0; i <= 3; i++) { const value = min + (max - min) * i / 3; const py = y(value); this.drawing.append(svg('line', { x1: left, y1: py, x2: right, y2: py, stroke: '#42494f', 'stroke-dasharray': i === 3 && this.kind === 'overview' ? '5 4' : '0' })); addText(left + (this.compact ? 4 : -6) * scale, py + 4 * scale, (value / unit).toFixed(1), this.compact ? 'start' : 'end'); }
    this.drawing.append(svg('line', { x1: left, y1: top, x2: left, y2: bottom, stroke: '#b8c0c6' }));
    addText(left, height - 12 * scale, elapsed(source.start)); addText(right, height - 12 * scale, elapsed(source.end), 'end');
    if (this.kind === 'overview') {
      const overview = view.overview;
      // Unknown attribution retains the valid system total as a neutral area; system failures leave a gap.
      const neutral = segments(overview.samples, p => p.used === null ? null : { x: x(p.at), low: y(0), high: y(Number(p.used)) });
      this.drawing.append(svg('path', { d: pathSegments(neutral), fill: '#7b858b', opacity: '0.5' }));
      overview.layers.forEach((_layer, index) => {
        const bands = segments(overview.samples, p => {
          if (!p.layers || p.used === null) return null;
          const low = p.layers.slice(0, index).reduce((sum, value) => sum + Number(value), 0);
          return { x: x(p.at), low: y(low), high: y(low + Number(p.layers[index] ?? 0)) };
        });
        const path = svg('path', { d: pathSegments(bands), fill: colors[index % colors.length], stroke: '#202327', 'stroke-width': 0.6, opacity: this.band === index ? '1' : '0.88' });
        path.addEventListener('pointerenter', () => { this.band = index; this.updateReadout(); }); this.drawing.append(path);
      });
      const latest = overview.samples.at(-1);
      this.readout.replaceChildren(el('span', '', `Capacity ${bytes(capacity)}`), el('span', '', `Used ${bytes(latest?.used ?? null)}`), el('span', '', `Available ${bytes(latest?.available ?? null)}`));
      if (latest?.used !== null && latest?.used !== undefined) {
        const py = y(Number(latest.used)); this.drawing.append(svg('circle', { cx: x(latest.at), cy: py, r: 3 * scale, fill: '#eceff1' }));
        if (!this.compact) this.drawing.append(svg('path', { d: `M${right - 6 * scale},${top} h${-5 * scale} M${right - 6 * scale},${top} V${py} h${-5 * scale}`, fill: 'none', stroke: '#b8c0c6' }));
        const endpointY = Math.max(top + 30 * scale, py + 4 * scale);
        const availableY = Math.min(endpointY - 16 * scale, Math.max(top + 11 * scale, (top + py) / 2 + 4 * scale));
        if (this.compact) addText(right - 4 * scale, Math.max(top + 16 * scale, py - 7 * scale), bytes(latest.used), 'end', '#eceff1');
        else {
          if (endpointY !== py + 4 * scale) this.drawing.append(svg('line', { x1: right, y1: py, x2: right + 6 * scale, y2: endpointY - 4 * scale, stroke: '#eceff1' }));
          addText(right + 8 * scale, endpointY, bytes(latest.used), 'start', '#eceff1');
          addText(right + 8 * scale, availableY, `Available ${bytes(latest.available)}`);
        }
      }
      overview.layers.forEach((layer, index) => {
        let entry = this.entries[index];
        if (!entry) { entry = button('', () => { this.band = index; this.inspection.open = true; this.updateReadout(); }, 'legend-entry'); this.entries.push(entry); this.legend.append(entry); entry.addEventListener('focus', () => { this.band = index; this.inspection.open = true; this.updateReadout(); }); entry.addEventListener('keydown', e => { if (['ArrowLeft', 'ArrowRight'].includes(e.key)) { e.preventDefault(); this.band = index; this.step(e.key === 'ArrowLeft' ? -1 : 1); } }); }
        entry.style.setProperty('--band-color', colors[index % colors.length]); text(entry, `${layer.name} · ${bytes(latest?.layers?.[index] ?? null)}`); entry.title = `${layer.name}; attributed private working set${index === 0 ? '; residual or unattributed system usage' : ''}`;
      });
      for (let index = overview.layers.length; index < this.entries.length; index++) this.entries[index].remove();
      text(this.description, `All processes + other used RAM · ${duration(overview.recorded)} recorded`);
    } else {
      const history = view.history!;
      const complete = segments(history.points, p => p.reading.value === null || p.reading.measured !== p.reading.total || !p.reading.enumeration_complete ? null : { x: x(p.at), low: bottom, high: y(Number(p.reading.value)) });
      this.drawing.append(svg('path', { d: pathSegments(complete), fill: '#6cbc8b', 'fill-opacity': '0.22', stroke: '#6cbc8b', 'stroke-width': 2 }));
      const point = history.points.at(-1); if (point && point.reading.value !== null && point.reading.measured === point.reading.total && point.reading.enumeration_complete) { const py = y(Number(point.reading.value)); this.drawing.append(svg('circle', { cx: x(point.at), cy: py, r: 3 * scale, fill: '#6cbc8b' })); addText(right - 7 * scale, Math.max(top + 16 * scale, py - 7 * scale), bytes(point.reading.value), 'end', '#6cbc8b'); }
      this.legend.hidden = true; text(this.readout, `${view.settings.from_zero ? 'From zero' : 'Fit values'} · ${bytes(Math.round(min).toString())} — ${bytes(Math.round(max).toString())}`);
      text(this.description, `Requested: ${rangeLabel(view.settings.history_range)} · Recorded: ${duration(history.recorded)}`);
    }
    this.drawing.append(annotations);
  }
}
