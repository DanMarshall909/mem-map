export function el<K extends keyof HTMLElementTagNameMap>(tag: K, className = '', text = ''): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag); node.className = className; if (text) node.textContent = text; return node;
}
export function text(node: Node, value: string): void { if (node.textContent !== value) node.textContent = value; }
export function button(label: string, action: () => void, className = ''): HTMLButtonElement {
  const node = el('button', className, label); node.type = 'button'; node.addEventListener('click', action); return node;
}
const icons = {
  pause: 'M8 5v14M16 5v14', play: 'm8 5 11 7-11 7Z',
  tag: 'm3 3 9 0 9 9-9 9-9-9ZM7 7h.01',
  folder: 'M3 7V4h7l3 3h8v13H3Z',
  tree: 'M10 3h4v4h-4ZM3 17h4v4H3Zm14 0h4v4h-4ZM12 7v5M5 17v-5h14v5',
  process: 'M6 6h12v12H6ZM9 9h6v6H9ZM9 2v4m6-4v4M9 18v4m6-4v4M2 9h4m-4 6h4m12-6h4m-4 6h4',
  family: 'M12 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM4 16a2 2 0 1 0 0-4 2 2 0 0 0 0 4Zm16 0a2 2 0 1 0 0-4 2 2 0 0 0 0 4ZM7 21v-4a5 5 0 0 1 10 0v4M1 21v-2a3 3 0 0 1 6 0m10 0a3 3 0 0 1 6 0v2',
  expand: 'M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6',
  back: 'm10 5-7 7 7 7M3 12h18', left: 'm15 5-7 7 7 7', right: 'm9 5 7 7-7 7',
  info: 'M12 11v6m0-10v.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
  close: 'm6 6 12 12M6 18 18 6', copy: 'M9 9h12v12H9ZM15 9V3H3v12h6',
  list: 'M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01',
  reset: 'M3 10a9 9 0 1 1 2 8M3 3v7h7',
} as const;
function createIcon(icon: keyof typeof icons): SVGSVGElement {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [name, value] of Object.entries({ viewBox: '0 0 24 24', 'aria-hidden': 'true', fill: 'none', stroke: 'currentColor', 'stroke-width': '1.8', 'stroke-linecap': 'round', 'stroke-linejoin': 'round' })) svg.setAttribute(name, value);
  const path = document.createElementNS(svg.namespaceURI, 'path'); path.setAttribute('d', icons[icon]); svg.append(path); return svg;
}
export function setIcon(node: HTMLButtonElement, label: string, icon: keyof typeof icons): void {
  node.setAttribute('aria-label', label); node.title = label; node.replaceChildren(createIcon(icon));
}
export function iconButton(label: string, icon: keyof typeof icons, action: () => void, className = ''): HTMLButtonElement {
  const node = button('', action, `icon-button ${className}`); setIcon(node, label, icon); return node;
}
export function checkbox(label: string, action: (value: boolean) => void): { label: HTMLLabelElement; input: HTMLInputElement } {
  const node = el('label', 'check'); const input = el('input'); input.type = 'checkbox';
  input.addEventListener('change', () => action(input.checked)); node.append(input, document.createTextNode(label));
  return { label: node, input };
}
export function select(label: string, choices: [string, string][], action: (value: string) => void): { label: HTMLLabelElement; input: HTMLSelectElement } {
  const node = el('label', 'field'); const caption = el('span', '', label); const input = el('select');
  input.setAttribute('aria-label', label.replace(/:$/, ''));
  for (const [value, name] of choices) { const option = el('option', '', name); option.value = value; input.append(option); }
  input.addEventListener('change', () => action(input.value)); node.append(caption, input); return { label: node, input };
}
let serial = 0;
export function radios<T extends string>(label: string, choices: [T, string][], action: (value: T) => void, iconNames?: Partial<Record<T, keyof typeof icons>>): { group: HTMLFieldSetElement; set: (value: T) => void } {
  const group = el('fieldset', 'segments'); const legend = el('legend', '', label); group.append(legend);
  const name = `choice-${++serial}`; const entries = choices.map(([value, caption]) => {
    const wrapper = el('label'); const input = el('input'); input.type = 'radio'; input.name = name; input.value = value;
    input.addEventListener('change', () => { if (input.checked) action(value); });
    const icon = iconNames?.[value], display = el('span', icon ? 'radio-icon' : '', icon ? '' : caption);
    if (icon) { input.setAttribute('aria-label', caption); wrapper.title = caption; display.append(createIcon(icon)); }
    wrapper.append(input, display); group.append(wrapper); return input;
  });
  return { group, set: value => { for (const input of entries) input.checked = input.value === value; } };
}
