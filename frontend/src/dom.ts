export function el<K extends keyof HTMLElementTagNameMap>(tag: K, className = '', text = ''): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag); node.className = className; if (text) node.textContent = text; return node;
}
export function text(node: Node, value: string): void { if (node.textContent !== value) node.textContent = value; }
export function button(label: string, action: () => void, className = ''): HTMLButtonElement {
  const node = el('button', className, label); node.type = 'button'; node.addEventListener('click', action); return node;
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
