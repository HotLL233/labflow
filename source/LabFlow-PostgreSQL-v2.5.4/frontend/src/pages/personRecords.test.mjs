import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import ts from 'typescript';
import React from 'react';
import dayjs from 'dayjs';
import { renderToStaticMarkup } from 'react-dom/server';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
function loadTs(file, overrides = {}) {
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { fileName: file, compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React, esModuleInterop: true } }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (key in overrides) return overrides[key];
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return loadTs(target + ext, overrides);
    return require(target);
  };
  new Function('require', 'module', 'exports', 'React', compiled)(localRequire, module, module.exports, React);
  return module.exports;
}
const statsPath = fileURLToPath(new URL('./StatsPage.tsx', import.meta.url));
const statsSource = ts.createSourceFile(statsPath, readFileSync(statsPath, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function action(name, environment) {
  let callback;
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.name.getText(statsSource) === name) callback = ts.isCallExpression(node.initializer) && node.initializer.expression.getText(statsSource) === 'useCallback' ? node.initializer.arguments[0] : node.initializer;
    ts.forEachChild(node, visit);
  };
  visit(statsSource); assert.ok(callback, name);
  const code = ts.transpileModule(`const tested = (${callback.getText(statsSource)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  return new Function(...Object.keys(environment), `${code}\nreturn tested;`)(...Object.values(environment));
}
const personModulePath = fileURLToPath(new URL('../api/personRecords.ts', import.meta.url));
const recordsApi = loadTs(personModulePath, { './http': { client: {} } });
const row = (origin, kind = 'auxiliary') => ({ id: 7, origin, kind, user_id: 11, user_name: '检测人员甲', recorded_at: '2026-10-27T08:30:00', department: '分析一部', lab: '', project: '', method: '维护仪器', instrument: '', type: '辅助工作', source: 'auxiliary_work', quantity: 3, coefficient: 0, workload: 0, auxiliary_key: 'item-7', auxiliary_category: '日常维护', auxiliary_area: '液相', notes: '备注保留', status: '有效', created_by_name: '录入账号甲', ownership_status: 'confirmed', normal_record: origin === 'work_record' ? { id: 7, subject_user_id: 11, multiplier: 2, high_item: 'original' } : null });
const writableAuxiliary = () => ({ ...row('auxiliary_record'), auxiliary_maintenance: { updated_at: 'auxiliary-revision', can_delete: true }, maintenance_target: { origin: 'auxiliary_record', id: 7, revision: 'auxiliary-revision', can_edit: true, can_delete: true } });
const writableLegacy = (method = '流动相瓶', id = 7) => ({ ...row('work_record'), id, method, normal_record: { ...row('work_record').normal_record, id }, auxiliary_key: `legacy-method-${id}`, auxiliary_area: '', auxiliary_category: '', maintenance_target: { origin: 'work_record', id, revision: `legacy-${id}-revision`, can_edit: true, can_delete: true } });
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
function descendants(node, predicate) {
  const result = [];
  const visit = children => React.Children.forEach(children, child => {
    if (!React.isValidElement(child)) return;
    if (predicate(child)) result.push(child);
    visit(child.props.children);
  });
  visit(node);
  return result;
}

test('person records use one paged server query and keep same-ID sources distinct when patching detection fields', async () => {
  const calls = [];
  const api = loadTs(personModulePath, { './http': { client: { get: async (...args) => { calls.push(args); return { data: { code: 0 } }; } } } });
  const query = { start: '2026-10-01', end: '2026-10-31', kind: 'auxiliary', subject_user_id: 11, page: 3, page_size: 50 };
  await api.getPersonRecords(query);
  assert.deepEqual(calls, [['/stats/person-records', { params: query }]]);
  const normal = row('work_record', 'detection'), auxiliary = row('auxiliary_record'), legacy = { ...row('work_record'), id: 8 };
  assert.notEqual(api.personRecordKey(normal), api.personRecordKey(auxiliary));
  const updated = api.patchDetectionRecord([normal, auxiliary, legacy], 7, { high_item: 'new', multiplier: 0 });
  assert.equal(updated[0].normal_record.high_item, 'new'); assert.equal(updated[0].normal_record.multiplier, 0);
  assert.equal(updated[1], auxiliary); assert.equal(updated[2], legacy);
});

test('auxiliary deletion accepts native and historical server targets but requires permission and a saved revision', () => {
  assert.equal(recordsApi.canDeleteAuxiliaryRecord(writableAuxiliary()), true);
  assert.equal(recordsApi.canDeleteAuxiliaryRecord(writableLegacy()), true);
  for (const record of [
    row('auxiliary_record'),
    { ...writableAuxiliary(), kind: 'detection' },
    { ...writableAuxiliary(), maintenance_target: null },
    { ...writableLegacy(), maintenance_target: { origin: 'work_record', id: 7, revision: 'revision', can_delete: false } },
    { ...writableAuxiliary(), maintenance_target: { origin: 'auxiliary_record', id: 7, revision: ' ', can_delete: true } },
  ]) assert.equal(recordsApi.canDeleteAuxiliaryRecord(record), false);
});

function deleteHarness(overrides = {}) {
  const state = { record: writableAuxiliary(), error: '', writes: [], invalidations: 0, loads: [] };
  const environment = {
    auxiliaryDeleteRecord: state.record, auxiliaryDeletePending: { current: false }, canDeleteAuxiliaryRecord: recordsApi.canDeleteAuxiliaryRecord, ulp: 2,
    setAuxiliaryDeleteRecord: value => { state.record = value; }, setAuxiliaryDeleteError: value => { state.error = value; }, setAuxiliaryDeleting: value => { state.deleting = value; },
    deleteRecordTarget: async (...args) => { state.writes.push(args); }, invalidateAggregates: () => { state.invalidations++; }, lul: async page => { state.loads.push(page); },
    deleteRecord: () => { throw Error('must not delete the normal record with the same ID'); }, ...overrides,
  };
  return { state, environment, remove: reason => action('deleteAuxiliaryRecord', environment)(reason) };
}

test('statistics auxiliary delete validates reason and source, preserves failures for retry, and refreshes after success', async () => {
  for (const reason of [undefined, '', '   ']) {
    const h = deleteHarness(); await h.remove(reason);
    assert.equal(h.state.writes.length, 0); assert.equal(h.state.invalidations, 0); assert.ok(h.state.record); assert.match(h.state.error, /删除原因/);
  }
  for (const auxiliaryDeleteRecord of [null, row('auxiliary_record'), { ...writableLegacy(), maintenance_target: null }]) {
    const h = deleteHarness({ auxiliaryDeleteRecord }); await h.remove('原因');
    assert.equal(h.state.writes.length, 0); assert.equal(h.state.invalidations, 0);
  }
  const h = deleteHarness(); await h.remove('  录入重复  ');
  assert.deepEqual(h.state.writes, [[writableAuxiliary().maintenance_target, '录入重复']]);
  assert.equal(h.state.record, null); assert.equal(h.state.invalidations, 1); assert.deepEqual(h.state.loads, [2]);
  assert.equal(h.state.deleting, false); assert.equal(h.environment.auxiliaryDeletePending.current, false);
  const failed = deleteHarness({ deleteRecordTarget: async () => { throw Error('记录已更新，请刷新'); } });
  await failed.remove('原因');
  assert.ok(failed.state.record); assert.match(failed.state.error, /记录已更新/);
  assert.equal(failed.state.invalidations, 0); assert.deepEqual(failed.state.loads, []);
  assert.equal(failed.state.deleting, false); assert.equal(failed.environment.auxiliaryDeletePending.current, false);
  failed.environment.deleteRecordTarget = async (...args) => { failed.state.writes.push(args); };
  await failed.remove('重新核对');
  assert.equal(failed.state.record, null); assert.equal(failed.state.writes.length, 1);
});

test('statistics auxiliary delete blocks repeated confirmation and opening another row until it finishes', async () => {
  const pending = deferred(), calls = [];
  const h = deleteHarness({ deleteRecordTarget: (...args) => { calls.push(args); return pending.promise; } });
  const task = h.remove('录入重复');
  await h.remove('再次点击');
  action('openAuxiliaryDelete', h.environment)(writableAuxiliary());
  assert.equal(calls.length, 1); assert.equal(h.state.deleting, true);
  pending.resolve(); await task;
  assert.equal(h.state.invalidations, 1); assert.equal(h.environment.auxiliaryDeletePending.current, false);
  action('openAuxiliaryDelete', h.environment)({ ...writableLegacy(), maintenance_target: null });
  assert.equal(h.state.record, null);
  action('openAuxiliaryDelete', h.environment)(writableAuxiliary());
  assert.equal(h.state.record.id, 7);
});

test('statistics historical and linked rows pass their authoritative target instead of the displayed row ID', async () => {
  for (const record of [
    writableLegacy('水相'),
    { ...writableAuxiliary(), id: 42, maintenance_target: { origin: 'work_record', id: 99, revision: 'original-record-revision', can_delete: true } },
  ]) {
    const h = deleteHarness({ auxiliaryDeleteRecord: record });
    await h.remove('历史误录');
    assert.deepEqual(h.state.writes, [[record.maintenance_target, '历史误录']]);
    assert.equal(h.state.invalidations, 1); assert.deepEqual(h.state.loads, [2]);
  }
});

test('statistics native and historical edit snapshots keep identity and coefficient; saving outside the current range opens the saved day', async () => {
  const selected = [];
  for (const row of [writableAuxiliary(), writableLegacy('已停用旧事项')]) {
    action('openAuxiliaryEdit', { auxiliaryDeletePending: { current: false }, canEditAuxiliaryRecord: recordsApi.canEditAuxiliaryRecord, setAuxiliaryEditRecord: value => selected.push(value) })(row);
    assert.equal(selected.at(-1).maintenance_target, row.maintenance_target);
    assert.equal(selected.at(-1).item_name, row.method);
    assert.equal(selected.at(-1).coefficient, 0);
    assert.equal(selected.at(-1).user_name, row.user_name);
  }
  action('openAuxiliaryEdit', { auxiliaryDeletePending: { current: false }, canEditAuxiliaryRecord: recordsApi.canEditAuxiliaryRecord, setAuxiliaryEditRecord: value => selected.push(value) })({ ...writableLegacy(), maintenance_target: null });
  assert.equal(selected.length, 2);
  for (const recorded_at of ['2026-09-30T23:59:00+08:00', '2026-10-12T08:30:00+08:00']) {
    const state = { invalidations: 0, loads: [] };
    await action('auxiliaryMetadataSaved', {
      dayjs, s: '2026-10-01', e: '2026-10-31', ulp: 3,
      invalidateAggregates: () => { state.invalidations++; }, lul: async page => state.loads.push(page),
      setS: value => { state.start = value; }, setE: value => { state.end = value; }, setUlp: value => { state.page = value; },
    })({ quantity: 4, recorded_at, notes: '备注' });
    assert.equal(state.invalidations, 1);
    if (recorded_at.startsWith('2026-09-30')) {
      assert.equal(state.start, '2026-09-30'); assert.equal(state.end, '2026-09-30'); assert.equal(state.page, 1); assert.deepEqual(state.loads, []);
    } else assert.deepEqual(state.loads, [3]);
  }
});

function logHarness(overrides = {}) {
  const state = { rows: [], total: 0, page: 1, error: '' }, pending = deferred();
  const environment = {
    ac: 'auxiliary-log', PS: 50, uf: '', canViewRoleStats: true, user: { id: 11 }, si: '2026-10-01', ei: '2026-10-31',
    validRange: true, personLogActive: true, canOpenTab: () => true,
    selectedDetectionDivisions: [2], selectedSendingDivisions: [], includePendingOwnership: false,
    userLogQueryKey: 'current', currentUserLogQuery: { current: 'current' }, userLogSeq: { current: 0 },
    setUll: value => { state.loading = value; }, setUl: value => { state.rows = value; }, setUlt: value => { state.total = value; }, setUlp: value => { state.page = value; },
    setPersonRowsQuery: value => { state.loadedQuery = value; }, setEr: value => { state.error = value; },
    getPersonRecords: query => { state.query = query; return pending.promise; }, ...overrides,
  };
  return { state, pending, environment, load: () => action('lul', environment) };
}

test('restoring a report and person together keeps the URL person after both actual page effects run', () => {
  const effects = [];
  const visit = node => {
    if (ts.isCallExpression(node) && node.expression.getText(statsSource) === 'useEffect') {
      const callback = node.arguments[0].getText(statsSource);
      if (callback.includes("const subject = canChoosePerson ? searchParams.get('subject_user_id')") || callback.includes('setFLab(""); setFProject("");')) effects.push(callback);
    }
    ts.forEachChild(node, visit);
  };
  visit(statsSource);
  assert.equal(effects.length, 2);
  const state = { uf: '8', fUser: '8', page: 3, lab: '旧实验室', project: '旧项目', instrument: '旧仪器', method: '旧方法', group: 2 };
  const environment = {
    canChoosePerson: true, searchParams: new URLSearchParams('report=user-log&subject_user_id=7'), user: { id: 1 }, fUser: '8',
    setUf: value => { state.uf = value; }, setFUser: value => { state.fUser = value; }, setUlp: value => { state.page = value; },
    setFLab: value => { state.lab = value; }, setFProject: value => { state.project = value; },
    setFInstrument: value => { state.instrument = value; }, setFMethod: value => { state.method = value; }, setGf: value => { state.group = value; },
  };
  for (const callback of effects) {
    const compiled = ts.transpileModule(`const effect = (${callback}); effect();`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
    new Function(...Object.keys(environment), compiled)(...Object.values(environment));
  }
  assert.deepEqual(state, { uf: '7', fUser: '7', page: 1, lab: '', project: '', instrument: '', method: '', group: 0 });
});

test('correcting an impossible date to its normalized valid day restarts aggregate and filter effects', async () => {
  const markers = ['summaryLoadedKey.current === aggregateQueryKey', 'breakdownLoadedKey.current === aggregateQueryKey', 'getExportFilterOptions({', 'setPersonOptions(response.data || [])'];
  const effects = [];
  const visit = node => {
    if (ts.isCallExpression(node) && node.expression.getText(statsSource) === 'useEffect' && markers.some(marker => node.arguments[0].getText(statsSource).includes(marker))) effects.push(node);
    ts.forEachChild(node, visit);
  };
  visit(statsSource);
  assert.equal(effects.length, 4);
  const { validRecordDate } = loadTs(fileURLToPath(new URL('../utils/recordDateRange.ts', import.meta.url)));
  const invalidStart = '2026-02-30', validStart = '2026-03-02';
  assert.equal(dayjs(invalidStart).format('YYYY-MM-DD'), validStart);
  let requests = 0, summary;
  const environment = {
    validRange: validRecordDate(invalidStart), si: dayjs(invalidStart).format('YYYY-MM-DDTHH:mm:ss'), ei: '2026-03-05T23:59:59',
    typeSummaryActive: false, analysisParams: {}, aggregateQueryKey: 'same-normalized-range',
    selectedDetectionDivisions: [], selectedSendingDivisions: [], includePendingOwnership: false, dataRevision: 0,
    summarySeq: { current: 0 }, summaryLoadedKey: { current: '' },
    getStatsSummary: async () => { requests++; return { code: 0, data: { workload: 17, details: [] } }; },
    setSm: value => { summary = value; }, setTrendDetails: () => {},
  };
  const evaluate = expression => {
    const compiled = ts.transpileModule(`const result = (${expression});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
    return new Function(...Object.keys(environment), `${compiled}\nreturn result;`)(...Object.values(environment));
  };
  const previousDependencies = effects.map(node => evaluate(node.arguments[1].getText(statsSource)));
  const summaryEffect = effects.find(node => node.arguments[0].getText(statsSource).includes(markers[0]));
  evaluate(summaryEffect.arguments[0].getText(statsSource))();
  assert.equal(requests, 0);
  environment.validRange = validRecordDate(validStart);
  effects.forEach((node, index) => assert.notDeepEqual(evaluate(node.arguments[1].getText(statsSource)), previousDependencies[index]));
  evaluate(summaryEffect.arguments[0].getText(statsSource))();
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(requests, 1);
  assert.equal(summary.workload, 17);
});

test('person-record loading skips invalid dates, inactive reports and unauthorized record tabs before issuing a request', async () => {
  for (const overrides of [
    { validRange: false, si: '2026-02-30', ei: '2026-02-28' },
    { validRange: false, si: '2026-10-31', ei: '2026-10-01' },
    { ac: 'user-log', canOpenTab: () => false },
    { ac: 'auxiliary-log', canOpenTab: () => false },
    { ac: 'sheet1', personLogActive: false },
    { ac: null },
  ]) {
    let requests = 0;
    const h = logHarness({ ...overrides, getPersonRecords: async () => { requests++; return { code: 0, data: { items: [], total: 0 } }; } });
    await h.load()(1);
    assert.equal(requests, 0);
    assert.equal(h.environment.userLogSeq.current, 0);
    assert.equal(h.state.loading, undefined);
    assert.deepEqual(h.state.rows, []);
    assert.equal(h.state.loadedQuery, undefined);
  }
});

test('auxiliary statistics keep server kind/person/department pagination and discard late old-scope results', async () => {
  const h = logHarness({ selectedSendingDivisions: [9] });
  const task = h.load()(2, '12');
  assert.deepEqual(h.state.query, { start: '2026-10-01', end: '2026-10-31', page: 2, page_size: 50, detection_division_ids: '2', sending_division_ids: undefined, include_pending_ownership: undefined, kind: 'auxiliary', subject_user_id: 12 });
  h.environment.currentUserLogQuery.current = 'new';
  h.pending.resolve({ code: 0, data: { items: [row('auxiliary_record')], total: 101 } }); await task;
  assert.deepEqual(h.state.rows, []);
});

test('person-record paging corrects a removed last page without showing a false empty result', async () => {
  const retries = [];
  const h = logHarness({ lul: async (...args) => retries.push(args) });
  const task = h.load()(7, '12');
  h.pending.resolve({ code: 0, data: { items: [], total: 51 } }); await task;
  assert.deepEqual(retries, [[2, '12']]);
  assert.deepEqual(h.state.rows, []); assert.equal(h.state.loadedQuery, undefined);
});

test('person-record reads force ordinary ownership but preserve public-created and supervisor filtering', async () => {
  for (const [user, canViewRoleStats, expected] of [[{ id: 11 }, false, 11], [{ id: 99, is_analysis_public_account: true }, false, undefined], [{ id: 11 }, true, undefined]]) {
    const h = logHarness({ ac: 'user-log', user, canViewRoleStats });
    const task = h.load()();
    assert.equal(h.state.query.subject_user_id, expected); assert.equal(h.state.query.kind, undefined);
    h.pending.resolve({ code: 0, data: { items: [row('auxiliary_record')], total: 1 } }); await task;
    assert.equal(h.state.rows.length, 1); assert.equal(h.state.loadedQuery, 'current');
  }
});

test('public-created record views do not show a selector backed by the old personal-only candidates', () => {
  assert.equal(action('renderToolbarFilters', { ac: 'auxiliary-log', personLogActive: true, user: { is_analysis_public_account: true }, canViewRoleStats: false })(), null);
});

test('statistics-only roles cannot mutate normal records while existing owner/supervisor/public writes remain gated', () => {
  const hasPermissionKey = (permissions, key) => permissions.includes('*') || permissions.includes(key);
  const record = { subject_user_id: 11, created_by_user_id: 99 };
  const cases = [
    [{ id: 11, permissions: ['stats:portal:workload', 'stats:workload:user-log'] }, true, false],
    [{ id: 11, permissions: ['entry:workload'] }, false, true],
    [{ id: 12, permissions: ['entry:workload'] }, false, false],
    [{ id: 99, is_analysis_public_account: true, permissions: ['entry:workload'] }, false, true],
    [{ id: 100, is_analysis_public_account: true, permissions: ['entry:workload'] }, false, false],
    [{ id: 12, is_admin: true, permissions: [] }, true, true],
  ];
  for (const [user, canViewRoleStats, expected] of cases) assert.equal(action('canMutateDetection', { user, canViewRoleStats, hasPermissionKey })(record), expected);
});

test('mixed auxiliary rows preserve snapshots and show deletion according to each source permission', () => {
  const selected = [];
  const edits = [];
  const renderRow = action('renderAuxiliaryPersonRow', { React, TableRow: mui.TableRow, TableCell: mui.TableCell, Typography: mui.Typography, Chip: mui.Chip, Button: mui.Button, Box: mui.Box, personRecordKey: recordsApi.personRecordKey, canDeleteAuxiliaryRecord: recordsApi.canDeleteAuxiliaryRecord, canEditAuxiliaryRecord: recordsApi.canEditAuxiliaryRecord, auxiliaryDeleting: false, openAuxiliaryDelete: record => selected.push(record), openAuxiliaryEdit: record => edits.push(record), ulp: 2, PS: 100, userLogNowrapCellSx: {}, userLogWrapCellSx: {}, adaptiveDateCellSx: {}, chipSx: {}, formatDateTimeDisplay: value => value });
  const html = renderToStaticMarkup(React.createElement(mui.Table, null, React.createElement(mui.TableBody, null, renderRow(row('auxiliary_record'), 0))));
  for (const value of ['维护仪器', '检测人员甲', '系数 0', '工作量 0', '液相', '日常维护', '无维护权限或记录不可维护', '101']) assert.ok(html.includes(value), value);
  assert.equal(html.includes('<input'), false); assert.equal(html.includes('<button'), false);
  assert.equal(html.includes('item-7'), false);
  const writable = writableAuxiliary();
  const buttons = descendants(renderRow(writable, 0), child => child.type === mui.Button && child.props.children === '删除');
  assert.equal(buttons.length, 1); assert.equal(buttons[0].props.children, '删除');
  buttons[0].props.onClick(); assert.deepEqual(selected, [writable]);
  for (const method of ['流动相瓶', '水相', '小瓶']) {
    const historical = writableLegacy(method);
    const historicalButtons = descendants(renderRow(historical, 0), child => child.type === mui.Button);
    assert.equal(historicalButtons.length, 2);
    historicalButtons.find(button => button.props.children === '编辑').props.onClick();
    historicalButtons.find(button => button.props.children === '删除').props.onClick();
    assert.equal(selected.at(-1), historical);
    assert.equal(edits.at(-1), historical);
  }
  assert.equal(descendants(renderRow({ ...writableLegacy(), maintenance_target: null }, 0), child => child.type === mui.Button).length, 0);
});

test('auxiliary card uses the existing statistics permission and saved style identities', () => {
  const config = loadTs(fileURLToPath(new URL('./statsConfig.tsx', import.meta.url)));
  assert.equal(config.STAT_CARDS.filter(card => card.key === 'auxiliary-log').length, 1);
  assert.equal(config.STAT_CARD_PERMISSIONS['auxiliary-log'], 'stats:workload:user-log');
  const panel = loadTs(fileURLToPath(new URL('../components/AuxiliaryStatsRecords.tsx', import.meta.url)), {
    '../api/personRecords': recordsApi, './recordTable/useRecordTableStyle': { useRecordTableStyle: () => {} },
    './recordTable/useCardLayout': { useCardLayout: () => false }, './recordTable/TableStyleBar': { __esModule: true, default: () => null },
    './recordTable/RecordCardSummary': { __esModule: true, default: () => null },
  });
  assert.equal(panel.auxiliaryStatsStyleKey(row('auxiliary_record')), 'auxiliary:7');
  assert.equal(panel.auxiliaryStatsStyleKey(row('work_record')), 'legacy_work_record:7');
  const values = panel.auxiliaryStatsValues(row('auxiliary_record'));
  assert.equal(values.coefficient, 0); assert.equal(values.workload, 0);
  assert.equal(values.auxiliary_work_name, '维护仪器'); assert.equal(values.area_name, '液相');
  assert.equal(values.notes, '备注保留'); assert.equal(values.status, '有效'); assert.equal(values.created_by_name, '录入账号甲');
  for (const key of ['project', 'lab', 'multiplier', 'item_id']) assert.equal(key in values, false);
});

test('auxiliary statistics reuse the saved table/card style on desktop and mobile without entry controls', () => {
  const styles = loadTs(fileURLToPath(new URL('../utils/recordTableStyle.ts', import.meta.url)));
  for (const card of [false, true]) {
    const spec = { ...styles.DEFAULT_RECORD_TABLE_STYLE, hiddenColumns: ['quantity'] };
    const observed = [];
    const style = { spec, resolver: styles.createStyleResolver(spec, {}), tableSx: styles.tableSxFromSpec(spec), setPageSize: () => {} };
    const hooks = { ...React, useMemo: callback => callback(), useEffect: () => {} };
    const panel = loadTs(fileURLToPath(new URL('../components/AuxiliaryStatsRecords.tsx', import.meta.url)), {
      react: hooks, '../api/personRecords': recordsApi,
      './recordTable/useRecordTableStyle': { useRecordTableStyle: (key, values) => { observed.push([key, values]); return style; } },
      './recordTable/useCardLayout': { useCardLayout: () => card },
      './recordTable/TableStyleBar': { __esModule: true, default: () => null },
    }).default;
    const html = renderToStaticMarkup(panel({ rows: [row('auxiliary_record')], loading: false, total: 1, page: 1, pageSize: 50, onPageChange: () => {}, onPageSizeChange: () => {} }));
    for (const value of ['维护仪器', '检测人员甲', '分析一部', '液相', '日常维护']) assert.ok(html.includes(value), value);
    assert.equal(observed[0][0], 'auxiliary_work_records');
    assert.equal(observed[0][1]['auxiliary:7'].coefficient, '0');
    assert.equal(html.includes('item-7'), false); assert.equal(html.includes('单价倍率'), false);
    if (card) assert.ok(html.includes('data-summary-row'));
    else { assert.ok(html.includes('<colgroup>')); assert.equal(html.includes('>数量</th>'), false); }
  }
});

test('auxiliary statistics delete controls preserve source, permissions and configured desktop/mobile button styles', () => {
  const styles = loadTs(fileURLToPath(new URL('../utils/recordTableStyle.ts', import.meta.url)));
  for (const card of [false, true]) for (const deleting of [false, true]) {
    const palette = { idle: { color: '#be123c', bgColor: '#fff1f2', borderColor: '#9f1239' }, loading: { color: '#444444' } };
    const spec = { ...styles.DEFAULT_RECORD_TABLE_STYLE, table: { ...styles.DEFAULT_RECORD_TABLE_STYLE.table, actionFontSize: 11, actionButtonWidth: 68 }, actionColors: { delete: palette } };
    const selected = [], writable = writableAuxiliary();
    const style = { spec, resolver: styles.createStyleResolver(spec, {}), tableSx: styles.tableSxFromSpec(spec), setPageSize: () => {} };
    const panel = loadTs(fileURLToPath(new URL('../components/AuxiliaryStatsRecords.tsx', import.meta.url)), {
      react: { ...React, useMemo: callback => callback(), useEffect: () => {} }, '../api/personRecords': recordsApi,
      './recordTable/useRecordTableStyle': { useRecordTableStyle: () => style }, './recordTable/useCardLayout': { useCardLayout: () => card },
      './recordTable/TableStyleBar': { __esModule: true, default: () => null },
    }).default;
    const props = { rows: [writable, row('auxiliary_record'), { ...writableLegacy(), maintenance_target: null }], loading: false, total: 3, page: 1, pageSize: 50, onPageChange: () => {}, onPageSizeChange: () => {}, onDelete: record => selected.push(record), deleting };
    const buttons = descendants(panel(props), child => child.type === mui.Button && child.props.children === '删除');
    assert.equal(buttons.length, 1);
    const button = buttons[0];
    assert.equal(button.props.disabled, deleting); assert.equal(button.props.sx.fontSize, 11); assert.equal(button.props.sx.width, 68);
    assert.equal(button.props.sx.minHeight, card ? 40 : 28); assert.equal(button.props.sx.color, palette[deleting ? 'loading' : 'idle'].color);
    if (!deleting) { button.props.onClick(); assert.deepEqual(selected, [writable]); }
    style.spec = { ...spec, hiddenColumns: ['_action'] };
    assert.equal(descendants(panel(props), child => child.type === mui.Button && child.props.children === '删除').length, 0);
  }
});

test('the screenshot historical auxiliary items expose delete controls on desktop and mobile without requiring mappings or labs', () => {
  const styles = loadTs(fileURLToPath(new URL('../utils/recordTableStyle.ts', import.meta.url)));
  const rows = ['流动相瓶', '水相', '小瓶'].map((name, index) => writableLegacy(name, 10 + index));
  for (const card of [false, true]) {
    const selected = [], edited = [];
    const spec = styles.DEFAULT_RECORD_TABLE_STYLE;
    const style = { spec, resolver: styles.createStyleResolver(spec, {}), tableSx: styles.tableSxFromSpec(spec), setPageSize: () => {} };
    const panel = loadTs(fileURLToPath(new URL('../components/AuxiliaryStatsRecords.tsx', import.meta.url)), {
      react: { ...React, useMemo: callback => callback(), useEffect: () => {} }, '../api/personRecords': recordsApi,
      './recordTable/useRecordTableStyle': { useRecordTableStyle: () => style }, './recordTable/useCardLayout': { useCardLayout: () => card },
      './recordTable/TableStyleBar': { __esModule: true, default: () => null },
    }).default;
    const tree = panel({ rows, loading: false, total: 3, page: 1, pageSize: 50, onPageChange: () => {}, onPageSizeChange: () => {}, onEdit: record => edited.push(record), onDelete: record => selected.push(record), deleting: false });
    const html = renderToStaticMarkup(tree);
    for (const name of ['流动相瓶', '水相', '小瓶']) assert.ok(html.includes(name), name);
    assert.equal(html.includes('历史辅助快照，只读'), false);
    const buttons = descendants(tree, child => child.type === mui.Button && child.props.children === '删除');
    assert.equal(buttons.length, 3);
    buttons.forEach(button => button.props.onClick());
    assert.deepEqual(selected, rows);
    const editButtons = descendants(tree, child => child.type === mui.Button && child.props.children === '编辑');
    assert.equal(editButtons.length, 3); editButtons.forEach(button => button.props.onClick());
    assert.deepEqual(edited, rows);
  }
});

test('saved note/status/creator-only summaries and hidden columns keep auxiliary cards nonempty and conditions intact', () => {
  const styles = loadTs(fileURLToPath(new URL('../utils/recordTableStyle.ts', import.meta.url)));
  const allKeys = ['recorded_at', 'user_name', 'detection_division_name', 'auxiliary_work_name', 'area_name', 'category', 'quantity', 'coefficient', 'workload', 'notes', 'status', 'created_by_name'];
  const fixtures = [row('auxiliary_record'), { ...row('work_record'), notes: '历史备注保留', status: '历史有效', created_by_name: '原录入账号' }];
  for (const card of [false, true]) for (const only of ['notes', 'status', 'created_by_name']) {
    const spec = {
      ...styles.DEFAULT_RECORD_TABLE_STYLE,
      hiddenColumns: allKeys.filter(key => key !== only),
      card: { ...styles.DEFAULT_RECORD_TABLE_STYLE.card, summaryFields: [[only], [], [], []] },
      rules: [{ id: 'saved-note-condition', name: '原备注规则', enabled: true, range: 'column', column: only, match: 'all',
        conditions: [{ field: 'notes', op: 'contains', value: '备注保留' }, { field: 'status', op: 'eq', value: '有效' }, { field: 'created_by_name', op: 'eq', value: '录入账号甲' }], style: { color: '#123456' } }],
    };
    let observed;
    const hooks = { ...React, useMemo: callback => callback(), useEffect: () => {} };
    const panel = loadTs(fileURLToPath(new URL('../components/AuxiliaryStatsRecords.tsx', import.meta.url)), {
      react: hooks, '../api/personRecords': recordsApi,
      './recordTable/useRecordTableStyle': { useRecordTableStyle: (key, values) => {
        observed = { key, values };
        return { spec, resolver: styles.createStyleResolver(spec, values), tableSx: styles.tableSxFromSpec(spec), setPageSize: () => {} };
      } },
      './recordTable/useCardLayout': { useCardLayout: () => card },
      './recordTable/TableStyleBar': { __esModule: true, default: () => null },
    }).default;
    const html = renderToStaticMarkup(panel({ rows: fixtures, loading: false, total: 2, page: 1, pageSize: 50, onPageChange: () => {}, onPageSizeChange: () => {} }));
    assert.ok(html.includes(fixtures[0][only]), `${card ? 'card' : 'table'} native ${only}`);
    assert.ok(html.includes(fixtures[1][only]), `${card ? 'card' : 'table'} legacy ${only}`);
    assert.equal(observed.key, 'auxiliary_work_records');
    assert.equal(observed.values['auxiliary:7'].notes, fixtures[0].notes);
    assert.equal(observed.values['legacy_work_record:7'].created_by_name, fixtures[1].created_by_name);
    assert.equal(styles.resolveCellStyle(spec, { columnKey: only, rowKey: 'auxiliary:7', rowValues: observed.values['auxiliary:7'] }).style.color, '#123456');
    assert.ok(html.includes('#123456'), 'the saved conditional style reaches the rendered field');
    if (card) assert.ok(html.includes('data-summary-row="1"'));
  }
});
