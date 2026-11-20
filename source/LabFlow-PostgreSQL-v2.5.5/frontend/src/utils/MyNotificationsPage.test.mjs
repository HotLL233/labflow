import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

// Execute the page's real callbacks and effect cleanup, as the existing record performance tests do.
const source = ts.createSourceFile('MyNotificationsPage.tsx', readFileSync(new URL('../pages/MyNotificationsPage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function code(name) {
  let found;
  const visit = node => {
    if (name === 'effect' && ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect' && node.arguments[0].getText(source).includes('mounted.current = true')) found = node.arguments[0];
    if (name === 'refreshEffect' && ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect' && node.arguments[0].getText(source).includes('INBOX_CHANGED')) found = node.arguments[0];
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) {
      found = ts.isCallExpression(node.initializer) ? node.initializer.arguments[0] : node.initializer;
    }
    if (!found) ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(found, `Missing page callback ${name}`);
  return found.getText(source);
}
function bind(name, env) {
  const compiled = ts.transpileModule(`const callback = (${code(name)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(env), `${compiled}\nreturn callback;`)(...Object.values(env));
}
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const ok = data => ({ code: 0, data });
const row = id => ({ id, is_read: false, target_url: `/sample-records?record_id=${id}` });
const result = (items, total = items.length, unread_count = total) => ok({ items, total, unread_count, page: 1, page_size: 50 });
const flush = () => new Promise(resolve => setImmediate(resolve));
function harness() {
  const state = { items: [row(1)], unread: 4, total: 4, page: 0, error: '', loading: false, markingAll: false, openingId: null, navigations: [] };
  const env = {
    session: 'account-a:token-a', activeSession: { current: 'account-a:token-a' }, mounted: { current: true },
    loadRequest: { current: 0 }, actionPending: { current: false }, loadedFor: { current: 'account-a:token-a' }, loading: false,
    hasPermission: () => true, notificationTarget: item => item.target_url,
    navigate: target => state.navigations.push(target),
    page: 0, pageSize: 50, onlyUnread: false, keyword: '',
    getNotificationInboxPage: async () => result([row(2)], 1, 0),
    markNotificationRead: async () => ok(null), markAllNotificationsRead: async () => ok(null),
  };
  for (const field of ['items', 'unread', 'total', 'page', 'error', 'loading', 'markingAll', 'openingId']) {
    env['set' + field[0].toUpperCase() + field.slice(1)] = value => { state[field] = typeof value === 'function' ? value(state[field]) : value; };
  }
  const callbacks = () => {
    const load = bind('load', env);
    return { load, openItem: bind('openItem', env), readAll: bind('readAll', { ...env, load }) };
  };
  return { state, env, callbacks };
}

test('later paged inbox loads retain their own rows, total and unread count when earlier responses finish late', async () => {
  for (const laterQuery of [{ page: 1 }, { onlyUnread: true, keyword: '旧记录' }]) {
    const h = harness(), lists = [], calls = [];
    h.env.getNotificationInboxPage = query => { calls.push(query); const task = deferred(); lists.push(task); return task.promise; };
    const { load } = h.callbacks(), oldLoad = load();
    Object.assign(h.env, laterQuery);
    const currentLoad = h.callbacks().load();
    lists[1].resolve(result([row(22)], 102, 2)); await currentLoad;
    lists[0].resolve(result([row(11)], 111, 111));
    await oldLoad;
    assert.deepEqual(h.state.items, [row(22)]); assert.equal(h.state.unread, 2);
    assert.equal(h.state.total, 102);
    assert.deepEqual(calls[1], { page: h.env.page + 1, page_size: 50, only_unread: h.env.onlyUnread, keyword: h.env.keyword });
    assert.equal(h.state.loading, false); assert.equal(h.state.error, '');
  }
});

test('account changes and unmount cleanup stop old loads/read actions from updating or navigating', async () => {
  for (const leave of ['account', 'unmount']) for (const action of ['load', 'openItem', 'readAll']) {
    const h = harness(), pending = deferred(); let refreshes = 0;
    const cleanup = bind('effect', { ...h.env, load: () => {} })();
    h.state.items = [row(1)]; h.state.unread = 4;
    if (action === 'load') h.env.getNotificationInboxPage = () => pending.promise;
    if (action === 'openItem') h.env.markNotificationRead = () => pending.promise;
    if (action === 'readAll') h.env.markAllNotificationsRead = () => pending.promise;
    const callbacks = h.callbacks();
    const oldTask = action === 'openItem' ? callbacks.openItem(row(1)) : callbacks[action]();
    await flush(); cleanup();
    if (leave === 'account') {
      h.env.activeSession.current = 'account-b:token-b';
      bind('effect', { ...h.env, session: 'account-b:token-b', load: () => { refreshes++; } })();
    }
    h.state.items = [row(99)]; h.state.unread = 9; h.state.error = 'current session';
    pending.resolve(action === 'load' ? result([row(1)]) : ok(null));
    await oldTask;
    assert.deepEqual(h.state.items, [row(99)]); assert.equal(h.state.unread, 9);
    assert.equal(h.state.error, 'current session'); assert.deepEqual(h.state.navigations, []);
    assert.equal(refreshes, leave === 'account' ? 1 : 0);
  }
});

test('same-frame double clicks issue one read PUT and share the lock across open and read-all actions', async () => {
  for (const firstAction of ['openItem', 'readAll']) {
    const h = harness(), pending = deferred(); let puts = 0, refreshes = 0;
    h.env.markNotificationRead = h.env.markAllNotificationsRead = () => { puts++; return pending.promise; };
    const openItem = bind('openItem', h.env), readAll = bind('readAll', { ...h.env, load: async () => { refreshes++; } });
    const first = firstAction === 'openItem' ? openItem(row(1)) : readAll();
    await openItem(row(1)); await readAll();
    assert.equal(puts, 1);
    pending.resolve(ok(null)); await first;
    assert.equal(h.env.actionPending.current, false);
    if (firstAction === 'openItem') {
      assert.equal(h.state.items[0].is_read, true); assert.equal(h.state.unread, 3);
      assert.deepEqual(h.state.navigations, ['/sample-records?record_id=1']); assert.equal(refreshes, 0);
    } else { assert.equal(refreshes, 1); assert.deepEqual(h.state.navigations, []); }
  }
});

test('read-all failures preserve unread state; success refreshes server state instead of clearing optimistically', async () => {
  for (const failure of ['business', 'network']) {
    const h = harness(); let refreshes = 0;
    h.env.markAllNotificationsRead = async () => { if (failure === 'network') throw Error('offline'); return { code: 1, message: 'denied' }; };
    await bind('readAll', { ...h.env, load: () => { refreshes++; } })();
    assert.equal(refreshes, 0); assert.deepEqual(h.state.items, [row(1)]); assert.equal(h.state.unread, 4);
    assert.match(h.state.error, failure === 'network' ? /offline/ : /denied/);
    assert.equal(h.state.markingAll, false); assert.equal(h.env.actionPending.current, false);
  }
  const h = harness(), list = deferred(); let listRequests = 0;
  h.env.getNotificationInboxPage = () => { listRequests++; return list.promise; };
  const task = h.callbacks().readAll(); await flush();
  assert.equal(listRequests, 1); assert.deepEqual(h.state.items, [row(1)]); assert.equal(h.state.unread, 4);
  list.resolve(result([{ ...row(1), is_read: true }], 1, 0)); await task;
  assert.equal(h.state.items[0].is_read, true); assert.equal(h.state.unread, 0);
  assert.equal(h.state.markingAll, false); assert.equal(h.env.actionPending.current, false);
});

test('inbox page moves back to the last valid page after filtering or marking the last page read', async () => {
  const h = harness();
  h.env.page = 5;
  h.env.getNotificationInboxPage = async () => result([], 4, 4);
  await h.callbacks().load();
  assert.equal(h.state.page, 0);
  assert.deepEqual(h.state.items, [row(1)], 'do not publish an invalid empty page as the result');
});

test('inbox refreshes on change, focus and restored visibility, skips hidden/busy work, and cleans listeners', () => {
  const h = harness(), listeners = new Map(), removed = [];
  let refreshes = 0, intervalCallback, cleared = false;
  const environment = { ...h.env, INBOX_CHANGED: 'inbox-change', load: () => { refreshes++; },
    document: { hidden: false, addEventListener: (name, callback) => listeners.set(name, callback), removeEventListener: name => removed.push(name) },
    window: { addEventListener: (name, callback) => listeners.set(name, callback), removeEventListener: name => removed.push(name),
      setInterval: callback => { intervalCallback = callback; return 7; }, clearInterval: id => { assert.equal(id, 7); cleared = true; } },
  };
  const cleanup = bind('refreshEffect', environment)();
  for (const name of ['inbox-change', 'focus', 'visibilitychange']) listeners.get(name)();
  assert.equal(refreshes, 3);
  environment.document.hidden = true; intervalCallback(); assert.equal(refreshes, 3);
  environment.document.hidden = false; h.env.actionPending.current = true; listeners.get('inbox-change')(); assert.equal(refreshes, 3);
  h.env.actionPending.current = false; listeners.get('focus')(); assert.equal(refreshes, 4);
  cleanup();
  assert.deepEqual(removed.sort(), ['focus', 'inbox-change', 'visibilitychange']);
  assert.equal(cleared, true);
});
