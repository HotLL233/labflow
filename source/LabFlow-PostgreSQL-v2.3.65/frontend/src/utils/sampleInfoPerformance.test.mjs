import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import ts from 'typescript';

const require = createRequire(import.meta.url);
const src = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ok = data => ({ code: 0, data });
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const columns = [
  { id: 1, field_key: 'status', label: '状态', data_type: 'text', is_predefined: true, is_active: true, show_in_list: true, show_in_form: false },
  { id: 2, field_key: 'batch_no', label: '批号', data_type: 'text', is_predefined: true, is_active: true, show_in_list: true, show_in_form: true },
  { id: 3, field_key: 'action_collect', label: '取样', data_type: 'action', is_active: true, show_in_list: true, show_in_form: false, width: 80 },
];
const record = (id, status = '待取样') => ({
  id, status, type_key: 'alpha', seq_no: id, batch_no: 'batch-' + id, quantity: 5,
  submitted_at: '2026-10-13T10:00:00', sampled_at: null, sampled_by: '', recorded_quantity: 0,
  workload_recorded: false, extra_fields: {},
});

// Run the real TSX function bodies with deterministic hooks and controllable API promises.
// This verifies request ordering/state behavior; it does not emulate browser layout or React DOM.
function harness() {
  const h = { active: null, cardLayout: false, search: new URLSearchParams('type=alpha'), timers: [], events: new Map(), cache: new Map() };
  const storage = new Map();
  globalThis.localStorage = globalThis.sessionStorage = { getItem: key => storage.get(key) || null, setItem: (key, value) => storage.set(key, value) };
  globalThis.window = {
    setInterval: (callback, ms) => { h.timers.push({ callback, ms }); return h.timers.length; }, clearInterval() {},
    addEventListener: (name, callback) => h.events.set(name, callback), removeEventListener: name => h.events.delete(name),
    requestAnimationFrame: callback => { callback(); return 1; }, scrollTo() {}, scrollY: 0, prompt: () => '',
  };
  const same = (a, b) => a && b && a.length === b.length && a.every((value, index) => Object.is(value, b[index]));
  const react = {
    createElement: (type, props, ...children) => ({ type, props: { ...props, children } }), Fragment: 'Fragment', memo: component => component,
    useState(initial) {
      const owner = h.active, index = owner.cursor++;
      if (!owner.hooks[index]) {
        const state = { value: typeof initial === 'function' ? initial() : initial };
        state.set = next => {
          const value = typeof next === 'function' ? next(state.value) : next;
          if (!Object.is(value, state.value)) { state.value = value; owner.dirty = true; }
        };
        owner.hooks[index] = state;
      }
      return [owner.hooks[index].value, owner.hooks[index].set];
    },
    useMemo(factory, deps) {
      const owner = h.active, index = owner.cursor++, previous = owner.hooks[index];
      if (!previous || !same(previous.deps, deps)) owner.hooks[index] = { deps, value: factory() };
      return owner.hooks[index].value;
    },
    useCallback(callback, deps) { return react.useMemo(() => callback, deps); },
    useRef(initial) { return react.useMemo(() => ({ current: initial }), []); },
    useEffect(callback, deps) {
      const owner = h.active, index = owner.cursor++, previous = owner.hooks[index];
      if (!previous || !same(previous.deps, deps)) {
        owner.hooks[index] = { deps, cleanup: previous?.cleanup };
        owner.effects.push(() => {
          owner.hooks[index].cleanup?.();
          owner.hooks[index].cleanup = callback();
        });
      }
    },
  };
  h.user = { user: { id: 7, username: 'operator', is_admin: true }, hasPermission: () => true };
  h.api = {
    getRdRecordColumns: async () => ok([]), getRdRecords: async () => ok({ items: [], total: 0 }),
    getSampleInfoTypes: async () => ok([{ type_key: 'alpha', label: 'Alpha' }]),
    getCurrentRoleDataScopes: async () => ok({}), getDivisions: async () => ok([]), getGroups: async () => ok([]), getProjects: async () => ok([]),
    getActiveSampleInfoColumns: async () => ok(columns), getSampleInfoDrafts: async () => ok([]),
    getSampleInfoRecords: async () => ok({ items: [record(1)], total: 1 }), batchGetSampleInfoAttachments: async () => ok({}),
    getSampleInfoAttachments: async () => ok([]), uploadSampleInfoAttachment: async () => ok({}), deleteSampleInfoAttachment: async () => ok(null),
    getServerTime: async () => ok({ unix_ms: Date.now() }),
    requireApiSuccess: (response, message) => { if (response.code !== 0) throw Error(response.message || message); return response.data; },
  };
  const stableRef = () => {};
  const material = new Proxy({}, { get: (_, key) => key });
  function load(file) {
    if (h.cache.has(file)) return h.cache.get(file);
    const module = { exports: {} };
    h.cache.set(file, module.exports);
    const compiled = ts.transpileModule(readFileSync(file, 'utf8'), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true, target: ts.ScriptTarget.ES2020 },
    }).outputText;
    const resolve = name => {
      if (name === 'react') return react;
      if (name === '@mui/material') return material;
      if (name.startsWith('@mui/icons-material/')) return { default: name, __esModule: true };
      if (name === 'react-router-dom') return { useSearchParams: () => {
        const owner = h.active;
        return [h.search, next => { h.search = new URLSearchParams(next); owner.dirty = true; }];
      }, useNavigate: () => stableRef };
      if (name.endsWith('/UserContext')) return { useUser: () => h.user };
      if (name.endsWith('/UiDisplayContext')) return { useUiDisplay: () => ({ showBusinessNo: true }) };
      if (name.endsWith('/api/client') || name.endsWith('/api/admin')) return h.api;
      if (name.endsWith('/http')) return { client: h.client, downloadFile() {} };
      if (name.endsWith('/useTableConfig')) return { useTableConfig: () => h.tableConfig };
      if (name.endsWith('/useRecordTableStyle')) return { useRecordTableStyle: () => h.styleApi };
      if (name.endsWith('/useCardLayout')) return { useCardLayout: () => h.cardLayout };
      if (name.endsWith('/RecordActionSettings')) return { useRecordActionLayout: () => ({ config: h.actionConfig }) };
      if (name.endsWith('/columnLayoutHooks')) return { useContainerWidth: () => ({ ref: stableRef, width: 1200 }), ColumnResizeHandle: 'ColumnResizeHandle' };
      if (name.endsWith('/SampleInfoRecordList')) return { default: 'SampleInfoRecordList', __esModule: true };
      if (name.includes('/components/') && !name.endsWith('/RecordTableTools') && !name.endsWith('/mobileCardStyles')) return { default: name.split('/').at(-1), __esModule: true };
      if (!name.startsWith('.')) return require(name);
      const base = path.resolve(path.dirname(file), name);
      const target = ['.ts', '.tsx', '.js'].map(ext => base + ext).find(existsSync);
      assert.ok(target, 'missing import ' + name);
      return load(target);
    };
    new Function('require', 'module', 'exports', compiled)(resolve, module, module.exports);
    h.cache.set(file, module.exports);
    return module.exports;
  }
  h.load = relative => load(path.join(src, relative));
  const styles = h.load('utils/recordTableStyle.ts');
  h.styleApi = { spec: styles.DEFAULT_RECORD_TABLE_STYLE, resolver: styles.createStyleResolver(styles.DEFAULT_RECORD_TABLE_STYLE, {}), tableSx: {}, setColumnWidth() {} };
  h.actionConfig = h.load('utils/recordActionLayout.ts').normalizeActionLayout(null);
  h.tableConfig = { row_height: 48, seq_column_width: 54, checkbox_column_width: 44 };
  h.mount = (component, props) => {
    const owner = { hooks: [], effects: [], dirty: true, cursor: 0, tree: null, props };
    owner.render = () => {
      owner.cursor = 0; owner.dirty = false; h.active = owner;
      const rendered = component(owner.props);
      if (typeof rendered?.type === 'function' && rendered.props?.key !== undefined) {
        if (owner.componentKey !== rendered.props.key) {
          for (const hook of owner.hooks) hook?.cleanup?.();
          owner.hooks = []; owner.effects = []; owner.cursor = 0;
          owner.componentKey = rendered.props.key;
        }
        owner.tree = rendered.type(rendered.props);
      } else owner.tree = rendered;
      h.active = null;
      for (const effect of owner.effects.splice(0)) effect();
    };
    owner.flush = async () => {
      for (let pass = 0; pass < 30; pass++) {
        if (owner.dirty) owner.render();
        await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
        if (!owner.dirty) return;
      }
      throw Error('render did not settle');
    };
    return owner;
  };
  return h;
}

function nodes(tree, predicate, found = []) {
  if (!tree || typeof tree !== 'object') return found;
  if (Array.isArray(tree)) { tree.forEach(child => nodes(child, predicate, found)); return found; }
  if (predicate(tree)) found.push(tree);
  nodes(tree.props?.children, predicate, found);
  return found;
}
const listProps = owner => nodes(owner.tree, node => node.type === 'SampleInfoRecordList')[0].props;

test('initial sample query uses Beijing thirty-day range; all/custom use server paging', async () => {
  const h = harness(), calls = [];
  h.api.getSampleInfoRecords = async params => { calls.push(params); return ok({ items: [], total: 0 }); };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default);
  await owner.flush();
  const dates = h.load('utils/recordDateRange.ts');
  assert.deepEqual({ start: calls[0].start, end: calls[0].end }, dates.recordDatePreset('recent-30', dates.beijingRecordToday()));
  listProps(owner).onStartChange(''); listProps(owner).onEndChange('');
  await owner.flush();
  assert.equal(calls.at(-1).start, undefined); assert.equal(calls.at(-1).end, undefined);
  listProps(owner).onStartChange('2025-01-01'); listProps(owner).onEndChange('2025-01-31');
  await owner.flush();
  assert.equal(calls.at(-1).start, '2025-01-01'); assert.equal(calls.at(-1).page_size, 20);
});

test('legacy sample notification URLs fetch one record without time/type filters and load its actual column type', async () => {
  const h = harness(), calls = [], columnTypes = [];
  h.search = new URLSearchParams('record_id=91');
  h.api.getSampleInfoRecords = async params => { calls.push(params); return ok({ items: [{ ...record(91), type_key: 'beta', submitted_at: '2020-01-01' }], total: 1 }); };
  h.api.getActiveSampleInfoColumns = async type => { columnTypes.push(type); return ok(type === 'beta' ? [{ ...columns[0], id: 99, label: 'Beta 状态' }] : columns); };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default); await owner.flush();
  assert.deepEqual(calls, [{ record_id: 91, page: 1, page_size: 1 }]);
  assert.ok(columnTypes.includes('beta')); assert.equal(listProps(owner).columns[0].id, 99);
  assert.equal(listProps(owner).notificationRecordId, 91); assert.equal(listProps(owner).records[0].id, 91);
  assert.equal(nodes(owner.tree, node => node.type?.name === 'SampleSubmissionClock').length, 0);
  const notice = nodes(owner.tree, node => node.type === 'Alert' && node.props.severity === 'info')[0];
  notice.props.action.props.onClick(); await owner.flush();
  assert.equal(h.search.has('record_id'), false); assert.equal(h.search.get('type'), 'beta');
  assert.equal(listProps(owner).notificationRecordId, undefined);
  assert.equal(calls.at(-1).type_key, 'beta'); assert.ok(calls.at(-1).start);
});

test('invalid, deleted and forbidden sample notification targets fail closed without showing stale records', async () => {
  for (const [query, response, message, expectedCalls] of [
    ['record_id=', ok({ items: [], total: 0 }), /编号无效/, 0],
    ['record_id=91', ok({ items: [], total: 0 }), /已删除或当前账号无权查看/, 1],
    ['record_id=91', ok({ items: [record(90)], total: 1 }), /已删除或当前账号无权查看/, 1],
    ['record_id=91', { code: 2001, message: '无权查看', data: null }, /无权查看/, 1],
  ]) {
    const h = harness(); let calls = 0;
    h.search = new URLSearchParams(query);
    h.api.getSampleInfoRecords = async () => { calls++; return response; };
    const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default); await owner.flush();
    assert.equal(calls, expectedCalls);
    assert.equal(nodes(owner.tree, node => node.type === 'SampleInfoRecordList').length, 0);
    const alert = nodes(owner.tree, node => node.type === 'Alert' && node.props.severity === 'error')[0];
    assert.match(alert.props.children.join(''), message);
  }
});

test('out-of-order page responses cannot overwrite the newest records or clear its loading', async () => {
  const h = harness(), calls = [];
  h.api.getSampleInfoRecords = (params, signal) => { const pending = deferred(); calls.push({ params, signal, ...pending }); return pending.promise; };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default);
  await owner.flush();
  listProps(owner).onPageChange(1); await owner.flush();
  assert.equal(calls[0].signal.aborted, true);
  calls[0].resolve(ok({ items: [record(1)], total: 50 })); await owner.flush();
  assert.equal(listProps(owner).loading, true); assert.deepEqual(listProps(owner).records, []);
  calls[1].resolve(ok({ items: [record(2)], total: 50 })); await owner.flush();
  assert.equal(listProps(owner).records[0].id, 2); assert.equal(listProps(owner).total, 50); assert.equal(listProps(owner).loading, false);
});

test('sampling is locked per row, patches the confirmed server row and does not refetch unchanged attachment IDs', async () => {
  const h = harness(), sample = deferred(), correction = deferred();
  let lists = 0, samples = 0, batches = 0;
  h.api.getSampleInfoRecords = () => ++lists === 1 ? Promise.resolve(ok({ items: [record(1)], total: 1 })) : correction.promise;
  h.api.sampleSampleInfo = () => { samples++; return sample.promise; };
  h.api.batchGetSampleInfoAttachments = async () => { batches++; return ok({ 1: [] }); };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default);
  await owner.flush();
  const first = listProps(owner).onStatusFlow(1, '待取样');
  await listProps(owner).onStatusFlow(1, '待取样'); await owner.flush();
  assert.equal(samples, 1); assert.equal(listProps(owner).pendingRecordIds.has(1), true);
  sample.resolve(ok({ ...record(1, '待检测'), sampled_by: 'server-operator', sampled_at: '2026-10-13T10:01:00' }));
  await owner.flush();
  assert.equal(listProps(owner).records[0].sampled_by, 'server-operator');
  assert.equal(listProps(owner).records[0].status, '待检测'); assert.equal(listProps(owner).loading, true); assert.equal(batches, 1);
  correction.resolve(ok({ items: [record(1, '待检测')], total: 1 })); await first; await owner.flush();
  assert.equal(listProps(owner).pendingRecordIds.size, 0); assert.equal(batches, 1);
});

test('attachment page races are ignored and upload/delete explicitly invalidate the current batch', async () => {
  const h = harness(), batches = [];
  h.api.getSampleInfoRecords = async params => ok({ items: [record(params.page)], total: 50 });
  h.api.batchGetSampleInfoAttachments = ids => { const pending = deferred(); batches.push({ ids, ...pending }); return pending.promise; };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default);
  await owner.flush();
  listProps(owner).onPageChange(1); await owner.flush();
  batches[1].resolve(ok({ 2: [{ id: 20, file_name: 'new.pdf' }] })); await owner.flush();
  batches[0].resolve(ok({ 1: [{ id: 10, file_name: 'old.pdf' }] })); await owner.flush();
  assert.equal(listProps(owner).attachmentsByRow[2][0].file_name, 'new.pdf');
  await listProps(owner).onUploadAttachment(2, {}); await owner.flush();
  assert.equal(batches.length, 3);
  await listProps(owner).onDeleteAttachment(20, 2); await owner.flush();
  assert.equal(batches.length, 4);
});

test('failed sampling preserves the record and unlocks the row without pretending success', async () => {
  const h = harness(); let lists = 0;
  h.api.getSampleInfoRecords = async () => { lists++; return ok({ items: [record(1)], total: 1 }); };
  h.api.sampleSampleInfo = async () => ({ code: 2001, message: '没有取样权限', data: null });
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default); await owner.flush();
  await listProps(owner).onStatusFlow(1, '待取样'); await owner.flush();
  assert.equal(listProps(owner).records[0].status, '待取样'); assert.equal(listProps(owner).pendingRecordIds.size, 0);
  assert.equal(lists, 1);
  assert.ok(nodes(owner.tree, node => node.type === 'Alert' && node.props.severity === 'error').length > 0);
});

test('background correction fixes an out-of-range page and page-size changes never request the obsolete page', async () => {
  const h = harness(), calls = []; let shrinking = false;
  h.api.getSampleInfoRecords = async params => {
    calls.push(params);
    const total = shrinking ? 21 : 50;
    return ok({ items: params.page > Math.ceil(total / params.page_size) ? [] : [record(params.page)], total });
  };
  h.api.sampleSampleInfo = async id => { shrinking = true; return ok(record(id, '待检测')); };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default); await owner.flush();
  listProps(owner).onPageChange(2); await owner.flush();
  await listProps(owner).onStatusFlow(3, '待取样'); await owner.flush();
  assert.equal(listProps(owner).page, 1); assert.equal(listProps(owner).total, 21); assert.equal(listProps(owner).records[0].id, 2);
  assert.deepEqual(calls.map(call => call.page), [1, 3, 3, 2]);
  const before = calls.length;
  h.styleApi.spec = { ...h.styleApi.spec, paging: { size: 100 } }; owner.dirty = true; await owner.flush();
  assert.deepEqual(calls.slice(before).map(call => [call.page, call.page_size]), [[1, 100]]);
});

test('column type changes reject late old form/list configurations', async () => {
  const h = harness(), calls = [];
  h.api.getActiveSampleInfoColumns = type => { const pending = deferred(); calls.push({ type, ...pending }); return pending.promise; };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default);
  await owner.flush();
  h.search.set('type', 'beta'); owner.dirty = true; await owner.flush();
  const beta = [{ ...columns[0], id: 99, label: 'Beta 状态' }];
  calls.filter(call => call.type === 'beta').forEach(call => call.resolve(ok(beta))); await owner.flush();
  calls.filter(call => call.type === 'alpha').forEach(call => call.resolve(ok(columns))); await owner.flush();
  assert.equal(listProps(owner).columns[0].id, 99);
});

test('same-record account changes immediately discard old sample rows and scoped column requests', async () => {
  const h = harness(), nextRecords = deferred(), oldColumns = deferred(), nextColumns = [];
  h.search = new URLSearchParams('record_id=91&type=alpha');
  h.user.token = 'token-a';
  h.api.getSampleInfoRecords = () => h.user.user.id === 7 ? Promise.resolve(ok({ items: [record(91)], total: 1 })) : nextRecords.promise;
  let pendingColumns = false;
  h.api.getActiveSampleInfoColumns = () => {
    if (!pendingColumns) return Promise.resolve(ok(columns));
    if (h.user.user.id === 7) return oldColumns.promise;
    const task = deferred(); nextColumns.push(task); return task.promise;
  };
  const owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default); await owner.flush();
  assert.equal(listProps(owner).records[0].id, 91); assert.equal(listProps(owner).columns[0].id, 1);
  pendingColumns = true; h.events.get('focus')();
  h.user = { ...h.user, user: { ...h.user.user, id: 8 }, token: 'token-b' };
  owner.render();
  assert.deepEqual(listProps(owner).records, []); assert.deepEqual(listProps(owner).columns, []);
  oldColumns.resolve(ok([{ ...columns[0], id: 88, label: 'old private column' }])); await owner.flush();
  assert.deepEqual(listProps(owner).columns, []); assert.deepEqual(listProps(owner).records, []);
  nextColumns.forEach(task => task.resolve(ok([{ ...columns[0], id: 99 }])));
  nextRecords.resolve(ok({ items: [{ ...record(91), user_name: 'new account' }], total: 1 })); await owner.flush();
  assert.equal(listProps(owner).records[0].user_name, 'new account'); assert.equal(listProps(owner).columns[0].id, 99);
});

test('RD targets hide unrelated old rows during loading and session keys discard old scope callbacks', async () => {
  const h = harness(), targetRecords = deferred(), oldColumns = deferred(), nextColumns = deferred(), newRecords = deferred();
  h.search = new URLSearchParams(); h.user.token = 'token-a';
  let targeted = false, switched = false;
  h.api.getRdRecords = () => switched ? newRecords.promise : targeted ? targetRecords.promise : Promise.resolve(ok({ items: [{ ...record(1), sequence_no: 1 }], total: 1 }));
  h.api.getRdRecordColumns = () => switched ? nextColumns.promise : oldColumns.promise;
  const owner = h.mount(h.load('pages/RdRecordsPage.tsx').default); await owner.flush();
  const summary = () => nodes(owner.tree, node => node.type === 'TableStyleBar')[0].props;
  assert.equal(summary().rows.length, 1);
  targeted = true; h.search.set('record_id', '91'); owner.render();
  assert.deepEqual(summary().rows, []);
  targetRecords.resolve(ok({ items: [{ ...record(91), sequence_no: 91 }], total: 1 })); await owner.flush();
  assert.equal(summary().rows[0].key, '91');
  switched = true; h.user = { ...h.user, user: { ...h.user.user, id: 8 }, token: 'token-b' }; owner.render();
  assert.deepEqual(summary().rows, []);
  oldColumns.resolve(ok([{ id: 88, name: 'private_field', label: 'old private column', is_active: true, show_in_list: true, data_type: 'text' }])); await owner.flush();
  assert.equal(summary().columns.some(column => column.key === 'private_field'), false);
  nextColumns.resolve(ok([])); newRecords.resolve(ok({ items: [{ ...record(91), sequence_no: 91 }], total: 1 })); await owner.flush();
  assert.equal(summary().rows[0].key, '91');
});

test('one-second clock ticks belong to the clock component, not the form/list owner', async () => {
  const h = harness(), owner = h.mount(h.load('pages/SampleInfoEntry.tsx').default);
  await owner.flush(); assert.equal(h.timers.length, 0);
  const clock = nodes(owner.tree, node => node.type?.name === 'SampleSubmissionClock')[0];
  assert.ok(clock);
  const clockOwner = h.mount(clock.type); await clockOwner.flush();
  assert.deepEqual(h.timers.map(timer => timer.ms), [1000, 60000]);
  const previousNow = Date.now; const now = previousNow();
  try { Date.now = () => now + 5000; h.timers[0].callback(); } finally { Date.now = previousNow; }
  assert.equal(clockOwner.dirty, true); assert.equal(owner.dirty, false);
});

test('active-column concurrent reads share only within an auth/type key and are not persistently cached', async () => {
  const h = harness(), calls = [];
  h.client = { get: (url, options) => { const pending = deferred(); calls.push({ url, options, ...pending }); return pending.promise; } };
  const api = h.load('api/sampleInfo.ts');
  localStorage.setItem('workload_token', 'account-a');
  const a = api.getActiveSampleInfoColumns('alpha'), duplicate = api.getActiveSampleInfoColumns('alpha');
  assert.equal(a, duplicate); assert.equal(calls.length, 1);
  const otherType = api.getActiveSampleInfoColumns('beta');
  localStorage.setItem('workload_token', 'account-b'); const otherUser = api.getActiveSampleInfoColumns('alpha');
  assert.equal(calls.length, 3); assert.notEqual(a, otherUser);
  calls.forEach(call => call.resolve({ data: ok(columns) })); await Promise.all([a, otherType, otherUser]);
  const fresh = api.getActiveSampleInfoColumns('alpha'); assert.equal(calls.length, 4);
  calls[3].resolve({ data: ok(columns) }); await fresh;
  const failed = api.getActiveSampleInfoColumns('alpha'); calls[4].reject(Error('offline')); await assert.rejects(failed, /offline/);
  const retried = api.getActiveSampleInfoColumns('alpha'); assert.equal(calls.length, 6);
  calls[5].resolve({ data: ok(columns) }); await retried;
});

test('only current layout is mounted; mobile detail mounts on first expansion and survives refresh/collapse', async () => {
  const h = harness(); let reads = 0;
  const owner = h.mount(h.load('components/SampleInfoRecordList.tsx').default, {
    records: [record(1)], total: 1, page: 0, pageSize: 20, loading: false, statusFilter: '全部', statusOptions: ['全部'],
    typeFilter: 'alpha', typeOptions: [], startDate: '', endDate: '', columns, editColumns: [], columnWidths: {},
    attachmentsByRow: {}, attachments: {}, attachmentLoading: {}, editingId: null, editForm: {}, divisionOptions: [],
    getRecordValue: (row, field) => { reads++; return row[field]; }, formatDate: value => value, hasPermission: () => true,
  });
  await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'TableContainer').length, 1);
  assert.equal(nodes(owner.tree, node => node.type === 'Card').length, 0);
  const previousReads = reads; owner.dirty = true; await owner.flush();
  assert.equal(reads - previousReads, 1, 'only the displayed batch cell is read; cached filter/style maps should not rescan');
  h.cardLayout = true; owner.dirty = true; await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'TableContainer').length, 0);
  const detail = () => nodes(owner.tree, node => node.props?.component === 'details')[0];
  assert.equal(detail().props.children[1], false);
  detail().props.onToggle({ currentTarget: { open: true } }); await owner.flush();
  assert.equal(detail().props.open, true); assert.ok(detail().props.children[1]);
  owner.props = { ...owner.props, loading: true, pendingRecordIds: new Set([1]) }; owner.dirty = true; await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'Card').length, 1); assert.equal(detail().props.open, true);
  assert.ok(nodes(owner.tree, node => node.type === 'Button' && node.props.disabled === true).length > 0);
  detail().props.onToggle({ currentTarget: { open: false } }); await owner.flush();
  assert.equal(detail().props.open, false); assert.ok(detail().props.children[1], 'visited content remains mounted to preserve field scroll');
  h.cardLayout = false; owner.dirty = true; await owner.flush();
  const filter = nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label'] === '筛选 批号')[0];
  assert.ok(filter); filter.props.onClick(); await owner.flush();
  nodes(owner.tree, node => node.type?.name === 'SampleInfoFilterDialog')[0].props.onApply('batch_no', ['another-batch']); await owner.flush();
  h.cardLayout = true; owner.dirty = true; await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'Card').length, 0);
  owner.props = { ...owner.props, notificationRecordId: 1 }; owner.dirty = true; await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'Card').length, 1, 'notification lookup bypasses stale local column filters');
  assert.equal(nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label']?.startsWith('筛选 ')).length, 0);
  assert.equal(nodes(owner.tree, node => node.type?.name === 'SampleInfoFilterDialog').length, 0);
  assert.equal(nodes(owner.tree, node => node.type === 'TablePagination').length, 0);
});

const sampleListFixture = overrides => ({
  records: [record(1)], total: 80, page: 0, pageSize: 20, loading: false,
  statusFilter: '全部', statusOptions: ['全部', '待取样', '待检测'], typeFilter: '', typeOptions: [],
  startDate: '', endDate: '', columns, editColumns: [], columnWidths: {},
  attachmentsByRow: {}, attachments: {}, attachmentLoading: {}, editingId: null, editForm: {}, divisionOptions: [],
  getRecordValue: (row, field) => row[field], formatDate: value => value, hasPermission: () => true,
  onStartChange() {}, onEndChange() {}, onResetFilters() {}, ...overrides,
});
const sampleFilterDialog = owner => nodes(owner.tree, node => node.type?.name === 'SampleInfoFilterDialog')[0].props;
const button = (owner, label) => nodes(owner.tree, node => node.type === 'Button' && node.props.children.includes(label))[0];

test('sample column headers keep full-width text filters in a separate row, including empty and operation columns', async () => {
  const h = harness(), owner = h.mount(h.load('components/SampleInfoRecordList.tsx').default, sampleListFixture({
    records: [], columns: [...columns, { ...columns[1], id: 9, field_key: 'notes', label: '注意事项' }],
    columnWidths: { notes: { mobile: 36, desktop: '36px' } },
  }));
  await owner.flush();
  const filter = nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label'] === '筛选 注意事项')[0];
  assert.ok(filter, 'empty candidates must not hide the filter');
  assert.equal(filter.props.sx.width, '100%'); assert.equal(filter.props.sx.minWidth, 0);
  const header = nodes(owner.tree, node => node.type === 'TableCell' && node.props.title === '注意事项')[0];
  const stack = nodes(header, node => node.type === 'Box' && node.props.sx?.flexDirection === 'column')[0];
  assert.equal(stack.props.children.at(-1), filter);
  assert.equal(header.props.sx.width, '36px');
  filter.props.onClick(); await owner.flush(); assert.equal(sampleFilterDialog(owner).field, 'notes');
  assert.deepEqual(sampleFilterDialog(owner).options.notes, []);
  const action = nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label'] === '筛选 操作进度')[0];
  action.props.onClick(); await owner.flush(); assert.equal(sampleFilterDialog(owner).field, '_action');
  button(owner, '按列筛选').props.onClick(); await owner.flush();
  assert.ok(sampleFilterDialog(owner).columns.some(column => column.key === 'submitted_at'), 'time remains accessible when its column is hidden');
});

test('sample reset clears local column filters and parent filters; 全部 is not counted and status removal restores 全部', async () => {
  const h = harness(), reset = [], statuses = [];
  const owner = h.mount(h.load('components/SampleInfoRecordList.tsx').default, sampleListFixture({
    onResetFilters: () => reset.push(true), onStatusChange: value => statuses.push(value),
  }));
  await owner.flush();
  const chips = () => nodes(owner.tree, node => node.type?.name === 'ActiveFilterChips')[0].props;
  assert.deepEqual(chips().items, []);
  sampleFilterDialog(owner).onApply('batch_no', ['another-batch']); await owner.flush();
  assert.equal(chips().matched, 0); assert.equal(chips().scopeHint, '当前页');
  button(owner, '重置').props.onClick(); await owner.flush();
  assert.equal(reset.length, 1); assert.equal(chips().matched, 1);
  assert.deepEqual(sampleFilterDialog(owner).filters, {});
  owner.props = { ...owner.props, statusFilter: '待检测' }; owner.dirty = true; await owner.flush();
  assert.equal(chips().items[0].label, '状态：待检测');
  chips().onRemove('server:status'); assert.deepEqual(statuses, ['全部']);
  sampleFilterDialog(owner).onApply('batch_no', ['another-batch']); await owner.flush();
  chips().onClearAll(); await owner.flush(); assert.equal(reset.length, 2); assert.equal(chips().matched, 1);
});

test('sample toolbar date presets and shared header/global date entry pass date-only server ranges', async () => {
  const h = harness(), ranges = [], dates = h.load('utils/recordDateRange.ts');
  const owner = h.mount(h.load('components/SampleInfoRecordList.tsx').default, sampleListFixture({
    columns: [...columns, { ...columns[1], id: 9, field_key: 'submitted_at', label: '送样时间', data_type: 'date' }],
    onStartChange: value => ranges.push(['start', value]), onEndChange: value => ranges.push(['end', value]),
  }));
  await owner.flush();
  for (const [label, preset] of [['近30天', 'recent-30'], ['本周', 'this-week'], ['上周', 'last-week'], ['近7天', 'recent'], ['全部历史', 'all']]) {
    button(owner, label).props.onClick(); await owner.flush();
    const expected = dates.recordDatePreset(preset, dates.beijingRecordToday());
    assert.deepEqual(ranges.splice(0), [['start', expected.start], ['end', expected.end]]);
  }
  button(owner, '自定义时间').props.onClick(); await owner.flush(); assert.equal(sampleFilterDialog(owner).field, 'submitted_at');
  sampleFilterDialog(owner).onApplyTime('2025-01-01', '2025-01-31');
  assert.deepEqual(ranges.splice(0), [['start', '2025-01-01'], ['end', '2025-01-31']]);
  sampleFilterDialog(owner).onClose(); await owner.flush();
  nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label'] === '筛选 送样时间')[0].props.onClick(); await owner.flush();
  assert.equal(sampleFilterDialog(owner).field, 'submitted_at');
});

test('mobile global and detail filters share actual current-page operation progress without claiming recorder identity', async () => {
  const h = harness(); h.cardLayout = true;
  const rows = [record(1), { ...record(2, '待检测'), sampled_at: '2026-10-13T11:00:00' },
    { ...record(3, '待检测'), recorded_quantity: 2 }, { ...record(4, '已检测'), workload_recorded: true },
    record(5, '已退回'), record(6, '已检测')];
  const owner = h.mount(h.load('components/SampleInfoRecordList.tsx').default, sampleListFixture({ records: rows }));
  await owner.flush();
  button(owner, '按列筛选').props.onClick(); await owner.flush();
  assert.equal(sampleFilterDialog(owner).field, '_action');
  assert.deepEqual(new Set(sampleFilterDialog(owner).options._action), new Set(['待取样', '已取样未录入工作量', '部分录入工作量', '已录入工作量', '已退回', '已完成检测（未录入工作量）']));
  const detail = nodes(owner.tree, node => node.props?.component === 'details')[0];
  detail.props.onToggle({ currentTarget: { open: true } }); await owner.flush();
  assert.ok(nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label'] === '筛选 批号').length);
  assert.ok(nodes(owner.tree, node => node.type === 'Button' && node.props['aria-label'] === '筛选 操作进度').length);
  sampleFilterDialog(owner).onApply('_action', ['已录入工作量']); await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'Card').length, 1);
  assert.equal(nodes(owner.tree, node => node.type?.name === 'ActiveFilterChips')[0].props.matched, 1);
  owner.props = { ...owner.props, notificationRecordId: 1, records: [rows[0]] }; owner.dirty = true; await owner.flush();
  assert.equal(nodes(owner.tree, node => node.type === 'Card').length, 1);
  assert.equal(nodes(owner.tree, node => node.type?.name === 'SampleInfoFilterDialog').length, 0);
  assert.equal(button(owner, '按列筛选'), undefined);
});

test('sample filter dialog validates date-only drafts, applies on confirmation and can clear an empty column', async () => {
  const h = harness(), applied = [], closed = [];
  const owner = h.mount(h.load('components/recordTable/SampleInfoFilterDialog.tsx').default, {
    field: 'submitted_at', columns: [{ key: 'submitted_at', label: '送样时间' }, { key: 'notes', label: '注意事项' }],
    filters: {}, options: { notes: [] }, start: '2026-10-01', end: '2026-10-15', fontSize: 12,
    onFieldChange() {}, onClose: () => closed.push(true), onApply: (field, values) => applied.push([field, values]),
    onApplyTime: (start, end) => applied.push([start, end]),
  });
  await owner.flush();
  const inputs = () => nodes(owner.tree, node => node.type === 'TextField' && node.props.type === 'date');
  assert.equal(inputs().length, 2);
  inputs()[0].props.onChange({ target: { value: '2026-10-20' } }); await owner.flush();
  button(owner, '应用筛选').props.onClick(); await owner.flush();
  assert.equal(applied.length, 0); assert.equal(closed.length, 0);
  assert.ok(nodes(owner.tree, node => node.type === 'Alert' && node.props.children.includes('开始日期不能晚于结束日期')).length);
  inputs()[0].props.onChange({ target: { value: '2026-10-02' } }); await owner.flush();
  button(owner, '应用筛选').props.onClick(); assert.deepEqual(applied, [['2026-10-02', '2026-10-15']]); assert.equal(closed.length, 1);
  button(owner, '清除此列').props.onClick(); assert.deepEqual(applied.at(-1), ['', '']);
  owner.props = { ...owner.props, field: 'notes', filters: { notes: ['旧条件'] } }; owner.dirty = true; await owner.flush();
  assert.ok(nodes(owner.tree, node => node.type === 'Chip' && node.props.label === '旧条件').length);
  button(owner, '清除此列').props.onClick(); assert.deepEqual(applied.at(-1), ['notes', []]);
});
