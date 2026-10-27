import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('WorkloadCoefficientCorrection.tsx', readFileSync(new URL('./WorkloadCoefficientCorrection.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function expression(name) {
  let found;
  const visit = node => {
    if (name === 'effect' && ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect') found = node.arguments[0];
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) found = node.initializer;
    if (!found) ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(found, `missing correction action ${name}`); return found.getText(source);
}
function evaluate(code, environment) {
  const compiled = ts.transpileModule(`const selectedExpression = (${code});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}\nreturn selectedExpression;`)(...Object.values(environment));
}
const number = evaluate(expression('number'), {});
const difference = evaluate(expression('difference'), { number });
const selectionFromPreview = evaluate(expression('selectionFromPreview'), {});
const correctionBatches = evaluate(expression('correctionBatches'), { APPLY_BATCH_SIZE: 100 });
const expiredSelection = evaluate(expression('expiredSelection'), {});
const dateCode = ts.transpileModule(readFileSync(new URL('../utils/recordDateRange.ts', import.meta.url), 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ESNext } }).outputText;
const { validateRecordDateRange } = await import(`data:text/javascript,${encodeURIComponent(dateCode)}`);
const ok = data => ({ code: 0, data });
const row = (id, methodCoefficient = 20) => ({ id, business_no: `WR-${id}`, user_name: 'Test operator', method_name: 'Test method', quantity: 2, coefficient_snapshot: 1, method_coefficient: methodCoefficient, before_workload: 2, after_workload: 2 * methodCoefficient, difference: 2 * methodCoefficient - 2 });
const page = (rows, overrides = {}) => ({ rows, total: rows.length, page: 1, page_size: 50, confirmation_token: 'preview-token', expires_at: new Date(Date.now() + 600000).toISOString(), ...overrides });
const rows = (start, count) => Array.from({ length: count }, (_, index) => row(start + index));
const deferred = () => { let resolve, reject; const promise = new Promise((done, fail) => { resolve = done; reject = fail; }); return { promise, resolve, reject }; };
const resultFor = ids => ({ updated_count: ids.length, before_workload: ids.length * 2, after_workload: ids.length * 40, difference: ids.length * 38 });

function harness() {
  const state = { filters: { start: '2026-09-01', end: '2026-09-30', keyword: '' }, page: 0, pageSize: 50, preview: page([row(1), row(2, 0)]), selected: [], loading: false, saving: false, confirmOpen: false, reason: '', needsPreview: false, error: '', success: '', prepareProgress: null, applyProgress: null };
  const environment = {
    mounted: { current: true }, request: { current: 1 }, previewRequest: { current: 1 }, applying: { current: false }, preparing: { current: false }, invalidated: { current: false }, confirmation: { current: null }, controller: { current: null },
    PAGE_SIZES: [20, 50, 100, 200, 500], PREPARE_PAGE_SIZE: 500, APPLY_BATCH_SIZE: 100,
    number, difference, validateRecordDateRange, selectionFromPreview, correctionBatches, expiredSelection,
    requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); return response.data; },
    previewWorkloadCoefficients: async () => ok(page([row(3)])),
    applyWorkloadCoefficients: async data => ok(resultFor(data.record_ids)),
  };
  for (const key of Object.keys(state)) environment['set' + key[0].toUpperCase() + key.slice(1)] = value => { state[key] = typeof value === 'function' ? value(state[key]) : value; };
  const values = () => ({ ...environment, ...state, batches: correctionBatches(state.selected), allSelected: state.preview?.total > 0 && state.selected.length === state.preview.total });
  const bind = (name, dependencies = {}) => {
    let action;
    action = evaluate(expression(name), { ...values(), ...dependencies, ...(name === 'loadPreview' ? { loadPreview: (...args) => action(...args) } : {}) });
    return action;
  };
  const callbacks = () => {
    const loadPreview = bind('loadPreview');
    return Object.fromEntries(['loadPreview', 'changeFilters', 'changePageSize', 'cancelPreparation', 'selectAllResults', 'selectRows', 'openConfirmation', 'closeConfirmation', 'applyCorrection'].map(name => [name, name === 'loadPreview' ? loadPreview : bind(name, { loadPreview })]));
  };
  const choose = (ids = [1]) => { callbacks().selectRows(ids); callbacks().openConfirmation(); state.reason = 'reviewed history'; };
  return { state, environment, values, bind, callbacks, choose };
}

test('paging preserves choices; filters abort pending requests and ignore old responses', async () => {
  const h = harness(), oldResult = deferred(), currentResult = deferred(), requests = [];
  assert.deepEqual(h.state.selected, []); h.callbacks().selectRows([1]);
  h.environment.previewWorkloadCoefficients = (params, signal) => { requests.push({ params, signal }); return requests.length === 1 ? oldResult.promise : currentResult.promise; };
  const oldLoad = h.callbacks().loadPreview(1);
  assert.deepEqual(h.state.selected.map(item => item.id), [1]);
  h.callbacks().changeFilters({ keyword: '  Test method  ' });
  assert.equal(requests[0].signal.aborted, true); assert.deepEqual(h.state.selected, []); assert.equal(h.state.preview, null); assert.equal(h.state.page, 0);
  const currentLoad = h.callbacks().loadPreview(1);
  currentResult.resolve(ok(page([row(22)], { page: 2, total: 55 }))); await currentLoad;
  oldResult.resolve(ok(page([row(11)]))); await oldLoad;
  assert.deepEqual(h.state.preview.rows.map(item => item.id), [22]); assert.equal(h.state.loading, false);
  assert.deepEqual(requests[1].params, { start: '2026-09-01', end: '2026-09-30', keyword: 'Test method', page: 2, page_size: 50 });
});

test('only offered IDs are individually selectable; choices persist across pages and re-preview clears them', async () => {
  const h = harness(); h.callbacks().selectRows([2, 99, 2]); assert.deepEqual(h.state.selected.map(item => item.id), [2]);
  h.environment.previewWorkloadCoefficients = async params => ok(page([row(51)], { page: params.page, total: 51, confirmation_token: 'page-2' }));
  await h.callbacks().loadPreview(1); h.callbacks().selectRows([51, 999]); assert.deepEqual(h.state.selected.map(item => item.id), [2, 51]);
  h.callbacks().selectRows([]); assert.deepEqual(h.state.selected.map(item => item.id), [2]);
  await h.callbacks().loadPreview(0, true); assert.deepEqual(h.state.selected, []);
  const pages = []; h.environment.previewWorkloadCoefficients = async params => { pages.push(params.page); return ok(page(params.page === 3 ? [] : [row(51)], { page: params.page, total: 51 })); };
  await h.callbacks().loadPreview(2); assert.deepEqual(pages, [3, 2]); assert.equal(h.state.page, 1); assert.equal(h.state.loading, false);
});

test('full selection reads all matching pages without writes and retains only minimal data', async () => {
  const h = harness(), requests = []; let writes = 0; h.state.preview = page(rows(1, 50), { total: 1201 });
  h.environment.previewWorkloadCoefficients = async params => {
    requests.push(params); const start = (params.page - 1) * 500;
    return ok(page(rows(start + 1, Math.min(500, 1201 - start)), { page: params.page, page_size: 500, total: 1201, confirmation_token: `group-${params.page}` }));
  };
  h.environment.applyWorkloadCoefficients = async () => { writes++; return ok(null); };
  await h.callbacks().selectAllResults(); assert.equal(h.state.selected.length, 1201); assert.equal(writes, 0);
  assert.deepEqual(requests.map(params => [params.page, params.page_size]), [[1, 500], [2, 500], [3, 500]]);
  assert.deepEqual(Object.keys(h.state.selected[0]).sort(), ['after_workload', 'before_workload', 'difference', 'expiresAt', 'id', 'token']);
  assert.equal(h.state.preview.rows.length, 50); assert.equal(h.state.prepareProgress, null); assert.equal(h.environment.preparing.current, false);
  assert.deepEqual(correctionBatches(h.state.selected).map(batch => batch.length), [100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 1]);
  await h.callbacks().selectAllResults(); assert.deepEqual(h.state.selected, []); assert.equal(requests.length, 3);
});

test('cancel, filter and page-size changes discard pending all-selection without polluting later results', async () => {
  for (const cancel of ['cancel', 'filter', 'size']) {
    const h = harness(), pending = deferred(); h.state.preview.total = 501; h.callbacks().selectRows([1]);
    let oldSignal; h.environment.previewWorkloadCoefficients = (_, signal) => { oldSignal = signal; return pending.promise; };
    const preparation = h.callbacks().selectAllResults(); assert.equal(h.environment.preparing.current, true);
    if (cancel === 'cancel') h.callbacks().cancelPreparation();
    else if (cancel === 'filter') h.callbacks().changeFilters({ keyword: 'other' });
    else { h.environment.previewWorkloadCoefficients = async params => ok(page([], { page_size: params.page_size })); h.callbacks().changePageSize(200); }
    assert.equal(oldSignal.aborted, true);
    pending.resolve(ok(page(rows(1, 500), { total: 501, page_size: 500 }))); await preparation;
    assert.deepEqual(h.state.selected.map(item => item.id), cancel === 'cancel' ? [1] : []);
    assert.equal(h.state.prepareProgress, null); assert.equal(h.environment.preparing.current, false);
    if (cancel === 'cancel') { h.callbacks().openConfirmation(); assert.equal(h.state.confirmOpen, true); }
    if (cancel === 'filter') assert.equal(h.state.preview, null);
  }
});

test('all-selection rejects total drift, duplicate rows, incomplete pages and expiry without selecting partial results', async () => {
  for (const failure of ['total', 'duplicate', 'incomplete', 'expired']) {
    const h = harness(); h.state.preview.total = 501;
    h.environment.previewWorkloadCoefficients = async params => {
      if (params.page === 1) return ok(page(rows(1, failure === 'incomplete' ? 499 : 500), { total: 501, page_size: 500, ...(failure === 'expired' ? { expires_at: '2000-01-01T00:00:00Z' } : {}) }));
      return ok(page([row(failure === 'duplicate' ? 1 : 501)], { total: failure === 'total' ? 502 : 501, page: 2, page_size: 500 }));
    };
    await h.callbacks().selectAllResults(); assert.deepEqual(h.state.selected, []); assert.equal(h.state.needsPreview, true);
    assert.equal(h.environment.previewRequest.current, null); assert.match(h.state.error, /没有修改记录/); assert.equal(h.state.prepareProgress, null);
  }
});

test('all-selection rejects changed visible coefficients, display details or missing rows instead of approving unseen values', async () => {
  for (const change of ['coefficient', 'details', 'missing']) {
    const h = harness();
    const changed = change === 'coefficient' ? [row(1, 30), row(2, 0)] : change === 'details' ? [{ ...row(1), user_name: 'Changed operator' }, row(2, 0)] : [row(1), row(3)];
    h.environment.previewWorkloadCoefficients = async () => ok(page(changed, { page_size: 500 }));
    await h.callbacks().selectAllResults();
    assert.deepEqual(h.state.selected, []); assert.equal(h.state.needsPreview, true); assert.match(h.state.error, /当前页记录/); assert.equal(h.environment.previewRequest.current, null);
  }
});

test('page sizes are 20/50/100/200/500 with default 50 and clear selection and confirmation on change', async () => {
  assert.deepEqual(evaluate(expression('PAGE_SIZES'), {}), [20, 50, 100, 200, 500]); assert.equal(harness().state.pageSize, 50);
  for (const size of [20, 50, 100, 200, 500]) {
    const h = harness(), requests = []; h.choose();
    h.environment.previewWorkloadCoefficients = async params => { requests.push(params); return ok(page([], { page_size: params.page_size })); };
    const staleApply = h.callbacks().applyCorrection; h.callbacks().changePageSize(size);
    assert.equal(h.state.pageSize, size); assert.deepEqual(h.state.selected, []); assert.equal(h.environment.confirmation.current, null);
    assert.equal(requests[0].page_size, size); assert.equal(requests[0].page, 1); await staleApply(); assert.equal(h.state.success, '');
  }
});

test('explicit current confirmation and reason are mandatory; zero is preserved; closed or expired previews cannot write', async () => {
  const h = harness(); let writes = 0, payload;
  h.environment.applyWorkloadCoefficients = async data => { writes++; payload = data; return ok({ updated_count: 1, before_workload: 2, after_workload: 0, difference: -2 }); };
  await h.callbacks().applyCorrection(); assert.equal(writes, 0); h.callbacks().selectRows([2]); await h.callbacks().applyCorrection(); assert.equal(writes, 0);
  h.callbacks().openConfirmation(); await h.callbacks().applyCorrection(); assert.equal(writes, 0); h.state.reason = 'x'.repeat(501); await h.callbacks().applyCorrection(); assert.equal(writes, 0);
  h.state.reason = '  reviewed method setting  ';
  assert.deepEqual(evaluate(expression('totals'), { selected: h.state.selected, useMemo: fn => fn() }), { before: 2, after: 0, difference: -2 });
  await h.callbacks().applyCorrection(); assert.equal(writes, 1); assert.deepEqual(payload, { confirmation_token: 'preview-token', record_ids: [2], reason: 'reviewed method setting' });
  assert.match(h.state.success, /2 → 0/); assert.match(h.state.success, /-2/); assert.deepEqual(h.state.selected, []); assert.equal(h.state.confirmOpen, false); assert.equal(h.state.reason, '');
  const closed = harness(); closed.choose(); const oldClick = closed.callbacks().applyCorrection; closed.callbacks().closeConfirmation(); await oldClick(); assert.equal(closed.state.success, '');
  for (const expireAt of ['2000-01-01T00:00:00Z', 'invalid']) {
    const expired = harness(); expired.state.preview.expires_at = expireAt; expired.choose(); assert.equal(expired.state.confirmOpen, false); assert.equal(expired.state.needsPreview, true); assert.match(expired.state.error, /过期/);
  }
});

test('token groups apply sequentially in <=100-row batches and report all-selected totals', async () => {
  const h = harness(), writes = [];
  h.state.selected = [...selectionFromPreview(page(rows(1, 250), { confirmation_token: 'group-a' })), ...selectionFromPreview(page(rows(251, 60), { confirmation_token: 'group-b' }))];
  h.callbacks().openConfirmation(); h.state.reason = 'audited';
  h.environment.applyWorkloadCoefficients = async data => { writes.push(data); return ok(resultFor(data.record_ids)); };
  await h.callbacks().applyCorrection();
  assert.deepEqual(writes.map(data => [data.confirmation_token, data.record_ids.length]), [['group-a', 100], ['group-a', 100], ['group-a', 50], ['group-b', 60]]);
  assert.equal(new Set(writes.flatMap(data => data.record_ids)).size, 310); assert.match(h.state.success, /4 批修正 310 条/); assert.match(h.state.success, /620 → 12,400/);
  assert.deepEqual(h.state.selected, []); assert.equal(h.state.saving, false); assert.equal(h.environment.applying.current, false);
});

test('partial conflict or network failure preserves successes, stops later batches and forbids blind retries', async () => {
  for (const failure of ['conflict', 'network']) {
    const h = harness(), pending = deferred(); let writes = 0;
    h.state.selected = selectionFromPreview(page(rows(1, 305))); h.callbacks().openConfirmation(); h.state.reason = 'review history';
    h.environment.applyWorkloadCoefficients = async data => { writes++; if (writes === 1) return ok(resultFor(data.record_ids)); return pending.promise; };
    const action = h.callbacks().applyCorrection, applying = action(); await action(); await Promise.resolve(); assert.equal(writes, 2);
    if (failure === 'conflict') pending.resolve({ code: 1001, message: '记录已变化，请重新预览' }); else pending.reject(Error('network unavailable'));
    await applying; assert.equal(writes, 2); assert.equal(h.state.selected.length, 305); assert.equal(h.state.reason, 'review history'); assert.equal(h.state.confirmOpen, true);
    assert.equal(h.state.needsPreview, true); assert.equal(h.state.saving, false); assert.equal(h.environment.applying.current, false);
    assert.match(h.state.error, /已确认成功 100 条，未确认完成 205 条/); assert.match(h.state.error, /当前批 100 条需核对，后续 105 条未提交/); assert.match(h.state.error, /已成功批次保留/);
    assert.deepEqual(h.state.applyProgress, { batch: 2, batches: 4, completed: 100, total: 305 });
    await action(); await h.callbacks().applyCorrection(); await h.callbacks().loadPreview(0); assert.equal(writes, 2); assert.equal(h.state.needsPreview, true);
    await h.callbacks().loadPreview(0, true); assert.deepEqual(h.state.selected, []); assert.equal(h.state.confirmOpen, false); assert.equal(h.state.needsPreview, false); assert.equal(h.state.error, '');
  }
});

test('expiry after an earlier success stops before the next POST and does not claim rollback', async () => {
  const h = harness(); let writes = 0; h.state.selected = selectionFromPreview(page(rows(1, 150))); h.callbacks().openConfirmation(); h.state.reason = 'audited';
  h.environment.applyWorkloadCoefficients = async data => { writes++; h.state.selected[100].expiresAt = '2000-01-01T00:00:00Z'; return ok(resultFor(data.record_ids)); };
  await h.callbacks().applyCorrection(); assert.equal(writes, 1); assert.match(h.state.error, /已确认成功 100 条，未确认完成 50 条/); assert.match(h.state.error, /当前批 0 条需核对，后续 50 条未提交/); assert.equal(h.state.needsPreview, true);
});

test('invalid dates do not query; unmount aborts reads, ignores late results and prevents later batch writes', async () => {
  const invalid = harness(); let queries = 0; invalid.state.filters.start = '2026-02-30'; invalid.environment.previewWorkloadCoefficients = async () => { queries++; return ok(page([])); };
  await invalid.callbacks().loadPreview(0); assert.equal(queries, 0); assert.match(invalid.state.error, /日期/);
  for (const task of ['preview', 'prepare', 'apply']) {
    const h = harness(), pending = deferred(); let writes = 0; const cleanup = evaluate(expression('effect'), h.values())();
    h.state.preview.total = 150; h.state.selected = selectionFromPreview(page(rows(1, 150))); h.callbacks().openConfirmation(); h.state.reason = 'review';
    h.environment.previewWorkloadCoefficients = () => pending.promise; h.environment.applyWorkloadCoefficients = () => { writes++; return pending.promise; };
    const action = task === 'preview' ? h.callbacks().loadPreview(0) : task === 'prepare' ? (h.state.selected = [], h.callbacks().selectAllResults()) : h.callbacks().applyCorrection();
    cleanup(); const before = structuredClone(h.state); pending.resolve(ok(task === 'apply' ? resultFor(Array.from({ length: 100 }, (_, i) => i)) : page(rows(1, 150), { total: 150, page_size: task === 'prepare' ? 500 : 50 })));
    await action; assert.deepEqual(h.state, before); assert.equal(writes, task === 'apply' ? 1 : 0); if (task !== 'apply') assert.equal(h.environment.controller.current.signal.aborted, true);
  }
});

test('governance correction is gated by backend-confirmed system admins and isolated per account session', () => {
  const pageSource = ts.createSourceFile('DataGovernancePage.tsx', readFileSync(new URL('../pages/DataGovernancePage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let gate; const visit = node => { if (ts.isBinaryExpression(node) && ts.isJsxSelfClosingElement(node.right) && node.right.tagName.getText(pageSource) === 'WorkloadCoefficientCorrection') gate = node; if (!gate) ts.forEachChild(node, visit); };
  visit(pageSource); assert.ok(gate);
  const render = (user, token, summary = { can_correct_workload_coefficients: false }) => evaluate(gate.getText(pageSource), { user, token, summary, React: { createElement: (type, props) => ({ type, props }) }, WorkloadCoefficientCorrection: 'correction' });
  assert.equal(render({ id: 1, is_admin: false, permissions: ['manage:data-governance:purge'] }, 'session-a'), false); assert.equal(render(null, 'session-a'), false); assert.equal(render({ id: 1, is_admin: false }, 'session-a', null), undefined);
  assert.equal(render({ id: 1, is_admin: false }, 'session-a', { can_correct_workload_coefficients: true }).type, 'correction');
  assert.notEqual(render({ id: 1, is_admin: true }, 'session-a').props.key, render({ id: 1, is_admin: true }, 'session-b').props.key); assert.notEqual(render({ id: 1, is_admin: true }, 'session-a').props.key, render({ id: 2, is_admin: true }, 'session-a').props.key);
  const wrapper = pageSource.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === 'DataGovernanceSessionPage'); const returning = wrapper?.body.statements.find(node => ts.isReturnStatement(node)); assert.ok(returning);
  const wrap = (user, token) => evaluate(returning.expression.getText(pageSource), { user, token, React: { createElement: (type, props) => ({ type, props }) }, DataGovernancePage: 'governance' });
  assert.notEqual(wrap({ id: 1 }, 'session-a').props.key, wrap({ id: 2 }, 'session-a').props.key); assert.notEqual(wrap({ id: 1 }, 'session-a').props.key, wrap({ id: 1 }, 'session-b').props.key);
});
