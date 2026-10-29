import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

// Execute the actual page callbacks without booting a browser or a database.
function findCode(file, matches) {
  const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let found;
  const visit = node => {
    if (!found && matches(node)) found = node;
    if (!found) ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(found, `Missing source function in ${file}`);
  return { found, source };
}
function callback(file, name) {
  const { found, source } = findCode(file, node => ts.isVariableDeclaration(node) && node.name.getText() === name);
  const expression = ts.isCallExpression(found.initializer) ? found.initializer.arguments[0] : found.initializer;
  return expression.getText(source);
}
function declaration(file, name) {
  const { found, source } = findCode(file, node => ts.isFunctionDeclaration(node) && node.name?.text === name);
  return found.getText(source).replace(/^export\s+/, '');
}
function effect(file, marker) {
  const { found, source } = findCode(file, node => ts.isCallExpression(node) && node.expression.getText() === 'useEffect'
    && node.arguments[0].getText().includes(marker));
  return found.arguments[0].getText(source);
}
function bind(code, env) {
  const compiled = ts.transpileModule(`const selected = (${code});`, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React },
  }).outputText;
  return new Function(...Object.keys(env), `${compiled}\nreturn selected;`)(...Object.values(env));
}
const flush = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

test('clearing RD filters resets a manually selected laboratory and preserves the fixed analysis laboratory', () => {
  for (const analysisGroupId of [undefined, 17]) {
    const state = { group: 9, columns: { notes: ['urgent'] }, operations: ['recorded'], recorders: ['7'], range: null };
    const clear = bind(callback('./RdRecordsPage.tsx', 'clearFilters'), {
      analysisGroupId,
      setColumnFilters: value => { state.columns = value; },
      setOperationFilters: value => { state.operations = value; },
      setRecorderFilters: value => { state.recorders = value; },
      setGroupFilter: value => { state.group = value; },
      applyTimeRange: (start, end) => { state.range = [start, end]; },
    });
    clear();
    assert.equal(state.group, analysisGroupId === undefined ? 0 : 9);
    assert.deepEqual([state.columns, state.operations, state.recorders, state.range], [{}, [], [], ['', '']]);
    const query = bind(callback('./RdRecordsPage.tsx', 'query'), {
      hasNotificationRecord: false, notificationRecordId: undefined, analysisGroupId,
      startDate: '', endDate: '', groupFilter: state.group,
      columnFilters: state.columns, operationFilters: state.operations, recorderFilters: state.recorders,
      rdFilterQuery: () => ({}),
    })();
    assert.equal(query.analysis_lab_id, analysisGroupId);
    assert.equal(query.group_id, undefined);
  }
});

test('a manually selected RD laboratory is visible in the active filter chips', () => {
  const text = readFileSync(new URL('./RdRecordsPage.tsx', import.meta.url), 'utf8');
  const code = text.slice(text.indexOf('const activeFilterItems:'), text.indexOf('const applyTimeRange ='));
  const compile = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  for (const analysisGroupId of [undefined, 17]) {
    const env = { columnFilters: {}, displayColumns: [], filterLabels: {}, startDate: '', endDate: '',
      analysisGroupId, groupFilter: 9, groups: [{ id: 9, name: '液相实验室' }], operationFilters: [], recorderFilters: [], RD_OPERATION_LABELS: {} };
    const items = new Function(...Object.keys(env), `${compile}\nreturn activeFilterItems;`)(...Object.values(env));
    assert.deepEqual(items, analysisGroupId === undefined ? [{ key: 'group', label: '实验室：液相实验室' }] : []);
  }
});

test('RD filter dialog quick weeks use Beijing midnight independently of the client timezone', async () => {
  const source = readFileSync(new URL('../utils/recordDateRange.ts', import.meta.url), 'utf8');
  const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
  const dates = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
  const { found, source: dialog } = findCode('../components/recordTable/RdRecordFilterDialog.tsx', node => ts.isArrowFunction(node) && node.parameters.length === 0
    && node.getText().includes('const range = recordDatePreset(key,') && node.getText().includes('setDraftStart(recordTimeDraft(range.start))'));
  let start, end;
  bind(found.getText(dialog), {
    key: 'this-week', recordDatePreset: dates.recordDatePreset,
    beijingRecordToday: () => dates.beijingRecordToday(new Date('2026-10-25T16:05:00Z')),
    recordTimeDraft: (value, last) => `${value}T${last ? '23:59:59.999' : '00:00:00'}`,
    setDraftStart: value => { start = value; }, setDraftEnd: value => { end = value; }, setError: () => {},
  })();
  assert.equal(start, '2026-10-26T00:00:00');
  assert.equal(end, '2026-11-01T23:59:59.999');
});

test('sheet1 project totals include every row and keep same-name projects in separate labs', () => {
  const totalsFor = bind(declaration('./StatsPage.tsx', 'previewProjectTotals'), {});
  const rows = [
    ['A', 'X', '', '', 1, 3], ['A', 'X', '', '', 1, 7], ['B', 'X', '', '', 1, 5],
    ['A|B', 'C', '', '', 1, 11], ['A', 'B|C', '', '', 1, 13],
  ];
  const before = structuredClone(rows);
  const result = totalsFor(rows);
  assert.equal(result.get(JSON.stringify(['A', 'X'])), 10);
  assert.equal(result.get(JSON.stringify(['B', 'X'])), 5);
  assert.equal(result.get(JSON.stringify(['A|B', 'C'])), 11);
  assert.equal(result.get(JSON.stringify(['A', 'B|C'])), 13);
  assert.deepEqual(rows, before);
  assert.equal(totalsFor([]).size, 0);
});

function sampleEnvironment() {
  const state = { rows: [{ id: 1, sequence_no: 41, status: '待取样', notes: 'preserved' }, { id: 2, sequence_no: 42, status: '待取样' }], samplingId: null, messages: [], loading: false };
  const env = {
    samplingLock: { current: false }, pageMounted: { current: true }, currentView: { current: 'view-a' },
    recordRequest: { current: 0 }, todayRecordRequest: { current: 0 }, page: 2, recordsPage: 2,
    loading: false, recordsLoading: false,
    selectedDetectorId: 0, recordSort: { field: 'submitted_at', direction: 'desc' }, operationFilters: [], columnFilters: {},
    hasNotificationRecord: false, notificationRecordId: undefined,
    setSamplingId: value => { state.samplingId = value; }, setLoading: value => { state.loading = value; },
    setRecordsLoading: value => { state.loading = value; }, setSnackMsg: value => { state.messages.push(value); }, setSnackErr: () => {},
    setRecords: updater => { state.rows = typeof updater === 'function' ? updater(state.rows) : updater; },
    setTodayRecords: updater => { state.rows = typeof updater === 'function' ? updater(state.rows) : updater; },
    requireApiSuccess: response => { if (response.code !== 0) throw new Error(response.message); return response.data; },
    loadRecords: async () => {}, loadTodayRecords: async () => {},
  };
  return { state, env };
}

test('take-sample cannot invalidate an in-flight foreground page request while old rows are still displayed', async () => {
  for (const file of ['./RdRecordsPage.tsx', './SampleEntryPage.tsx']) {
    const { state, env } = sampleEnvironment();
    env.loading = true; env.recordsLoading = true;
    env.recordRequest.current = 4; env.todayRecordRequest.current = 5;
    let samples = 0;
    env.sampleRdRecord = async () => { samples += 1; return { code: 0, data: { id: 1, status: '已取样' } }; };
    const takeSample = bind(callback(file, 'handleSample'), env);
    await takeSample(file.includes('SampleEntry') ? state.rows[0] : 1);
    assert.equal(samples, 0);
    assert.equal(env.recordRequest.current, 4);
    assert.equal(env.todayRecordRequest.current, 5);
    assert.equal(state.rows[0].status, '待取样');
    assert.equal(env.samplingLock.current, false);
    assert.equal(state.samplingId, null);
  }
});

test('both RD take-sample actions lock double clicks and patch only the returned row without a list request', async () => {
  for (const file of ['./RdRecordsPage.tsx', './SampleEntryPage.tsx']) {
    const { state, env } = sampleEnvironment();
    const pending = deferred();
    let samples = 0;
    let reloads = 0;
    env.sampleRdRecord = () => { samples += 1; return pending.promise; };
    env.loadRecords = env.loadTodayRecords = async () => { reloads += 1; };
    const takeSample = bind(callback(file, 'handleSample'), env);
    const argument = file.includes('SampleEntry') ? state.rows[0] : 1;
    const first = takeSample(argument);
    await takeSample(argument);
    assert.equal(samples, 1);
    assert.equal(env.samplingLock.current, true);
    pending.resolve({ code: 0, data: { id: 1, sequence_no: 0, status: '已取样', sampler: 'tester' } });
    await first;
    assert.deepEqual(state.rows, [{ id: 1, sequence_no: 41, status: '已取样', sampler: 'tester', notes: 'preserved' }, { id: 2, sequence_no: 42, status: '待取样' }]);
    assert.equal(reloads, 0);
    assert.equal(env.samplingLock.current, false);
    assert.equal(state.samplingId, null);
  }
});

test('a list refresh started after take-sample gets a locked silent correction so other row updates are retained', async () => {
  for (const file of ['./RdRecordsPage.tsx', './SampleEntryPage.tsx']) {
    const { state, env } = sampleEnvironment();
    const sample = deferred(), correction = deferred();
    let reloads = 0, samples = 0;
    env.sampleRdRecord = () => { samples += 1; return sample.promise; };
    env.loadRecords = env.loadTodayRecords = async (page, background) => {
      reloads += 1; assert.equal(page, 2); assert.equal(background, true);
      await correction.promise;
      state.rows = state.rows.map(row => row.id === 2 ? { ...row, status: '已录入' } : row);
    };
    const takeSample = bind(callback(file, 'handleSample'), env);
    const argument = file.includes('SampleEntry') ? state.rows[0] : 1;
    const first = takeSample(argument);
    // Another successful row action starts a same-view list request while sampling is in flight.
    env.recordRequest.current += 1; env.todayRecordRequest.current += 1;
    sample.resolve({ code: 0, data: { id: 1, sequence_no: 0, status: '已取样' } });
    await flush();
    assert.equal(reloads, 1);
    assert.equal(state.rows[0].sequence_no, 41);
    assert.equal(env.samplingLock.current, true);
    await takeSample(argument); assert.equal(samples, 1);
    correction.resolve(); await first;
    assert.equal(state.rows[1].status, '已录入');
    assert.equal(env.samplingLock.current, false);
  }
});

test('take-sample keeps its lock through affected-sort correction and ignores responses from an old view', async () => {
  for (const file of ['./RdRecordsPage.tsx', './SampleEntryPage.tsx']) {
    const { state, env } = sampleEnvironment();
    const correction = deferred();
    let samples = 0;
    env.recordSort = { field: 'sampling_person', direction: 'asc' };
    env.sampleRdRecord = async () => { samples += 1; return { code: 0, data: { id: 1, status: '已取样' } }; };
    env.loadRecords = env.loadTodayRecords = async (page, background) => {
      assert.equal(page, 2); assert.equal(background, true); await correction.promise;
    };
    const takeSample = bind(callback(file, 'handleSample'), env);
    const argument = file.includes('SampleEntry') ? state.rows[0] : 1;
    const first = takeSample(argument);
    await flush();
    await takeSample(argument);
    assert.equal(samples, 1);
    assert.equal(env.samplingLock.current, true);
    correction.resolve(); await first;
    assert.equal(env.samplingLock.current, false);

    for (const leave of ['query', 'unmount']) {
      const pending = deferred();
      const next = sampleEnvironment();
      next.env.sampleRdRecord = () => pending.promise;
      const action = bind(callback(file, 'handleSample'), next.env);
      const task = action(file.includes('SampleEntry') ? next.state.rows[0] : 1);
      if (leave === 'query') next.env.currentView.current = 'view-b';
      else next.env.pageMounted.current = false;
      pending.resolve({ code: 0, data: { id: 1, status: '已取样' } }); await task;
      assert.equal(next.state.rows[0].status, '待取样');
      assert.equal(next.state.messages.length, 0);
      assert.equal(next.env.samplingLock.current, false);
    }
  }
});

test('RD silent correction failure preserves rows and stale queries cannot replace newer rows', async () => {
  const { state, env } = sampleEnvironment();
  state.loading = false;
  Object.assign(env, {
    recordQueryKey: 'query-a', currentQuery: { current: 'query-a' }, query: { start: '2026-09-14', end: '2026-10-13' },
    pageSize: 20, analysisGroupId: undefined, setLoadError: value => { state.error = value; },
    setTotal: value => { state.total = value; }, setPage: () => {}, setSelectedIds: () => {},
    getRdRecords: async () => { throw new Error('offline'); },
  });
  const load = bind(callback('./RdRecordsPage.tsx', 'loadRecords'), env);
  await load(2, true);
  assert.equal(state.rows.length, 2);
  assert.equal(state.error, '');
  assert.equal(state.loading, false);
  assert.deepEqual(state.messages, ['offline']);
  const pending = deferred();
  env.getRdRecords = () => pending.promise;
  const nextLoad = bind(callback('./RdRecordsPage.tsx', 'loadRecords'), env);
  const task = nextLoad(2, true);
  env.currentQuery.current = 'query-b';
  pending.resolve({ code: 0, data: { total: 1, items: [{ id: 9 }] } }); await task;
  assert.equal(state.rows[0].id, 1);
  const sequence = env.recordRequest.current;
  await nextLoad(2, true);
  assert.equal(env.recordRequest.current, sequence, 'old callback must not invalidate the new query');
});

test('RD notification lookup ignores list filters and old paging while retaining analysis laboratory scope', async () => {
  const { state, env } = sampleEnvironment();
  const calls = [];
  Object.assign(env, {
    hasNotificationRecord: true, notificationRecordId: 91, analysisGroupId: 7,
    startDate: '2026-10-01', endDate: '2026-10-15', groupFilter: 99,
    columnFilters: { status: ['待取样'] }, operationFilters: ['recorded'], recorderFilters: ['5'],
    rdFilterQuery: () => { throw Error('temporary filters must not restrict notification lookup'); },
    recordQueryKey: 'target', currentQuery: { current: 'target' }, pageSize: 20,
    setLoadError: value => { state.error = value; }, setTotal: value => { state.total = value; },
    setPage: () => { throw Error('target lookup must not correct normal-list paging'); }, setSelectedIds: () => {},
    getRdRecords: async params => { calls.push(params); return { code: 0, data: { items: [{ id: 91 }], total: 1 } }; },
  });
  env.query = bind(callback('./RdRecordsPage.tsx', 'query'), env)();
  assert.deepEqual(env.query, { record_id: 91, analysis_lab_id: 7 });
  await bind(callback('./RdRecordsPage.tsx', 'loadRecords'), env)(4);
  assert.deepEqual(calls, [{ record_id: 91, analysis_lab_id: 7, page: 1, page_size: 1, sort_by: 'submitted_at', sort_dir: 'desc' }]);
  assert.deepEqual(state.rows, [{ id: 91 }]);
  assert.equal(state.error, '');
});

test('RD missing or invalid notification targets clear stale rows and never broaden into a list query', async () => {
  for (const id of [undefined, 91]) {
    const { state, env } = sampleEnvironment(); let requests = 0;
    Object.assign(env, {
      hasNotificationRecord: true, notificationRecordId: id, analysisGroupId: undefined,
      recordQueryKey: 'target', currentQuery: { current: 'target' }, query: { record_id: id }, pageSize: 20,
      setLoadError: value => { state.error = value; }, setTotal: value => { state.total = value; },
      setPage: () => {}, setSelectedIds: () => {},
      getRdRecords: async () => { requests++; return { code: 0, data: { items: [], total: 0 } }; },
    });
    await bind(callback('./RdRecordsPage.tsx', 'loadRecords'), env)(0);
    assert.equal(requests, id ? 1 : 0);
    assert.deepEqual(state.rows, []); assert.equal(state.total, 0);
    assert.match(state.error, id ? /已删除或当前账号无权查看/ : /编号无效/);
  }
});

test('today loaders preserve Beijing-day and page scope and reject old callback queries', async () => {
  for (const file of ['./EntryPage.tsx', './SampleEntryPage.tsx']) {
    const { state, env } = sampleEnvironment();
    const requested = [];
    Object.assign(env, {
      gid: 7, pageSize: 50, getTodayStr: () => '2026-10-13', isAnalysisPublicAccount: true, detectorId: 9,
      todayViewKey: 'view-a', currentTodayView: env.currentView, panelMounted: env.pageMounted, viewKey: 'view-a',
      setRecordsTotal: () => {},
      getRecords: async params => { requested.push(params); return { code: 0, data: { items: [{ id: 3 }], total: 1 } }; },
    });
    env.getRdRecords = env.getRecords;
    const load = bind(callback(file, 'loadTodayRecords'), env);
    await load();
    assert.equal(requested.length, 1);
    assert.equal(requested[0].group_id, 7);
    assert.equal(requested[0].start, '2026-10-13');
    assert.equal(requested[0].end, '2026-10-13');
    assert.equal(requested[0].page, 3);
    assert.equal(requested[0].page_size, 50);
    if (file.includes('EntryPage') && !file.includes('Sample')) assert.equal(requested[0].subject_user_id, 9);
    env.currentView.current = 'view-b';
    const sequence = env.todayRecordRequest.current;
    await load();
    assert.equal(requested.length, 1);
    assert.equal(env.todayRecordRequest.current, sequence);
    assert.equal(state.rows[0].id, 3);
  }

  const { env } = sampleEnvironment();
  Object.assign(env, { gid: 7, pageSize: 50, isAnalysisPublicAccount: true, detectorId: null,
    todayViewKey: 'view-a', currentTodayView: env.currentView, panelMounted: env.pageMounted, setRecordsTotal: () => {},
    getRecords: () => { throw new Error('public detector must be chosen before any record request'); } });
  await bind(callback('./EntryPage.tsx', 'loadTodayRecords'), env)();
});

test('today refresh at another page only changes the page, avoiding a duplicate direct GET', () => {
  for (const file of ['./EntryPage.tsx', './SampleEntryPage.tsx']) {
    const { env } = sampleEnvironment();
    let pages = [], loads = [];
    Object.assign(env, { todayViewKey: 'view-a', currentTodayView: env.currentView, panelMounted: env.pageMounted,
      viewKey: 'view-a', setSelectedRecordId: () => {}, setRecordsPage: value => { pages.push(value); },
      loadTodayRecords: page => { loads.push(page); } });
    bind(callback(file, 'refreshRecords'), env)();
    assert.deepEqual(pages, [0]); assert.deepEqual(loads, []);
    env.recordsPage = 0;
    bind(callback(file, 'refreshRecords'), env)();
    assert.deepEqual(loads, [0]);
  }
});

test('public summary skips the same successful query, refreshes on data revision, and drops stale responses', async () => {
  const env = { summarySeq: { current: 0 }, summaryLoadedKey: { current: '' }, aggregateQueryKey: 'query:0',
    typeSummaryActive: false, si: '2026-10-01', ei: '2026-10-13', analysisParams: { subject_user_id: 7 },
    setSm: value => { env.summary = value; }, setTrendDetails: value => { env.trend = value; } };
  let requests = 0;
  env.getStatsSummary = async () => { requests += 1; return { code: 0, data: { workload: requests, details: [] } }; };
  const code = effect('./StatsPage.tsx', 'summaryLoadedKey.current === aggregateQueryKey');
  bind(code, env)(); await flush();
  bind(code, env)(); await flush();
  assert.equal(requests, 1);
  env.aggregateQueryKey = 'query:1';
  bind(code, env)(); await flush();
  assert.equal(requests, 2);
  assert.equal(env.summary.workload, 2);
  const pending = deferred();
  env.getStatsSummary = () => pending.promise;
  env.aggregateQueryKey = 'new-query';
  const cleanup = bind(code, env)(); cleanup();
  pending.resolve({ code: 0, data: { workload: 100, details: [] } }); await flush();
  assert.equal(env.summary.workload, 2);
});

test('person log keeps personal permission scope and rejects late responses after query changes', async () => {
  const pending = deferred();
  let requests = 0, rows = [], params;
  const env = { userLogQueryKey: 'person-a', currentUserLogQuery: { current: 'person-a' }, userLogSeq: { current: 0 },
    uf: 'other-user', canViewRoleStats: false, user: { id: 7 }, si: '2026-10-01', ei: '2026-10-13', PS: 100,
    selectedDetectionDivisions: [2], selectedSendingDivisions: [], includePendingOwnership: false,
    setUll: () => {}, setUl: value => { rows = value; }, setUlt: () => {}, setUlp: () => {},
    getRecords: value => { requests += 1; params = value; return pending.promise; } };
  const load = bind(callback('./StatsPage.tsx', 'lul'), env);
  const task = load(3);
  assert.equal(params.subject_user_id, 7);
  assert.equal(params.page, 3);
  assert.equal(params.detection_division_ids, '2');
  env.currentUserLogQuery.current = 'person-b';
  pending.resolve({ code: 0, data: { items: [{ id: 8 }], total: 1 } }); await task;
  assert.deepEqual(rows, []);
  const sequence = env.userLogSeq.current;
  await load(3);
  assert.equal(requests, 1);
  assert.equal(env.userLogSeq.current, sequence);
});

test('RD statistics person records and sender options pass whole-day dates without losing fractional final seconds', async () => {
  const requests = [];
  const env = {
    canViewAllStats: true, currentUserName: 'current-user', uf: 'selected-user',
    s: '2026-10-01', e: '2026-10-26', si: '2026-10-01T00:00:00', ei: '2026-10-26T23:59:59', PS: 100,
    getRdRecordUsers: async params => { requests.push(['users', params]); return { code: 0, data: ['selected-user'] }; },
    getRdRecords: async params => { requests.push(['records', params]); return { code: 0, data: { items: [], total: 0 } }; },
    setUserNames: () => {}, setUll: () => {}, setUl: () => {}, setUlt: () => {}, setUlp: () => {},
  };
  bind(effect('./SampleStatsPage.tsx', 'getRdRecordUsers('), env)();
  await flush();
  await bind(callback('./SampleStatsPage.tsx', 'lul'), env)(2);
  assert.deepEqual(requests, [
    ['users', { start: '2026-10-01', end: '2026-10-26' }],
    ['records', { start: '2026-10-01', end: '2026-10-26', page: 2, page_size: 100, user_name: 'selected-user' }],
  ]);
});

test('focus and visibility share one statistics refresh and hidden pages cancel the scheduled request', () => {
  let now = 10000, previews = 0, summaries = 0, nextTimer = 0;
  const timers = new Map(), listeners = new Map();
  const events = { addEventListener: (name, handler) => listeners.set(name, handler), removeEventListener: name => listeners.delete(name) };
  const env = { window: events, document: { ...events, visibilityState: 'visible' }, Date: { now: () => now },
    lastFocusRefresh: { current: 0 }, ld2: () => { previews += 1; }, invalidateAggregates: () => { summaries += 1; },
    setTimeout: handler => { timers.set(++nextTimer, handler); return nextTimer; }, clearTimeout: id => timers.delete(id) };
  const runTimer = () => { const callbacks = [...timers.values()]; timers.clear(); callbacks.forEach(handler => handler()); };
  const cleanup = bind(effect('./StatsPage.tsx', 'window.addEventListener("focus", refresh)'), env)();
  listeners.get('focus')(); listeners.get('visibilitychange')();
  assert.equal(timers.size, 1); runTimer();
  assert.equal(previews, 1); assert.equal(summaries, 1);
  now += 200; listeners.get('focus')(); runTimer();
  assert.equal(previews, 1);
  now += 1200; listeners.get('focus')(); env.document.visibilityState = 'hidden'; listeners.get('visibilitychange')();
  assert.equal(timers.size, 0);
  env.document.visibilityState = 'visible'; listeners.get('visibilitychange')(); runTimer();
  assert.equal(previews, 2); assert.equal(summaries, 2);
  cleanup(); assert.equal(listeners.size, 0); assert.equal(timers.size, 0);
});

function previewHarness(mobile) {
  let slots = [], cursor = 0, dirty = false, pending = [];
  const same = (a, b) => a && a.length === b.length && a.every((value, index) => Object.is(value, b[index]));
  const env = {
    React: { createElement: (type, props, ...children) => ({ type, props: props || {}, children }) },
    useState: initial => {
      const index = cursor++;
      if (!slots[index]) slots[index] = { value: initial };
      return [slots[index].value, next => { if (!Object.is(slots[index].value, next)) { slots[index].value = next; dirty = true; } }];
    },
    useMemo: fn => { cursor += 1; return fn(); },
    useEffect: (fn, dependencies) => {
      const index = cursor++;
      if (!same(slots[index]?.dependencies, dependencies)) { slots[index] = { dependencies }; pending.push(fn); }
    },
    useTheme: () => ({ breakpoints: { down: value => value } }), useMediaQuery: () => mobile,
    adaptiveTableSx: {}, adaptiveCellSx: () => ({}), getAdaptiveColumnWidths: () => ({}),
  };
  for (const name of ['Table', 'TableBody', 'TableCell', 'TableContainer', 'TableHead', 'TableRow', 'TablePagination',
    'Paper', 'Typography', 'Box', 'CircularProgress', 'Card', 'CardContent']) env[name] = name;
  const component = bind(declaration('../components/PreviewTable.tsx', 'PreviewTable'), env);
  return props => {
    let tree;
    do { dirty = false; cursor = 0; pending = []; tree = component(props); pending.forEach(fn => fn()); } while (dirty);
    return tree;
  };
}
function elements(tree, type) {
  if (!tree) return [];
  if (Array.isArray(tree)) return tree.flatMap(node => elements(node, type));
  return (tree.type === type ? [tree] : []).concat(elements(tree.children, type));
}

test('desktop and mobile previews render 20/50 rows with global indices while full data remains available', () => {
  for (const mobile of [false, true]) {
    const render = previewHarness(mobile);
    const data = Array.from({ length: 63 }, (_, id) => ({ id }));
    const indices = [];
    const props = { title: 'sheet1', columns: [{ field: 'id', headerName: 'ID' }], data,
      getRowKey: row => row.id, renderCell: (row, field, index) => { indices.push(index); return row.id; } };
    let tree = render(props);
    const pagination = () => elements(tree, 'TablePagination')[0].props;
    assert.equal(elements(tree, mobile ? 'Card' : 'TableRow').length, mobile ? 20 : 21);
    assert.equal(elements(tree, mobile ? 'TableContainer' : 'Card').length, 0);
    assert.equal(pagination().count, 63);
    assert.deepEqual(indices, Array.from({ length: 20 }, (_, i) => i));
    indices.length = 0;
    pagination().onPageChange(null, 2); tree = render(props);
    assert.deepEqual(indices, Array.from({ length: 20 }, (_, i) => i + 40));
    indices.length = 0;
    pagination().onRowsPerPageChange({ target: { value: '50' } }); tree = render(props);
    assert.deepEqual(indices, Array.from({ length: 50 }, (_, i) => i));
    assert.equal(data.length, 63);
    pagination().onPageChange(null, 1); tree = render({ ...props, data: data.slice(0, 25) });
    assert.equal(pagination().page, 0);
    assert.equal(pagination().count, 25);
    tree = render(props); pagination().onPageChange(null, 1); tree = render({ ...props, title: 'sheet2' });
    assert.equal(pagination().page, 0);
  }
});
