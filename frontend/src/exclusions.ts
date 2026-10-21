import type { Action, Settings } from './contract.ts';
import { button, checkbox, el, iconButton, radios, text } from './dom.ts';

/** Session-only global rules; the backend remains the source of truth. */
export class ExclusionControls {
  readonly dialog = el('dialog', 'diagnostics exclusion-dialog');
  readonly open: HTMLButtonElement;
  private input = el('input');
  private rules = el('div', 'exclusion-rules');
  private kind = 'process';
  private lastRules = '';
  private ram: ReturnType<typeof checkbox>;
  private other: ReturnType<typeof checkbox>;
  private show: ReturnType<typeof checkbox>;
  constructor(send: (action: Action) => void) {
    this.open = iconButton('Global exclusions', 'filter', () => { this.dialog.showModal(); this.input.focus(); }, 'accent');
    this.open.setAttribute('aria-haspopup', 'dialog'); this.open.setAttribute('aria-controls', 'global-exclusions');
    this.dialog.id = 'global-exclusions'; this.dialog.setAttribute('aria-label', 'Global exclusions');
    const close = () => { this.dialog.close(); this.open.focus(); };
    this.dialog.addEventListener('cancel', event => { event.preventDefault(); close(); });
    const heading = el('div', 'diagnostic-heading'); heading.append(el('h2', '', 'Global exclusions'), iconButton('Close exclusions', 'close', close));
    const content = el('div', 'diagnostic-content');
    const scopes = radios('Exclude by', [['process', 'Process name'], ['tree', 'Process tree root name'], ['path', 'Executable path']], value => { this.kind = value; }, { process: 'process', tree: 'tree', path: 'folder' }); scopes.set('process');
    this.input.type = 'search'; this.input.maxLength = 2048; this.input.required = true; this.input.setAttribute('aria-label', 'Exclusion text'); this.input.placeholder = 'Name or path contains…';
    const form = el('form', 'exclusion-form');
    const add = iconButton('Add exclusion', 'add', () => {}); add.type = 'submit';
    form.addEventListener('submit', event => { event.preventDefault(); if (!this.input.value.trim()) return; send({ type: 'add_exclusion', kind: this.kind, value: this.input.value.trim() }); this.input.value = ''; this.input.focus(); });
    form.append(scopes.group, this.input, add);
    this.ram = checkbox('Exclude Other used RAM', value => send({ type: 'set_exclude_other_ram', value }), 'ram');
    this.other = checkbox('Exclude Other processes', value => send({ type: 'set_exclude_other_processes', value }), 'process');
    this.show = checkbox('Show excluded memory as a band', value => send({ type: 'set_show_excluded', value }), 'list');
    for (const [control, label] of [[this.ram, 'Other used RAM'], [this.other, 'Other processes'], [this.show, 'Show excluded memory as a band']] as const) control.label.append(document.createTextNode(label));
    this.rules.setAttribute('aria-label', 'Active exclusion rules');
    const clear = button('Clear all exclusions', () => send({ type: 'clear_exclusions' }));
    content.append(el('p', '', 'Case-insensitive text matching. Tree rules include descendants. Collection continues; top-bar totals and inspector values include hidden processes.'), form, this.rules, this.ram.label, this.other.label, this.show.label, clear);
    this.dialog.append(heading, content);
    this.rules.addEventListener('click', event => {
      const remove = (event.target as Element).closest<HTMLButtonElement>('button[data-kind]');
      if (remove) { send({ type: 'remove_exclusion', kind: remove.dataset.kind!, value: remove.dataset.value! }); this.input.focus(); }
    });
  }
  update(settings: Settings): void {
    const state = settings.exclusions ?? { rules: [], other_ram: false, other_processes: false, show_excluded: true };
    this.ram.input.checked = state.other_ram; this.other.input.checked = state.other_processes; this.show.input.checked = state.show_excluded;
    const count = state.rules.length + Number(state.other_ram) + Number(state.other_processes);
    this.open.title = count ? `Global exclusions (${count} active)` : 'Global exclusions';
    this.open.setAttribute('aria-description', `${count} active exclusions`);
    this.open.classList.toggle('filter-active', count > 0);
    const key = JSON.stringify(state.rules); if (key === this.lastRules) return; this.lastRules = key;
    this.rules.replaceChildren();
    if (!state.rules.length) this.rules.append(el('p', '', 'No process or path exclusions'));
    for (const rule of state.rules) {
      const row = el('div', 'exclusion-rule'); const label = el('span'); text(label, `${rule.kind === 'tree' ? 'Tree' : rule.kind === 'path' ? 'Path' : 'Process'}: ${rule.value}`);
      const remove = iconButton(`Remove ${rule.kind} exclusion: ${rule.value}`, 'close', () => {});
      remove.dataset.kind = rule.kind; remove.dataset.value = rule.value; row.append(label, remove); this.rules.append(row);
    }
  }
}
