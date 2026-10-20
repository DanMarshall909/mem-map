export function el<K extends keyof HTMLElementTagNameMap>(tag: K, className = '', text = ''): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag); node.className = className; if (text) node.textContent = text; return node;
}
export function text(node: Node, value: string): void { if (node.textContent !== value) node.textContent = value; }
export function button(label: string, action: () => void, className = ''): HTMLButtonElement {
  const node = el('button', className, label); node.type = 'button'; node.addEventListener('click', action); return node;
}
const icons = {
  pause: 'M8 5v14M16 5v14', play: 'm8 5 11 7-11 7Z',
  export: 'M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5',
  expand: 'M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6',
  back: 'm10 5-7 7 7 7M3 12h18', left: 'm15 5-7 7 7 7', right: 'm9 5 7 7-7 7',
  info: 'M12 11v6m0-10v.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
  close: 'm6 6 12 12M6 18 18 6', copy: 'M9 9h12v12H9ZM15 9V3H3v12h6',
  list: 'M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01',
  reset: 'M3 10a9 9 0 1 1 2 8M3 3v7h7',
} as const;
export function setIcon(node: HTMLButtonElement, label: string, icon: keyof typeof icons): void {
  node.setAttribute('aria-label', label); node.title = label;
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [name, value] of Object.entries({ viewBox: '0 0 24 24', 'aria-hidden': 'true', fill: 'none', stroke: 'currentColor', 'stroke-width': '1.8', 'stroke-linecap': 'round', 'stroke-linejoin': 'round' })) svg.setAttribute(name, value);
  const path = document.createElementNS(svg.namespaceURI, 'path'); path.setAttribute('d', icons[icon]); svg.append(path); node.replaceChildren(svg);
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
export function radios<T extends string>(label: string, choices: [T, string][], action: (value: T) => void): { group: HTMLFieldSetElement; set: (value: T) => void } {
  const group = el('fieldset', 'segments'); const legend = el('legend', '', label); group.append(legend);
  const name = `choice-${++serial}`; const entries = choices.map(([value, caption]) => {
    const wrapper = el('label'); const input = el('input'); input.type = 'radio'; input.name = name; input.value = value;
    input.addEventListener('change', () => { if (input.checked) action(value); });
    wrapper.append(input, el('span', '', caption)); group.append(wrapper); return input;
  });
  return { group, set: value => { for (const input of entries) input.checked = input.value === value; } };
}
