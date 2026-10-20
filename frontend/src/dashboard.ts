import type { Action, Grouping, Metric, View } from './contract.ts';
import { checkbox, el, iconButton, radios, select, setIcon, text } from './dom.ts';
import { bytes, changePercent, dateTime, duration, elapsed, partial, reading, readingHelp } from './format.ts';
import { ProcessTreeGrid } from './treegrid.ts';
import { MemoryChart } from './charts.ts';

const metricChoices: [Metric, string][] = [['private_working_set', 'Private working set'], ['private_commit', 'Private commit']];
const rangeChoices: [string, string][] = [['300', '5m'], ['900', '15m'], ['1800', '30m'], ['all', 'All']];
export class Dashboard {
  readonly element = el('main', 'dashboard');
  readonly table: ProcessTreeGrid;
  readonly inspector = el('section', 'panel inspector-scroll');
  private view: View | null = null;
  private send: (action: Action) => void;
  private overview: MemoryChart;
  private overviewTitle = el('h2', '', 'Overall memory usage');
  private overviewExpanded = false;
  private overviewToggle: HTMLButtonElement;
  private processPanel = el('section', 'process-panel');
  private returnPosition = { tableTop: 0, tableLeft: 0, inspectorTop: 0 };
  private history: MemoryChart;
  private header = el('header', 'app-header');
  private footer = el('footer', 'app-footer');
  private system = el('p', 'system-summary', 'Waiting for first memory sample…');
  private available = el('span');
  private progress = el('progress');
  private live = el('p', 'live-status');
  private pause: HTMLButtonElement;
  private filter = el('input');
  private clear: HTMLButtonElement;
  private group: ReturnType<typeof radios<Grouping>>;
  private exited: ReturnType<typeof checkbox>;
  private compact: ReturnType<typeof checkbox>;
  private changeChoice: ReturnType<typeof select>;
  private overviewRange: ReturnType<typeof radios<string>>;
  private metric: ReturnType<typeof radios<Metric>>;
  private overviewMetric: ReturnType<typeof radios<Metric>>;
  private overviewAxis: ReturnType<typeof radios<string>>;
  private ranges: ReturnType<typeof radios<string>>;
  private axis: ReturnType<typeof radios<string>>;
  private follow: ReturnType<typeof checkbox>;
  private reset: HTMLButtonElement;
  private title = el('h2', '', 'Select a process');
  private counts = el('p', 'selection-counts');
  private hiddenSelection = el('p', 'selection-warning');
  private path = el('div', 'path-display path-preview');
  private pathPrefix = el('span', 'path-prefix');
  private pathFilename = el('span', 'path-filename');
  private paths = el('details', 'paths');
  private pathList = el('ul');
  private pathSummary = el('summary', '', 'Executable paths');
  private copy: HTMLButtonElement;
  private show: HTMLButtonElement;
  private values: HTMLElement[] = [];
  private changeLabel = el('dt', '', 'Change over 5m');
  private coverage = el('p', 'coverage');
  private tableStatus = el('p', 'table-status sr-only');
  private barsHelp = el('p', 'bars-help sr-only');
  private notice = el('p', 'notice');
  private zoom: ReturnType<typeof select>;
  private diagnostics = el('dialog', 'diagnostics');
  private diagnosticText = el('div', 'diagnostic-content');
  private diagnosticOpen = false;
  private debounce: ReturnType<typeof setTimeout> | undefined;
  private editingFilter = false;
  private splitter = el('div', 'splitter');
  private lastNotice = '';
  constructor(send: (action: Action) => void, native: (action: 'copy' | 'snapshot') => void) {
    this.send = send; this.table = new ProcessTreeGrid(send);
    this.overview = new MemoryChart('overview', send); this.history = new MemoryChart('history', send);
    const summary = el('div', 'header-summary');
    this.progress.max = 100; this.progress.setAttribute('aria-label', 'System physical memory used');
    summary.append(this.system, this.progress, this.available, this.live);
    this.pause = iconButton('Pause display', 'pause', () => { if (this.view) send({ type: 'set_paused', value: !this.view.paused }); }, 'accent');
    const snapshotCopy = iconButton('Copy snapshot to clipboard', 'copy', () => native('snapshot'), 'accent');
    snapshotCopy.title = 'Copy the displayed, filtered snapshot as CSV, including records outside the viewport; expansion does not change copied membership';
    const details = iconButton('Details', 'info', () => this.openDiagnostics(), 'accent'); details.setAttribute('aria-haspopup', 'dialog');
    const actions = el('div', 'header-actions'); actions.append(this.pause, details, snapshotCopy); this.header.append(summary, actions);
    const workspace = el('div', 'workspace'); const left = this.processPanel; left.setAttribute('aria-label', 'Process table');
    const toolbar = el('div', 'toolbar panel');
    this.group = radios('Group by', [['name', 'Name'], ['path', 'Path'], ['tree', 'Process tree'], ['process', 'Individual processes'], ['family', 'Family']], value => send({ type: 'set_grouping', value }), { name: 'tag', path: 'folder', tree: 'tree', process: 'process', family: 'family' });
    this.group.group.title = 'Family preserves the original launched-family grouping; Process tree shows actual ancestry';
    const filterLabel = el('div', 'filter-field'); this.filter.type = 'search'; this.filter.placeholder = 'Name, path or PID'; this.filter.id = 'process-filter';
    const filterCaption = el('label', '', 'Filter:'); filterCaption.htmlFor = this.filter.id; this.clear = iconButton('Clear filter', 'close', () => this.clearFilter(), 'quiet');
    this.filter.addEventListener('input', () => {
      this.editingFilter = true; if (this.debounce) clearTimeout(this.debounce);
      this.clear.hidden = !this.filter.value;
      this.debounce = setTimeout(() => { this.editingFilter = false; send({ type: 'set_filter', value: this.filter.value }); }, 200);
    });
    this.filter.addEventListener('keydown', e => { if (e.key === 'Escape') { e.preventDefault(); this.clearFilter(); } });
    filterLabel.append(filterCaption, this.filter, this.clear);
    this.exited = checkbox('Show exited processes', value => send({ type: 'set_show_exited', value }));
    this.compact = checkbox('Compact rows', value => send({ type: 'set_compact', value }));
    this.changeChoice = select('Change over:', [['60', '1 minute'], ['300', '5 minutes'], ['900', '15 minutes'], ['1800', '30 minutes']], value => send({ type: 'set_change_seconds', value: Number(value) }));
    const options = el('div', 'toolbar-options'); options.append(this.exited.label, this.compact.label, this.changeChoice.label);
    toolbar.append(this.group.group, filterLabel, options); left.append(toolbar, this.table.element, this.tableStatus, this.barsHelp);
    const right = el('div', 'right-pane'); const overviewPanel = el('section', 'panel overview-panel');
    overviewPanel.id = 'overall-memory-panel';
    const overviewHeading = el('div', 'panel-heading'); overviewHeading.append(this.overviewTitle);
    this.overviewMetric = radios('Overall memory metric', metricChoices, value => send({ type: 'set_overview_metric', value }), { private_working_set: 'ram', private_commit: 'commit' });
    this.overviewAxis = radios('Overall memory Y-axis', [['zero', 'From zero'], ['fit', 'Fit values']], value => send({ type: 'set_overview_from_zero', value: value === 'zero' }), { zero: 'zero', fit: 'fit' });
    this.overviewRange = radios('Overall memory view last', rangeChoices, value => send({ type: 'set_overview_range', seconds: value === 'all' ? null : Number(value) }));
    this.overviewToggle = iconButton('Expand overview', 'expand', () => this.setOverviewExpanded(!this.overviewExpanded), 'accent');
    this.overviewToggle.title = 'Make overall memory the main view'; this.overviewToggle.setAttribute('aria-controls', overviewPanel.id);
    this.overviewRange.group.querySelector('legend')!.classList.add('sr-only');
    const overviewControls = el('div', 'chart-controls'); overviewControls.append(this.overviewMetric.group, this.overviewAxis.group, this.overviewRange.group, this.overviewToggle); overviewHeading.append(overviewControls); overviewPanel.append(overviewHeading, this.overview.element);
    this.inspector.setAttribute('aria-label', 'Selected process inspector'); this.inspector.tabIndex = -1;
    const inspectorHeading = el('div', 'inspector-heading'); this.title.id = 'selected-title';
    this.inspector.setAttribute('aria-labelledby', this.title.id);
    this.show = iconButton('Show processes', 'list', () => send({ type: 'show_selected_processes' })); inspectorHeading.append(this.title, this.counts, this.show);
    const pathRow = el('div', 'path-row'); const pathLabel = el('div', 'path-label'); pathLabel.append(el('span', '', 'Executable path:'));
    this.path.tabIndex = 0; this.path.setAttribute('role', 'group'); this.path.setAttribute('aria-label', 'Full executable path');
    this.path.append(this.pathPrefix, this.pathFilename); pathLabel.append(this.path);
    this.copy = iconButton('Copy path', 'copy', () => native('copy')); pathRow.append(pathLabel, this.copy);
    this.paths.append(this.pathSummary, this.pathList);
    const summaryValues = el('dl', 'summary-values');
    ['Private working set', 'Private commit', '', 'Peak (last 30m)'].forEach((label, index) => { const dt = index === 2 ? this.changeLabel : el('dt', '', label); const dd = el('dd', '', '—'); dd.tabIndex = 0; this.values.push(dd); summaryValues.append(dt, dd); });
    this.metric = radios('Selected history metric', metricChoices, value => send({ type: 'set_metric', value }), { private_working_set: 'ram', private_commit: 'commit' });
    this.ranges = radios('View last', rangeChoices, value => send({ type: 'set_history_range', seconds: value === 'all' ? null : Number(value) }));
    this.ranges.group.classList.add('history-range'); this.ranges.group.querySelector('legend')!.classList.add('sr-only');
    this.axis = radios('Selected history Y-axis', [['zero', 'From zero'], ['fit', 'Fit values']], value => send({ type: 'set_from_zero', value: value === 'zero' }), { zero: 'zero', fit: 'fit' });
    for (const controls of [this.metric, this.axis, this.overviewMetric, this.overviewAxis]) controls.group.querySelector('legend')!.classList.add('sr-only');
    this.reset = iconButton('Reset view', 'reset', () => send({ type: 'reset_history_view' }));
    this.follow = checkbox('Follow latest', value => send({ type: 'set_follow', value })); const navigation = el('div', 'history-navigation'); navigation.append(this.reset, this.follow.label);
    const historyControls = el('div', 'history-controls'); historyControls.append(this.metric.group, this.ranges.group, this.axis.group, navigation);
    this.inspector.append(inspectorHeading, this.hiddenSelection, pathRow, this.paths, summaryValues, this.coverage, historyControls, this.history.element);
    right.append(overviewPanel, this.inspector); this.splitter.tabIndex = 0; this.splitter.setAttribute('role', 'separator'); this.splitter.setAttribute('aria-label', 'Resize process and memory panes'); this.splitter.setAttribute('aria-orientation', 'vertical');
    this.splitter.setAttribute('aria-valuemin', '40'); this.splitter.setAttribute('aria-valuemax', '80');
    workspace.append(left, this.splitter, right);
    this.zoom = select('View scale:', [50, 75, 100, 125, 150, 175, 200].map(n => [String(n), `${n}%`]), value => send({ type: 'set_ui_zoom', value: Number(value) }));
    const footerBottom = el('div', 'footer-bottom'); footerBottom.append(this.notice, this.zoom.label);
    this.notice.setAttribute('role', 'status'); this.notice.setAttribute('aria-live', 'polite');
    this.footer.append(footerBottom);
    const diagnosticHeading = el('h2', '', 'Collection details'); diagnosticHeading.id = 'diagnostic-title'; this.diagnostics.setAttribute('aria-labelledby', diagnosticHeading.id);
    const diagnosticHeader = el('div', 'diagnostic-heading');
    diagnosticHeader.append(diagnosticHeading, iconButton('Close details', 'close', () => this.closeDiagnostics()));
    this.diagnostics.append(diagnosticHeader, this.diagnosticText);
    this.diagnostics.addEventListener('close', () => { this.diagnosticOpen = false; });
    this.diagnostics.addEventListener('cancel', event => { event.preventDefault(); this.closeDiagnostics(); });
    this.element.append(this.header, workspace, this.footer, this.diagnostics);
    this.element.dataset.ready = 'false'; this.installShortcuts(); this.installSplitter();
    window.addEventListener('resize', () => this.updateLayout()); this.updateLayout();
  }
  private clearFilter(): void { if (this.debounce) clearTimeout(this.debounce); this.editingFilter = false; this.filter.value = ''; this.send({ type: 'set_filter', value: '' }); this.filter.focus(); }
  showNotice(value: string): void { this.lastNotice = value; text(this.notice, value); }
  update(view: View, explicitReveal = false): void {
    this.view = view; this.element.dataset.ready = 'true'; this.element.dataset.displayAt = String(view.displayed_at); this.element.dataset.revision = String(view.revision);
    document.documentElement.style.fontSize = `${14 * view.settings.ui_zoom / 100}px`;
    this.updateLayout();
    this.element.style.setProperty('--split', `${view.settings.splitter_permille / 10}%`);
    this.splitter.setAttribute('aria-valuenow', String(view.settings.splitter_permille / 10));
    const latest = view.system_memory; const used = latest.used, capacity = latest.capacity;
    const knownCapacity = BigInt(capacity) > 0n ? capacity : null;
    const percent = used === null || knownCapacity === null ? null : Number(BigInt(used) * 100n / BigInt(knownCapacity));
    const system = `System RAM used ${bytes(used)} / ${bytes(knownCapacity)}${percent === null ? '' : ` (${percent}%)`}`;
    text(this.system, system); text(this.available, `Available ${bytes(latest?.available ?? null)}`);
    if (percent === null) this.progress.removeAttribute('value'); else this.progress.value = percent;
    this.progress.setAttribute('aria-valuetext', `${system} · Available ${bytes(latest?.available ?? null)}`);
    const status = view.collection.status.toLowerCase(); const live = status === 'live';
    text(this.live, view.paused ? `Display paused at ${dateTime(view.displayed_unix_ms)} · ${live ? 'Recording continues' : `Collector ${status}`}` : live ? '' : `Collector ${status}${view.collection.error ? ` · ${view.collection.error}` : ''}`);
    this.live.hidden = live && !view.paused;
    this.live.title = `Last collection ${view.collection.age_seconds}s ago`; setIcon(this.pause, view.paused ? 'Resume display' : 'Pause display', view.paused ? 'play' : 'pause');
    this.group.set(view.settings.grouping); this.exited.input.checked = view.settings.show_exited; this.compact.input.checked = view.settings.compact;
    if (!this.editingFilter && document.activeElement !== this.filter) this.filter.value = view.settings.filter;
    this.clear.hidden = !this.filter.value; this.changeChoice.input.value = String(view.settings.change_seconds);
    this.overviewRange.set(view.settings.overview_range?.toString() ?? 'all'); this.metric.set(view.settings.metric); this.overviewMetric.set(view.settings.overview_metric); this.overviewAxis.set(view.settings.overview_from_zero ? 'zero' : 'fit');
    this.ranges.set(view.settings.history_range?.toString() ?? 'all'); this.axis.set(view.settings.from_zero ? 'zero' : 'fit');
    this.follow.input.checked = view.history_viewport.follow; this.reset.disabled = !view.history_viewport.reset_enabled;
    this.zoom.input.value = String(view.settings.ui_zoom);
    if (!this.overviewExpanded) this.table.update(view, explicitReveal);
    text(this.tableStatus, view.table.order_held ? 'Order held while inspecting' : view.table.selection_hidden ? 'Selection hidden by current filter or grouping; inspector retained' : '');
    text(this.barsHelp, `Row fill: relative to largest ${view.settings.grouping === 'process' || view.settings.grouping === 'tree' ? 'process' : 'group'} · Change: red ↑ · green ↓ · white unchanged`);
    const selected = view.selected; text(this.title, selected?.name ?? 'Select a process');
    text(this.hiddenSelection, view.table.selection_hidden ? 'Selected entity is hidden by the current filter or grouping.' : ''); this.hiddenSelection.hidden = !view.table.selection_hidden;
    text(this.counts, selected ? `${selected.count} ${selected.count === 1 ? 'process' : 'processes'} · ${selected.path_count} executable ${selected.path_count === 1 ? 'path' : 'paths'}${selected.pid === null ? '' : ` · PID ${selected.pid}`}${selected.lifecycle === 'exited' ? ` · Exited${selected.exited_at === null ? '' : ` at ${elapsed(selected.exited_at)} elapsed`}` : selected.lifecycle === 'unavailable' ? ' · Unavailable' : ''}` : '');
    this.show.hidden = !selected || selected.pid !== null;
    const fullPaths = selected?.paths ?? [];
    const pathValue = fullPaths.length === 1 ? fullPaths[0] : selected ? `${selected.path_count} executable paths${selected.paths_complete ? '' : ' · coverage incomplete'}` : '';
    this.path.title = fullPaths.join('\n'); this.path.setAttribute('aria-description', fullPaths.join('; '));
    const split = fullPaths.length === 1 ? Math.max(pathValue.lastIndexOf('\\'), pathValue.lastIndexOf('/')) : -1;
    text(this.pathPrefix, fullPaths.length === 1 ? pathValue.slice(0, Math.max(0, split)) : pathValue);
    text(this.pathFilename, fullPaths.length === 1 ? pathValue.slice(Math.max(0, split)) : '');
    this.pathFilename.hidden = fullPaths.length !== 1;
    this.path.classList.toggle('has-filename', fullPaths.length === 1);
    this.copy.setAttribute('aria-label', selected && !selected.paths_complete ? 'Copy known paths' : selected && selected.path_count > 1 ? 'Copy all paths' : 'Copy path'); this.copy.title = this.copy.getAttribute('aria-label')!; this.copy.disabled = !fullPaths.length;
    this.paths.hidden = fullPaths.length < 2; text(this.pathSummary, `${fullPaths.length} known executable paths${selected?.paths_complete ? '' : ' · coverage incomplete'}`);
    const existing = [...this.pathList.children]; fullPaths.forEach((path, i) => { let li = existing[i]; if (!li) { li = el('li'); this.pathList.append(li); } text(li, path); }); existing.slice(fullPaths.length).forEach(li => li.remove());
    [selected?.private, selected?.commit, null, selected?.peak].forEach((r, i) => {
      const percentage = i === 2 && selected ? changePercent(selected.change, selected.private, selected.lifecycle) : null;
      const value = i === 2 ? percentage?.label ?? '—' : r ? reading(r) : '—'; text(this.values[i], value);
      if (i === 2) this.values[i].style.color = percentage?.color ?? '#b8c0c6';
      const help = i === 2 ? percentage?.help ?? 'Select an entity' : r ? readingHelp(r) : 'Select an entity'; this.values[i].title = help; this.values[i].setAttribute('aria-label', `${['Private working set', 'Private commit', `Change over ${view.settings.change_seconds / 60}m`, 'Peak private working set last 30 minutes'][i]} ${value}; ${help}`);
    });
    text(this.changeLabel, `Change over ${view.settings.change_seconds / 60}m`);
    text(this.coverage, selected ? `${selected.private.measured} of ${selected.private.total} processes measured${partial(selected.private) ? ' · Partial' : ''}${selected.private.reason ? ` · ${selected.private.reason}` : ''}${selected.change.reason ? ` · Change: ${selected.change.reason}` : ''} · ${duration(selected.recorded)} recorded; peak uses the last 30 minutes` : '');
    text(this.overviewTitle, view.settings.overview_metric === 'private_commit' ? 'Measured private commit' : 'Overall memory usage');
    this.overview.update(view); this.history.update(view);
    if (view.notice !== this.lastNotice && view.notice) this.showNotice(view.notice);
    if (this.diagnosticOpen) this.renderDiagnostics();
  }
  private updateLayout(): void {
    const scale = (this.view?.settings.ui_zoom ?? 100) / 100;
    this.element.classList.toggle('reflow', window.innerWidth < 760 * scale || window.innerHeight < 600 * scale);
  }
  private setOverviewExpanded(expanded: boolean): void {
    if (expanded === this.overviewExpanded) return;
    if (expanded) this.returnPosition = { tableTop: this.table.element.scrollTop, tableLeft: this.table.element.scrollLeft, inspectorTop: this.inspector.scrollTop };
    this.overviewExpanded = expanded; this.element.classList.toggle('overview-expanded', expanded);
    this.processPanel.hidden = expanded; this.splitter.hidden = expanded; this.inspector.hidden = expanded;
    setIcon(this.overviewToggle, expanded ? 'Back to processes' : 'Expand overview', expanded ? 'back' : 'expand');
    this.overviewToggle.title = expanded ? 'Restore the process table and inspector (Escape)' : 'Make overall memory the main view';
    this.overview.setExpanded(expanded);
    if (!expanded && this.view) {
      this.table.update(this.view);
      this.table.element.scrollTop = this.returnPosition.tableTop; this.table.element.scrollLeft = this.returnPosition.tableLeft;
      this.inspector.scrollTop = this.returnPosition.inspectorTop;
    }
    this.overviewToggle.focus({ preventScroll: true });
  }
  private openDiagnostics(): void { this.renderDiagnostics(); this.diagnosticOpen = true; this.diagnostics.showModal(); }
  private closeDiagnostics(): void { this.diagnosticOpen = false; this.diagnostics.close(); }
  private renderDiagnostics(): void {
    const d = this.view?.diagnostics; if (!d) return;
    const lines = [`Coverage: ${d.current_measured} of ${d.current_observed} current processes measured${d.current_enumeration_complete ? '' : ' · enumeration incomplete'}`, `${d.current_unavailable} current unavailable readings`, `Last successful collection: ${d.last_success === null ? 'None' : `${elapsed(d.last_success)} elapsed`}`, `Retained snapshots: ${d.retained_snapshots}`, `Omitted detailed tracks: ${d.omitted_tracks}`, `Mem Map private memory: ${bytes(d.own_private)} · working set: ${bytes(d.own_working_set)}`, 'Recorded collection reasons:', ...(d.failures.length ? d.failures : ['No recorded collection failures'])];
    text(this.diagnosticText, lines.join('\n'));
  }
  private installSplitter(): void {
    this.splitter.addEventListener('keydown', e => { if (this.view && ['ArrowLeft', 'ArrowRight'].includes(e.key)) { e.preventDefault(); this.send({ type: 'set_splitter', value: Math.max(400, Math.min(800, this.view.settings.splitter_permille + (e.key === 'ArrowRight' ? 20 : -20))) }); } });
    this.splitter.addEventListener('pointerdown', e => {
      e.preventDefault(); this.splitter.setPointerCapture(e.pointerId);
      const box = this.splitter.parentElement!.getBoundingClientRect(); let split = this.view?.settings.splitter_permille ?? 680;
      const move = (event: PointerEvent) => { split = Math.round(Math.max(400, Math.min(800, (event.clientX - box.left) / box.width * 1000))); this.element.style.setProperty('--split', `${split / 10}%`); };
      const up = () => { this.splitter.removeEventListener('pointermove', move); this.splitter.releasePointerCapture(e.pointerId); this.send({ type: 'set_splitter', value: split }); };
      this.splitter.addEventListener('pointermove', move); this.splitter.addEventListener('pointerup', up, { once: true });
    });
  }
  private installShortcuts(): void {
    document.addEventListener('keydown', e => {
      const target = e.target; const editing = target instanceof HTMLElement && target.matches('input,textarea,select,[contenteditable="true"]');
      if (e.ctrlKey && e.key.toLowerCase() === 'f') { e.preventDefault(); this.setOverviewExpanded(false); this.filter.focus(); this.filter.select(); return; }
      if (e.key === 'Escape' && this.overviewExpanded && !editing && !this.diagnosticOpen) { e.preventDefault(); this.setOverviewExpanded(false); return; }
      if (!this.view) return;
      if (e.ctrlKey && ['+', '=', '-', '0'].includes(e.key)) {
        e.preventDefault(); const zoom = e.key === '0' ? 100 : Math.max(50, Math.min(200, this.view.settings.ui_zoom + (e.key === '-' ? -25 : 25))); this.send({ type: 'set_ui_zoom', value: zoom }); return;
      }
      if (editing) return;
      if (['+', '-', '1', '2', '3', 'F2', 'F3', 'F4', 'F5', 'F8'].includes(e.key)) this.setOverviewExpanded(false);
      if (e.key === '+' || e.key === '-') { e.preventDefault(); this.history.zoom(e.key === '+'); }
      if (['1', '2', '3'].includes(e.key)) { e.preventDefault(); this.send({ type: 'set_history_range', seconds: [300, 900, 1800][Number(e.key) - 1] }); }
      if (e.key === 'F2') { e.preventDefault(); const groups: Grouping[] = ['name', 'path', 'tree', 'process', 'family']; this.send({ type: 'set_grouping', value: groups[(groups.indexOf(this.view.settings.grouping) + 1) % groups.length] }); }
      if (e.key === 'F3') { e.preventDefault(); this.send({ type: 'sort_by', column: this.view.settings.sort.column === 'change' ? 'private_working_set' : 'change' }); }
      if (e.key === 'F4') { e.preventDefault(); this.send({ type: 'set_show_exited', value: !this.view.settings.show_exited }); }
      if (e.key === 'F5') { e.preventDefault(); this.send({ type: 'set_metric', value: this.view.settings.metric === 'private_commit' ? 'private_working_set' : 'private_commit' }); }
      if (e.key === 'F8') { e.preventDefault(); this.send({ type: 'select_process_parent' }); }
    });
    this.element.addEventListener('wheel', e => { if (e.ctrlKey && this.view) { e.preventDefault(); this.send({ type: 'set_ui_zoom', value: Math.max(50, Math.min(200, this.view.settings.ui_zoom + (e.deltaY < 0 ? 25 : -25))) }); } }, { passive: false });
  }
}
