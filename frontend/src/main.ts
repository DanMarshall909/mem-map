import { invoke } from '@tauri-apps/api/core';
import type { Action, View } from './contract.ts';
import { Dashboard } from './dashboard.ts';
import { ViewClient } from './transport.ts';
import './styles.css';

const mount = document.querySelector<HTMLElement>('#app');
if (!mount) throw new Error('Application mount is missing');
let client: ViewClient<View, Action>;
const dashboard = new Dashboard(action => { void client.send(action); }, action => {
  void client.native(action).then(notice => { if (notice) dashboard.showNotice(notice); });
});
client = new ViewClient<View, Action>({
  getView: () => invoke<View>('get_view'),
  dispatch: action => invoke<View>('dispatch', { action }),
  copyPaths: () => invoke<string>('copy_paths'),
  copySnapshot: () => invoke<string>('copy_snapshot'),
}, (view, action) => dashboard.update(view, action?.type === 'show_selected_processes' || action?.type === 'select_process_parent' || action?.type === 'open_overview_process'), error => dashboard.showNotice(error));
mount.append(dashboard.element);
void client.refresh();
const polling = setInterval(() => { void client.refresh(); }, 500);
window.addEventListener('pagehide', () => clearInterval(polling), { once: true });
