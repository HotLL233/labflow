import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

function action(file, name, environment) {
  const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let callback;
  const visit = node => {
    if (name === 'archiveEffect' && ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect' && node.arguments[0].getText(source).includes('getLogArchiveEntries')) callback = node.arguments[0];
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) callback = ts.isCallExpression(node.initializer) ? node.initializer.arguments[0] : node.initializer;
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(callback, name);
  const compiled = ts.transpileModule(`const callback = (${callback.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}\nreturn callback;`)(...Object.values(environment));
}
const flush = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };

test('archive download and search reuse authenticated APIs and preserve server pagination/date filters', async () => {
  const calls = [];
  const client = { get: async (...args) => { calls.push(args); return { data: { code: 0 } }; } };
  const query = { page: 3, page_size: 20, keyword: '送样', start_date: '2026-10-01', end_date: '2026-10-31' };
  await action('../api/admin.ts', 'getLogArchiveEntries', { client })(17, query);
  assert.deepEqual(calls, [['/log-maintenance/archives/17/entries', { params: query }]]);
  const downloads = [];
  await action('../api/admin.ts', 'downloadLogArchive', { downloadFile: async (...args) => downloads.push(args) })(17);
  assert.deepEqual(downloads, [['/api/log-maintenance/archives/17/download', {}, 'labflow-log-archive-17.zip']]);
  calls.length = 0;
  const batches = { page: 5, page_size: 50, keyword: 'audit' };
  await action('../api/admin.ts', 'getLogArchivePage', { client })(batches);
  assert.deepEqual(calls, [['/log-maintenance/archives/page', { params: batches }]]);
});

test('archive search rejects inverted dates and resets the page on a valid full-history query', () => {
  const filters = [], pages = [], errors = [];
  const environment = { archiveDraft: { keyword: '  业务号  ', start_date: '2026-10-01', end_date: '2026-10-31' },
    setArchiveFilters: value => filters.push(value), setArchivePage: value => pages.push(value), setArchiveError: value => errors.push(value) };
  action('./LogMaintenancePanel.tsx', 'queryArchive', { ...environment, archiveDraft: { ...environment.archiveDraft, end_date: '2026-09-01' } })();
  assert.equal(filters.length, 0); assert.equal(pages.length, 0); assert.equal(errors.length, 1);
  action('./LogMaintenancePanel.tsx', 'queryArchive', environment)();
  assert.deepEqual(filters, [{ keyword: '业务号', start_date: '2026-10-01', end_date: '2026-10-31' }]);
  assert.deepEqual(pages, [0]);
});

test('archive result responses cannot replace a newer archive or update a closed dialog', async () => {
  const request = { current: 0 }, pending = [], queries = [], writes = [];
  const environment = {
    archiveRequest: request, archiveTarget: { id: 17 }, archivePage: 0, archivePageSize: 50, archiveFilters: { keyword: '', start_date: '', end_date: '' },
    getLogArchiveEntries: (id, query) => { const task = deferred(); pending.push(task); queries.push([id, query]); return task.promise; },
    setArchiveLoading: () => {}, setArchiveError: value => writes.push(['error', value]), setArchivePage: () => {},
    setArchiveRows: value => writes.push(['rows', value]), setArchiveTotal: value => writes.push(['total', value]),
  };
  const cleanup = action('./LogMaintenancePanel.tsx', 'archiveEffect', environment)();
  cleanup();
  const cleanupNext = action('./LogMaintenancePanel.tsx', 'archiveEffect', { ...environment, archiveTarget: { id: 18 } })();
  pending[1].resolve({ code: 0, data: { items: [{ id: 2 }], total: 1 } }); await flush();
  pending[0].resolve({ code: 0, data: { items: [{ id: 1 }], total: 99 } }); await flush();
  assert.deepEqual(writes.filter(([key]) => key === 'rows'), [['rows', [{ id: 2 }]]]);
  assert.deepEqual(queries.map(([id]) => id), [17, 18]);
  cleanupNext();
});

test('online audit forwards notification module and dates and keeps a later query when an older one finishes last', async () => {
  const pending = [], queries = [], state = { rows: [], total: 0 }, request = { current: 0 };
  const environment = {
    auditLoadRequest: request, auditFilters: { module: 'notifications', action: 'retry', user_name: ' admin ', business_no: ' RD001 ', start_date: '2026-10-01', end_date: '2026-10-31' },
    getAuditLogs: query => { queries.push(query); const task = deferred(); pending.push(task); return task.promise; },
    setAl: value => { state.rows = value; }, setAt: value => { state.total = value; }, setAp: () => {}, showMessage: () => {},
  };
  const load = action('../pages/ManagePage.tsx', 'la', environment);
  const first = load(1), second = load(2);
  pending[1].resolve({ code: 0, data: { items: [{ id: 2 }], total: 101 } }); await second;
  pending[0].resolve({ code: 0, data: { items: [{ id: 1 }], total: 1 } }); await first;
  assert.deepEqual(state.rows, [{ id: 2 }]); assert.equal(state.total, 101);
  assert.deepEqual(queries[0], { page: 1, page_size: 50, module: 'notifications', action: 'retry', user_name: 'admin', business_no: 'RD001', start_date: '2026-10-01', end_date: '2026-10-31' });
});

test('archive batches beyond the old 50/200 windows load by server page and reject late previous queries', async () => {
  const pending = [], queries = [], request = { current: 0 }, state = {};
  const environment = {
    batchRequest: request, batchPage: 4, batchPageSize: 50, batchKeyword: '',
    getLogArchivePage: query => { queries.push(query); const task = deferred(); pending.push(task); return task.promise; },
    setBatchLoading: value => { state.loading = value; }, setBatchError: value => { state.error = value; },
    setBatchPage: value => { state.page = value; }, setArchiveBatches: value => { state.items = value; }, setBatchTotal: value => { state.total = value; },
  };
  const load = action('./LogMaintenancePanel.tsx', 'loadBatches', environment);
  const first = load();
  const rows = Array.from({ length: 25 }, (_, index) => ({ id: index + 201 }));
  pending[0].resolve({ code: 0, data: { items: rows, total: 225 } }); await first;
  assert.deepEqual(queries[0], { page: 5, page_size: 50, keyword: undefined });
  assert.equal(state.items.length, 25); assert.equal(state.total, 225);
  const oldLoad = load();
  const filteredLoad = action('./LogMaintenancePanel.tsx', 'loadBatches', { ...environment, batchPage: 0, batchKeyword: 'audit' })();
  pending[2].resolve({ code: 0, data: { items: [{ id: 1 }], total: 1 } }); await filteredLoad;
  pending[1].resolve({ code: 0, data: { items: rows, total: 225 } }); await oldLoad;
  assert.deepEqual(state.items, [{ id: 1 }]); assert.equal(state.total, 1);
  assert.equal(state.loading, false); assert.equal(state.error, '');
});

test('archive batch loading corrects pages after expiry and reports query errors without a false empty success', async () => {
  const request = { current: 0 }, state = { items: [{ id: 17 }] };
  const environment = {
    batchRequest: request, batchPage: 4, batchPageSize: 50, batchKeyword: '',
    getLogArchivePage: async () => ({ code: 0, data: { items: [], total: 40 } }),
    setBatchLoading: value => { state.loading = value; }, setBatchError: value => { state.error = value; },
    setBatchPage: value => { state.page = value; }, setArchiveBatches: value => { state.items = value; }, setBatchTotal: value => { state.total = value; },
  };
  await action('./LogMaintenancePanel.tsx', 'loadBatches', environment)();
  assert.equal(state.page, 0); assert.deepEqual(state.items, [{ id: 17 }]);
  await action('./LogMaintenancePanel.tsx', 'loadBatches', { ...environment, getLogArchivePage: async () => ({ code: 1, message: '归档不可用' }) })();
  assert.deepEqual(state.items, []); assert.equal(state.error, '归档不可用'); assert.equal(state.loading, false);
});
