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
      if (name === 'react-router-dom') return { useSearchParams: () => [h.search], useNavigate: () => stableRef };
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
      owner.tree = component(owner.props);
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
});
