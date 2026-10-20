import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ViewClient } from '../src/transport.ts';

test('an old poll cannot replace a newer action response', async () => {
  let resolvePoll!: (v: { revision: number; value: string }) => void;
  const rendered: string[] = [];
  const client = new ViewClient({
    getView: () => new Promise(r => { resolvePoll = r; }),
    dispatch: async (_a: string) => ({ revision: 4, value: 'selected new process' }),
    copyPaths: async () => '', copySnapshot: async () => '',
  }, v => rendered.push(v.value), e => assert.fail(e));
  const old = client.refresh();
  await client.send('select');
  resolvePoll({ revision: 3, value: 'old process' });
  await old;
  assert.deepEqual(rendered, ['selected new process']);
});

test('equal-revision collector health is refreshed without a new snapshot', async () => {
  let age = 0; const seen: number[] = [];
  const client = new ViewClient({ getView: async () => ({ revision: 1, age: age++ }), dispatch: async (_a: string) => ({ revision: 1, age: 0 }), copyPaths: async () => '', copySnapshot: async () => '' }, view => seen.push(view.age), e => assert.fail(e));
  await client.refresh(); await client.refresh(); assert.deepEqual(seen, [0, 1]);
});

test('actions and copy run in order, and a rejected action does not strand the queue', async () => {
  const calls: string[] = [];
  const renderedCauses: (string | undefined)[] = [];
  const errors: string[] = [];
  let finish!: () => void;
  let revision = 0;
  const client = new ViewClient({
    getView: async () => ({ revision }),
    dispatch: async (a: string) => {
      calls.push(a);
      if (a === 'select') await new Promise<void>(r => { finish = r; });
      if (a === 'bad') throw new Error('Selection is unavailable');
      return { revision: ++revision };
    },
    copyPaths: async () => { calls.push('copy selected'); return 'Path copied'; },
    copySnapshot: async () => '',
  }, (_view, action) => renderedCauses.push(action), e => errors.push(e));
  const first = client.send('select');
  const copy = client.native('copy');
  const bad = client.send('bad');
  const last = client.send('last');
  await new Promise(r => setImmediate(r));
  assert.deepEqual(calls, ['select']);
  finish();
  await Promise.all([first, copy, bad, last]);
  assert.deepEqual(calls, ['select', 'copy selected', 'bad', 'last']);
  assert.match(errors[0], /Selection is unavailable/);
  assert.equal(await copy, 'Path copied');
  await client.refresh();
  assert.deepEqual(renderedCauses, ['select', 'last', undefined], 'Only accepted action responses carry a scroll-effect cause; polls and rejected actions do not');
});

test('overlapping polls are bounded and collection IPC errors allow a later recovery', async () => {
  let complete!: (view: { revision: number }) => void; let count = 0; const errors: string[] = []; const rendered: number[] = [];
  const client = new ViewClient({ getView: async () => {
    count++; if (count === 1) return new Promise<{ revision: number }>(resolve => { complete = resolve; });
    if (count === 2) throw new Error('Collector IPC disconnected'); return { revision: 2 };
  }, dispatch: async (_a: string) => ({ revision: 1 }), copyPaths: async () => '', copySnapshot: async () => 'Copied snapshot as CSV' }, view => rendered.push(view.revision), message => errors.push(message));
  const first = client.refresh(); await client.refresh(); assert.equal(count, 1); complete({ revision: 1 }); await first;
  await client.refresh(); assert.match(errors[0], /Collector IPC disconnected/);
  await client.refresh(); assert.deepEqual(rendered, [1, 2]); assert.equal(await client.native('snapshot'), 'Copied snapshot as CSV');
});


test('a poll finishing during an action cannot reset the pending control selection', async () => {
  type Frame = { revision: number; metric: string };
  let pollDone!: (view: Frame) => void, actionDone!: (view: Frame) => void;
  const seen: string[] = [];
  const client = new ViewClient({ getView: () => new Promise<Frame>(r => { pollDone = r; }),
    dispatch: (_action: string) => new Promise<Frame>(r => { actionDone = r; }), copyPaths: async () => '', copySnapshot: async () => '' },
    view => seen.push(view.metric), error => assert.fail(error));
  const poll = client.refresh(), action = client.send('commit');
  await new Promise(resolve => setImmediate(resolve));
  pollDone({ revision: 1, metric: 'working set' }); await poll;
  assert.deepEqual(seen, [], 'Pending actions suppress stale poll renders');
  actionDone({ revision: 2, metric: 'commit' }); await action;
  assert.deepEqual(seen, ['commit']);
});

test('derived actions recover from failed reads or dispatches using fresh authoritative state', async () => {
  for (const failure of ['read', 'dispatch']) {
    let current = { revision: 1, scale: 100 }, failed = false;
    const errors: string[] = [], seen: number[] = [], sent: number[] = [];
    const client = new ViewClient({
      getView: async () => {
        if (failure === 'read' && !failed) { failed = true; current = { revision: 2, scale: 125 }; throw Error('read rejected'); }
        return { ...current };
      },
      dispatch: async (scale: number) => {
        sent.push(scale);
        if (failure === 'dispatch' && !failed) { failed = true; current = { revision: 2, scale: 125 }; throw Error('dispatch rejected'); }
        current = { revision: current.revision + 1, scale }; return { ...current };
      }, copyPaths: async () => '', copySnapshot: async () => '',
    }, view => seen.push(view.scale), error => errors.push(error));
    await Promise.all([client.sendDerived(view => view.scale + 25), client.sendDerived(view => view.scale + 25)]);
    assert.deepEqual(sent, failure === 'read' ? [150] : [125, 150]);
    assert.deepEqual(seen, [150]); assert.equal(errors.length, 1);
    assert.match(errors[0], /rejected/); assert.equal(current.scale, 150);
  }
});
