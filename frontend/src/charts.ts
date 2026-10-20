import type { Action, Inspection, OverviewSample, View } from './contract.ts';
import { iconButton, el, text } from './dom.ts';
import { bytes, changePercent, dateTime, plotValue, readingHelp } from './format.ts';
const NS = 'http://www.w3.org/2000/svg';
function age(seconds: number): string {
  const value = Math.max(0, Math.round(seconds));
  if (value < 60) return `${value}s`;
  if (value < 3600) return `${Math.floor(value / 60)}m${value % 60 ? ` ${value % 60}s` : ''}`;
  return `${Math.floor(value / 3600)}h${Math.floor(value % 3600 / 60) ? ` ${Math.floor(value % 3600 / 60)}m` : ''}`;
}
const colors = ['#7b858b', '#347758', '#6cbc8b', '#f2a14a', '#d48c36'];
function share(value: string | null, capacity: string): string | null {
  if (value === null || BigInt(value) < 0n || BigInt(capacity) <= 0n) return null;
  const total = BigInt(capacity), tenths = (BigInt(value) * 1000n + total / 2n) / total;
  return `${tenths / 10n}.${tenths % 10n}%`;
}
function nearest<T extends { at: number }>(points: readonly T[], at: number): T | null {
  return points.reduce<T | null>((best, point) => !best || Math.abs(point.at - at) < Math.abs(best.at - at) ? point : best, null);
}
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
  private data = el('output', 'chart-data sr-only');
  private bandLabels = svg('g', { class: 'band-labels', 'pointer-events': 'none' });
  private labels = el('div', 'chart-labels');
  private labelToggle: HTMLButtonElement;
  private compact = false;
  private expanded = false;
  private detailed = false;
  private cursor = svg('g', { class: 'chart-cursor', 'pointer-events': 'none' });
  private pointerInside = false;
  private pinned = false;
  private dismissed = false;
  private entries: SVGGElement[] = [];
  private view: View | null = null;
  private selectedId: string | null = null;
  private kind: 'overview' | 'history';
  private send: (action: Action) => void;
  private inspect: (sample: Inspection | null) => void;
  private hover: number | null = null;
  private band = 0;
  private drag: { x: number; end: number; width: number } | null = null;
  private plot = { width: 1, left: 0, right: 1, top: 0, bottom: 1, minimum: 0, maximum: 1 };
  constructor(kind: 'overview' | 'history', send: (action: Action) => void, inspect: (sample: Inspection | null) => void = () => {}) {
    this.kind = kind; this.send = send; this.inspect = inspect;
    this.element.setAttribute('role', 'group');
    this.element.setAttribute('aria-label', `${kind === 'overview' ? 'Overall memory' : 'Selected history'} chart; focus or hover for labels; Left and Right Arrow inspect samples, Up and Down Arrow select bands`);
    this.labels.id = `${kind}-chart-labels`;
    this.labelToggle = iconButton('Show chart labels', 'info', () => { this.pinned = !this.pinned; this.dismissed = !this.pinned; this.updateLabels(); }, 'chart-label-toggle');
    this.labelToggle.setAttribute('aria-controls', this.labels.id);
    this.element.addEventListener('pointerenter', e => { if (e.pointerType !== 'touch') { this.pointerInside = true; this.dismissed = false; this.updateLabels(); } });
    this.element.addEventListener('pointerleave', () => { this.pointerInside = false; if (!this.drag) { this.hover = null; this.updateReadout(); } this.updateLabels(); });
    this.element.addEventListener('focusin', () => { this.dismissed = false; this.updateLabels(); });
    this.element.addEventListener('focusout', () => queueMicrotask(() => {
      if (!this.element.contains(document.activeElement)) this.dismissLabels(); else this.updateLabels();
    }));
    document.addEventListener('pointerdown', e => {
      if (this.compact && !this.element.contains(e.target as Node)) this.dismissLabels();
    });
    window.addEventListener('scroll', () => { if (this.compact && !this.labels.hidden) this.positionLabels(); }, { capture: true, passive: true });
    this.element.addEventListener('keydown', e => {
      if (this.kind === 'history' && e.key === 'Escape') { this.hover = null; this.updateReadout(); }
      if (e.key === 'Escape' && this.compact && !this.labels.hidden) {
        e.preventDefault(); e.stopPropagation(); this.element.focus({ preventScroll: true });
        this.dismissLabels();
      }
    });
    this.data.id = `${kind}-chart-data`; this.data.setAttribute('aria-live', 'off');
    this.element.setAttribute('aria-describedby', this.data.id);
    this.labels.append(this.description);
    this.element.append(this.drawing, this.labelToggle, this.labels, this.data);
    this.element.addEventListener('keydown', e => {
      if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') { e.preventDefault(); this.step(e.key === 'ArrowLeft' ? -1 : 1); }
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault(); this.band = Math.max(0, Math.min((this.view?.overview.layers.length ?? 1) - 1, this.band + (e.key === 'ArrowUp' ? 1 : -1))); this.updateReadout();
      }
    });
    this.drawing.addEventListener('pointermove', e => {
      if (!this.view) return;
      const source = this.kind === 'overview' ? this.view.overview : this.view.history; if (!source) return;
      const box = this.drawing.getBoundingClientRect();
      const pixels = Math.max(1, (this.plot.right - this.plot.left) / this.plot.width * box.width);
      const left = box.left + this.plot.left / this.plot.width * box.width;
      if (this.drag) { const delta = (e.clientX - this.drag.x) / pixels * this.drag.width; this.send({ type: 'set_history_viewport', end: Math.round(Math.max(0, Math.min(this.view.displayed_at, this.drag.end - delta))), width: this.drag.width }); }
      else { this.hover = source.start + Math.max(0, Math.min(1, (e.clientX - left) / pixels)) * (source.end - source.start); this.updateReadout(); }
    });
    this.drawing.addEventListener('pointerleave', () => { if (!this.drag) { this.hover = null; this.updateReadout(); } });
    if (kind === 'history') {
      this.drawing.addEventListener('wheel', e => { if (e.ctrlKey || !this.view?.history) return; e.preventDefault(); this.zoom(e.deltaY < 0); }, { passive: false });
      this.drawing.addEventListener('pointerdown', e => {
        if (!this.view?.history || e.button !== 0) return; e.preventDefault();
        this.drag = { x: e.clientX, end: this.view.history.end, width: Math.max(30, this.view.history.end - this.view.history.start) };
        this.drawing.setPointerCapture(e.pointerId); this.element.focus({ preventScroll: true });
      });
      this.drawing.addEventListener('pointerup', e => { this.drag = null; this.drawing.releasePointerCapture(e.pointerId); });
      this.drawing.addEventListener('pointercancel', () => { this.drag = null; });
    }
    if (typeof ResizeObserver !== 'undefined') new ResizeObserver(() => this.draw()).observe(this.element);
  }
  private color(index: number): string { return colors[(index + (this.view?.settings.overview_metric === 'private_commit' ? 1 : 0)) % colors.length]; }
  update(view: View): void {
    const selected = this.view?.overview.layers[this.band];
    if (this.kind === 'history' && this.selectedId !== (view.selected?.id ?? null)) this.hover = null;
    this.selectedId = view.selected?.id ?? null;
    this.view = view;
    if (selected) this.band = Math.max(0, view.overview.layers.findIndex(layer => selected.id ? layer.id === selected.id : layer.name === selected.name));
    this.draw();
  }
  setExpanded(value: boolean): void { this.expanded = value; this.draw(); }
  private dismissLabels(): void { this.pinned = false; this.dismissed = true; if (this.kind === 'history') { this.hover = null; this.updateReadout(); } this.updateLabels(); }
  private updateLabels(): void {
    const visible = !this.compact || !this.dismissed && (this.pinned || this.pointerInside || this.element.contains(document.activeElement));
    const popover = typeof this.labels.showPopover === 'function' && this.labels.isConnected;
    if (popover && this.labels.matches(':popover-open') && (!visible || !this.compact)) this.labels.hidePopover();
    if (this.compact) this.labels.setAttribute('popover', 'manual');
    else { this.labels.removeAttribute('popover'); this.labels.style.left = ''; this.labels.style.top = ''; }
    this.labels.hidden = !visible; this.labelToggle.hidden = !this.compact;
    this.element.tabIndex = 0;
    this.element.classList.toggle('labels-visible', this.compact && visible);
    this.labelToggle.setAttribute('aria-expanded', String(visible));
    this.labelToggle.setAttribute('aria-label', `${visible ? 'Hide' : 'Show'} chart labels`); this.labelToggle.title = this.labelToggle.getAttribute('aria-label')!;
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
    const current = nearest<{ at: number }>(points, this.hover ?? points.at(-1)?.at ?? 0)?.at ?? 0;
    const next = direction < 0 ? [...points].reverse().find(p => p.at < current) : points.find(p => p.at > current);
    if (next) { this.hover = next.at; this.updateReadout(); }
  }
  private updateReadout(): void {
    if (!this.view) return;
    this.cursor.replaceChildren();
    if (this.kind === 'overview') {
      const overview = this.view.overview;
      const commit = this.view.settings.overview_metric === 'private_commit';
      const sample = nearest(overview.samples, this.hover ?? overview.end);
      this.updateOverviewDetails(sample);
      if (!sample) { text(this.data, 'No system memory samples recorded'); return; }
      const time = sample.captured_unix_ms ? dateTime(sample.captured_unix_ms) : `${age(overview.end - sample.at)} ago`;
      const layer = overview.layers[this.band];
      const bandShare = this.expanded ? share(sample.layers?.[this.band] ?? null, commit ? sample.used ?? '0' : sample.capacity) : null;
      text(this.data, `${time} · ${age(overview.end - sample.at)} ago · ${layer?.program_name?.trim() || layer?.name || 'System memory'} ${bytes(sample.layers?.[this.band] ?? null)} (${sample.layers?.[this.band] ?? 'Unavailable'} raw bytes)${bandShare === null ? '' : ` (${bandShare} of ${commit ? 'measured private commit' : 'physical RAM'})`} · ${commit ? `Measured private commit ${bytes(sample.used)}` : `Capacity ${bytes(BigInt(sample.capacity) > 0n ? sample.capacity : null)} · Used ${bytes(sample.used)} · Available ${bytes(sample.available)}`} · ${sample.measured} of ${sample.observed} processes measured${sample.reason ? ` · ${sample.reason}` : ''}`);
      if (this.expanded && this.hover !== null) {
        const px = overview.end === overview.start ? this.plot.right : this.plot.left + (sample.at - overview.start) / (overview.end - overview.start) * (this.plot.right - this.plot.left);
        this.cursor.append(svg('line', { x1: px, x2: px, y1: this.plot.top, y2: this.plot.bottom, stroke: '#eceff1', 'stroke-dasharray': '3 3' }));
        if (sample.used !== null) this.cursor.append(svg('circle', { cx: px, cy: this.plot.bottom - (Number(sample.used) - this.plot.minimum) / (this.plot.maximum - this.plot.minimum) * (this.plot.bottom - this.plot.top), r: 4 * this.view.settings.ui_zoom / 100, fill: '#eceff1', stroke: '#202327' }));
      }
    } else {
      const history = this.view.history;
      const point = nearest(history?.points ?? [], this.hover ?? history?.end ?? 0);
      this.inspect(this.hover === null ? history?.latest_inspection ?? null : point?.inspection ?? null);
      text(this.data, point ? `${this.view.selected?.name ?? 'Selection'} · ${point.captured_unix_ms ? dateTime(point.captured_unix_ms) : `${age(history!.end - point.at)} ago`} · ${age(history!.end - point.at)} ago · ${this.view.settings.metric === 'private_commit' ? 'Private commit' : 'Private working set'} ${bytes(point.reading.value)} · ${readingHelp(point.reading)}` : 'Select a process or group to inspect history');
    }
    let tooltip = this.drawing.querySelector<SVGTitleElement>(':scope > title');
    if (!tooltip) { tooltip = svg('title'); this.drawing.prepend(tooltip); }
    tooltip.textContent = this.data.textContent;
  }
  private updateOverviewDetails(sample: OverviewSample | null): void {
    const overview = this.view!.overview, scale = this.view!.settings.ui_zoom / 100;
    const { left, right, top } = this.plot;
    const commit = this.view!.settings.overview_metric === 'private_commit';
    const capacity = commit ? sample?.used ?? '0' : sample?.capacity ?? overview.capacity;
    const labelWidth = Math.min(420 * scale, right - left - 20 * scale);
    const interval = this.view!.settings.change_seconds;
    // The Rust projection supplies the retained baseline even when it falls
    // outside the visible plot. Missing/stale readings stay unavailable.
    const baseline = sample?.baseline;
    if (this.expanded) this.bandLabels.removeAttribute('display');
    else this.bandLabels.setAttribute('display', 'none');
    overview.layers.forEach((layer, index) => {
      let entry = this.entries[index];
      if (!entry) { entry = svg('g', { class: 'band-label', 'pointer-events': 'all' }); this.entries[index] = entry; entry.addEventListener('pointerenter', () => { this.band = index; this.updateReadout(); }); }
      const amount = sample?.used == null ? null : sample.layers?.[index] ?? null;
      const percentage = share(amount, capacity), before = baseline?.layers?.[index] ?? null;
      const delta = amount === null || before === null ? null : (BigInt(amount) - BigInt(before)).toString();
      const change = changePercent({ value: delta, reason: baseline ? `Baseline ${age((sample?.at ?? 0) - baseline.at)} before sample` : `No valid baseline for ${age(interval)} change in retained history` }, { value: amount, measured: amount === null ? 0 : 1, total: 1, enumeration_complete: amount !== null, reason: null });
      const y = top + 10 * scale + index * 24 * scale, x = left + 10 * scale;
      const fullName = layer.program_name?.trim() || layer.name;
      const letters = [...fullName], name = letters.length > 20 ? letters.slice(0, 19).join('') + '\u2026' : fullName;
      entry.setAttribute('data-label-y', String(y));
      entry.replaceChildren(svg('title', {}, `${fullName}; ${layer.name}; ${amount ?? 'Unavailable'} raw bytes; share of ${commit ? 'measured private commit' : 'physical RAM'} at ${sample ? dateTime(sample.captured_unix_ms) : 'unavailable time'}; Change over ${age(interval)}: ${change.label}; ${change.help}${sample?.reason ? `; ${sample.reason}` : ''}`));
      entry.append(svg('rect', { x, y, width: labelWidth, height: 22 * scale, rx: 3 * scale, fill: '#202327', 'fill-opacity': '.88' }));
      entry.append(svg('rect', { x, y, width: 3 * scale, height: 22 * scale, fill: this.color(index) }));
      const fields = [
        { kind: 'name', value: name, start: 0, end: .40, color: '#eceff1' },
        { kind: 'value', value: bytes(amount), start: .40, end: .63, color: '#eceff1' },
        { kind: 'share', value: percentage ?? '\u2014', start: .63, end: .80, color: '#b8c0c6' },
        { kind: 'change', value: change.label, start: .80, end: 1, color: change.color },
      ];
      fields.forEach(field => {
        const id = `overview-legend-${index}-${field.kind}`, clip = svg('clipPath', { id });
        clip.append(svg('rect', { x: x + labelWidth * field.start + 7 * scale, y, width: Math.max(1, labelWidth * (field.end - field.start) - 14 * scale), height: 22 * scale }));
        const cell = svg('text', { class: `legend-${field.kind}`, x: field.kind === 'name' ? x + 7 * scale : x + labelWidth * field.end - 7 * scale, y: y + 15 * scale, fill: field.color, 'font-size': 12 * scale, 'text-anchor': field.kind === 'name' ? 'start' : 'end', 'clip-path': `url(#${id})` }, field.value);
        entry.append(clip, cell);
      });
      this.bandLabels.append(entry);
    });
    for (let index = overview.layers.length; index < this.entries.length; index++) this.entries[index].remove();
    text(this.description, sample ? `${this.hover === null ? 'Latest' : 'Inspecting'} · ${age(overview.end - sample.at)} ago · ${sample.measured} of ${sample.observed} processes measured` : 'No system memory samples recorded');
    this.description.title = sample ? dateTime(sample.captured_unix_ms) : '';
  }
  private draw(): void {
    const view = this.view; if (!view) return;
    const scale = view.settings.ui_zoom / 100;
    const width = Math.max(260 * scale, this.element.clientWidth || 600 * scale);
    this.compact = width / scale < 440;
    this.detailed = this.expanded && this.kind === 'overview' && !this.compact && (this.element.parentElement?.clientHeight || 760 * scale) / scale >= 560;
    this.description.classList.toggle('sr-only', !this.compact);
    this.element.classList.toggle('chart-detailed', this.detailed);
    if (!this.detailed) this.description.title = '';
    this.element.classList.toggle('chart-compact', this.compact); this.updateLabels();
    let height = 220 * scale;
    if (this.kind === 'overview') {
      const panel = this.element.parentElement;
      // Legend and sample help float over/outside the plot; only the panel
      // heading and padding consume height.
      const overhead = (panel?.querySelector('.panel-heading')?.getBoundingClientRect().height ?? 0) + 28 * scale;
      height = !this.expanded && this.element.closest('.reflow') ? 200 * scale
        : Math.max((this.compact && !this.expanded ? 168 : 112) * scale, (panel?.clientHeight || 260 * scale) - overhead);
    }
    const left = (this.compact ? 12 : 52) * scale, right = width - (this.compact ? 12 : 18) * scale, top = 25 * scale, bottom = height - 34 * scale;
    this.plot = { width, left, right, top, bottom, minimum: 0, maximum: Number(view.overview.capacity) || 1 };
    this.drawing.setAttribute('viewBox', `0 0 ${width} ${height}`); this.drawing.style.height = `${height}px`;
    this.drawing.replaceChildren();
    const source = this.kind === 'overview' ? view.overview : view.history;
    if (!source) { text(this.description, 'No selected history'); this.updateReadout(); return; }
    const x = (at: number) => source.end === source.start ? right : left + (at - source.start) / (source.end - source.start) * (right - left);
    const commit = view.settings.overview_metric === 'private_commit';
    let min = 0, max = this.kind === 'overview' && commit ? Math.max(1073741824, ...view.overview.samples.map(sample => Number(sample.used ?? 0))) * 1.1 : Number(view.overview.capacity) || 1;
    if (this.kind === 'history') {
      const values = view.history!.points.filter(p => p.reading.measured === p.reading.total && p.reading.enumeration_complete).map(p => plotValue(p.reading.value)).filter((v): v is number => v !== null);
      max = Math.max(1, ...values) * 1.1;
      if (!view.settings.from_zero && values.length) { min = Math.max(0, Math.min(...values)); const padding = Math.max(1048576, (max - min) * 0.1); min = Math.max(0, min - padding); max += padding; }
    }
    if (this.kind === 'overview' && !view.settings.overview_from_zero) {
      const values = view.overview.samples.filter(p => p.used !== null).map(p => Number(p.used));
      if (values.length) {
        min = Math.min(...values); max = Math.max(1, ...values);
        const padding = Math.max(1048576, (max - min) * 0.1);
        min = Math.max(0, min - padding); max += padding;
        if (!commit && Number(view.overview.capacity) > 0) max = Math.min(max, Number(view.overview.capacity));
      }
    }
    this.plot.minimum = min;
    this.plot.maximum = max;
    const clipId = `${this.kind}-plot-clip`;
    const clip = svg('clipPath', { id: clipId, class: 'chart-plot-clip' });
    clip.append(svg('rect', { x: left, y: top, width: right - left, height: bottom - top }));
    this.drawing.append(clip);
    const areas = svg('g', { class: 'chart-areas', 'clip-path': `url(#${clipId})` });
    this.drawing.append(areas); this.cursor.setAttribute('clip-path', `url(#${clipId})`);
    const y = (value: number) => bottom - (value - min) / Math.max(1, max - min) * (bottom - top);
    const annotations = svg('g', { class: 'chart-annotations' });
    const addText = (px: number, py: number, value: string, anchor = 'start', color = '#b8c0c6') => annotations.append(svg('text', { x: px, y: py, 'font-size': 12 * scale, 'text-anchor': anchor, fill: color }, value));
    const unit = this.kind === 'overview' ? 1073741824 : 1048576;
    const capacity = BigInt(view.overview.capacity) > 0n ? view.overview.capacity : null;
    addText(left, 14 * scale, this.kind === 'overview' ? 'GiB' : 'MiB');
    const detailed = this.expanded && this.kind === 'overview' && !this.compact;
    const memoryDivisions = detailed ? Math.max(3, Math.min(8, Math.floor((bottom - top) / (60 * scale)))) : 3;
    const digits = Math.max(1, Math.min(4, Math.ceil(-Math.log10((max - min) / unit / memoryDivisions))));
    if (this.kind === 'history' || commit || capacity !== null) for (let i = 0; i <= memoryDivisions; i++) {
      const value = min + (max - min) * i / memoryDivisions, py = y(value);
      this.drawing.append(svg('line', { x1: left, y1: py, x2: right, y2: py, stroke: '#42494f', 'stroke-dasharray': i === memoryDivisions && this.kind === 'overview' && !commit && max === Number(view.overview.capacity) ? '5 4' : '0' }));
      addText(left + (this.compact ? 4 : -6) * scale, py + 4 * scale, (value / unit).toFixed(digits), this.compact ? 'start' : 'end');
    }
    this.drawing.append(svg('line', { x1: left, y1: top, x2: left, y2: bottom, stroke: '#b8c0c6' }));
    const timeDivisions = source.end === source.start ? 0 : detailed ? Math.min(Math.max(0, Math.floor(source.end - source.start)), Math.max(1, Math.min(8, Math.floor((right - left) / (90 * scale))))) : 1;
    for (let i = 0; i <= timeDivisions; i++) {
      const at = source.start + (source.end - source.start) * i / Math.max(1, timeDivisions), px = x(at);
      if (i > 0 && i < timeDivisions) this.drawing.append(svg('line', { x1: px, y1: top, x2: px, y2: bottom, stroke: '#42494f', 'stroke-dasharray': '2 4' }));
      addText(px, height - 12 * scale, age(source.end - at), timeDivisions === 0 ? 'end' : i === 0 ? 'start' : i === timeDivisions ? 'end' : 'middle');
    }
    if (this.kind === 'overview') {
      const overview = view.overview;
      // Unknown attribution retains the valid system total as a neutral area; system failures leave a gap.
      const neutral = segments(overview.samples, p => p.used === null ? null : { x: x(p.at), low: y(0), high: y(Number(p.used)) });
      areas.append(svg('path', { d: pathSegments(neutral), fill: '#7b858b', opacity: '0.5' }));
      overview.layers.forEach((_layer, index) => {
        const bands = segments(overview.samples, p => {
          if (!p.layers || p.used === null) return null;
          const low = p.layers.slice(0, index).reduce((sum, value) => sum + Number(value), 0);
          return { x: x(p.at), low: y(low), high: y(low + Number(p.layers[index] ?? 0)) };
        });
        const path = svg('path', { d: pathSegments(bands), fill: this.color(index), stroke: '#202327', 'stroke-width': 0.6, opacity: this.band === index ? '1' : '0.88' });
        path.addEventListener('pointerenter', () => { this.band = index; this.updateReadout(); }); areas.append(path);
      });
    } else {
      const history = view.history!;
      const complete = segments(history.points, p => p.reading.value === null || p.reading.measured !== p.reading.total || !p.reading.enumeration_complete ? null : { x: x(p.at), low: bottom, high: y(Number(p.reading.value)) });
      areas.append(svg('path', { d: pathSegments(complete), fill: '#6cbc8b', 'fill-opacity': '0.22', stroke: '#6cbc8b', 'stroke-width': 2 }));
      const point = history.points.at(-1); if (point && point.reading.value !== null && point.reading.measured === point.reading.total && point.reading.enumeration_complete) { const py = y(Number(point.reading.value)); areas.append(svg('circle', { cx: x(point.at), cy: py, r: 3 * scale, fill: '#6cbc8b' })); addText(right - 7 * scale, Math.max(top + 16 * scale, py - 7 * scale), bytes(point.reading.value), 'end', '#6cbc8b'); }
      text(this.description, 'Drag to pan · Scroll to zoom');
    }
    this.drawing.append(annotations, this.bandLabels, this.cursor); this.updateReadout();
  }
}
