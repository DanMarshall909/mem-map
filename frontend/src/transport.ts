export interface Bridge<V, A> {
  getView(): Promise<V>;
  dispatch(action: A): Promise<V>;
  copyPaths(): Promise<string>;
  copySnapshot(): Promise<string>;
}

/** Owns IPC ordering; DOM consumers receive complete, increasing revisions. */
export class ViewClient<V extends { revision: number }, A> {
  private bridge: Bridge<V, A>;
  private render: (view: V, action?: A) => void;
  private error: (message: string) => void;
  constructor(bridge: Bridge<V, A>, render: (view: V, action?: A) => void, error: (message: string) => void) {
    this.bridge = bridge; this.render = render; this.error = error;
  }
  private revision = -1;
  private queue: Promise<unknown> = Promise.resolve();
  private pending = 0;
  private polling = false;
  private apply(view: V, action?: A): void {
    // Health can change without a collection revision, so equal revisions are valid.
    if (view.revision < this.revision) return;
    this.revision = view.revision;
    this.render(view, action);
  }
  async refresh(): Promise<void> {
    if (this.polling || this.pending) return;
    this.polling = true;
    try { this.apply(await this.bridge.getView()); }
    catch (e) { this.error(String(e)); }
    finally { this.polling = false; }
  }
  private enqueue<T>(work: () => Promise<T>): Promise<T | undefined> {
    this.pending++;
    const next = this.queue.then(work).catch(e => { this.error(String(e)); return undefined; }).finally(() => { this.pending--; });
    this.queue = next;
    return next;
  }
  async send(action: A): Promise<void> {
    await this.enqueue(async () => { this.apply(await this.bridge.dispatch(action), action); });
  }
  async native(action: 'copy' | 'snapshot'): Promise<string | undefined> {
    return this.enqueue(async () => action === 'copy' ? this.bridge.copyPaths() : this.bridge.copySnapshot());
  }
}
