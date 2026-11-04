import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
function loadTs(file, overrides = {}) {
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React, esModuleInterop: true } }).outputText;
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
const writableAuxiliary = () => ({ ...row('auxiliary_record'), auxiliary_maintenance: { updated_at: 'auxiliary-revision', can_delete: true } });
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

test('auxiliary deletion requires the independent source, server permission and a saved revision', () => {
  assert.equal(recordsApi.canDeleteAuxiliaryRecord(writableAuxiliary()), true);
  for (const record of [
    row('auxiliary_record'),
    { ...writableAuxiliary(), origin: 'work_record' },
    { ...writableAuxiliary(), kind: 'detection' },
    { ...writableAuxiliary(), auxiliary_maintenance: null },
    { ...writableAuxiliary(), auxiliary_maintenance: { updated_at: 'revision', can_delete: false } },
    { ...writableAuxiliary(), auxiliary_maintenance: { updated_at: ' ', can_delete: true } },
  ]) assert.equal(recordsApi.canDeleteAuxiliaryRecord(record), false);
});

function deleteHarness(overrides = {}) {
  const state = { record: writableAuxiliary(), error: '', writes: [], invalidations: 0, loads: [] };
  const environment = {
    auxiliaryDeleteRecord: state.record, auxiliaryDeletePending: { current: false }, canDeleteAuxiliaryRecord: recordsApi.canDeleteAuxiliaryRecord, ulp: 2,
    setAuxiliaryDeleteRecord: value => { state.record = value; }, setAuxiliaryDeleteError: value => { state.error = value; }, setAuxiliaryDeleting: value => { state.deleting = value; },
    changeAuxiliaryState: async (...args) => { state.writes.push(args); }, invalidateAggregates: () => { state.invalidations++; }, lul: async page => { state.loads.push(page); },
    deleteRecord: () => { throw Error('must not delete the normal record with the same ID'); }, ...overrides,
  };
  return { state, environment, remove: reason => action('deleteAuxiliaryRecord', environment)(reason) };
}

test('statistics auxiliary delete validates reason and source, preserves failures for retry, and refreshes after success', async () => {
  for (const reason of [undefined, '', '   ']) {
    const h = deleteHarness(); await h.remove(reason);
    assert.equal(h.state.writes.length, 0); assert.equal(h.state.invalidations, 0); assert.ok(h.state.record); assert.match(h.state.error, /删除原因/);
  }
  for (const auxiliaryDeleteRecord of [null, row('auxiliary_record'), { ...writableAuxiliary(), origin: 'work_record' }]) {
    const h = deleteHarness({ auxiliaryDeleteRecord }); await h.remove('原因');
    assert.equal(h.state.writes.length, 0); assert.equal(h.state.invalidations, 0);
  }
  const h = deleteHarness(); await h.remove('  录入重复  ');
  assert.deepEqual(h.state.writes, [['records', 7, false, 'auxiliary-revision', '录入重复']]);
  assert.equal(h.state.record, null); assert.equal(h.state.invalidations, 1); assert.deepEqual(h.state.loads, [2]);
  assert.equal(h.state.deleting, false); assert.equal(h.environment.auxiliaryDeletePending.current, false);
  const failed = deleteHarness({ changeAuxiliaryState: async () => { throw Error('记录已更新，请刷新'); } });
  await failed.remove('原因');
  assert.ok(failed.state.record); assert.match(failed.state.error, /记录已更新/);
  assert.equal(failed.state.invalidations, 0); assert.deepEqual(failed.state.loads, []);
  assert.equal(failed.state.deleting, false); assert.equal(failed.environment.auxiliaryDeletePending.current, false);
  failed.environment.changeAuxiliaryState = async (...args) => { failed.state.writes.push(args); };
  await failed.remove('重新核对');
  assert.equal(failed.state.record, null); assert.equal(failed.state.writes.length, 1);
});

test('statistics auxiliary delete blocks repeated confirmation and opening another row until it finishes', async () => {
  const pending = deferred(), calls = [];
  const h = deleteHarness({ changeAuxiliaryState: (...args) => { calls.push(args); return pending.promise; } });
  const task = h.remove('录入重复');
  await h.remove('再次点击');
  action('openAuxiliaryDelete', h.environment)(writableAuxiliary());
  assert.equal(calls.length, 1); assert.equal(h.state.deleting, true);
  pending.resolve(); await task;
  assert.equal(h.state.invalidations, 1); assert.equal(h.environment.auxiliaryDeletePending.current, false);
  action('openAuxiliaryDelete', h.environment)({ ...writableAuxiliary(), origin: 'work_record' });
  assert.equal(h.state.record, null);
  action('openAuxiliaryDelete', h.environment)(writableAuxiliary());
  assert.equal(h.state.record.id, 7);
});

function logHarness(overrides = {}) {
  const state = { rows: [], total: 0, page: 1, error: '' }, pending = deferred();
  const environment = {
    ac: 'auxiliary-log', PS: 50, uf: '', canViewRoleStats: true, user: { id: 11 }, si: '2026-10-01', ei: '2026-10-31',
    selectedDetectionDivisions: [2], selectedSendingDivisions: [], includePendingOwnership: false,
    userLogQueryKey: 'current', currentUserLogQuery: { current: 'current' }, userLogSeq: { current: 0 },
    setUll: value => { state.loading = value; }, setUl: value => { state.rows = value; }, setUlt: value => { state.total = value; }, setUlp: value => { state.page = value; },
    setPersonRowsQuery: value => { state.loadedQuery = value; }, setEr: value => { state.error = value; },
    getPersonRecords: query => { state.query = query; return pending.promise; }, ...overrides,
  };
  return { state, pending, environment, load: () => action('lul', environment) };
}

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

test('mixed auxiliary rows preserve snapshots and show deletion only for server-authorized independent records', () => {
  const selected = [];
  const renderRow = action('renderAuxiliaryPersonRow', { React, TableRow: mui.TableRow, TableCell: mui.TableCell, Typography: mui.Typography, Chip: mui.Chip, Button: mui.Button, personRecordKey: recordsApi.personRecordKey, canDeleteAuxiliaryRecord: recordsApi.canDeleteAuxiliaryRecord, auxiliaryDeleting: false, openAuxiliaryDelete: record => selected.push(record), ulp: 2, PS: 100, userLogNowrapCellSx: {}, userLogWrapCellSx: {}, adaptiveDateCellSx: {}, chipSx: {}, formatDateTimeDisplay: value => value });
  const html = renderToStaticMarkup(React.createElement(mui.Table, null, React.createElement(mui.TableBody, null, renderRow(row('auxiliary_record'), 0))));
  for (const value of ['维护仪器', '检测人员甲', '系数 0', '工作量 0', '液相', '日常维护', '辅助记录，只读', '101']) assert.ok(html.includes(value), value);
  assert.equal(html.includes('<input'), false); assert.equal(html.includes('<button'), false);
  assert.equal(html.includes('item-7'), false);
  const writable = writableAuxiliary();
  const buttons = descendants(renderRow(writable, 0), child => child.type === mui.Button);
  assert.equal(buttons.length, 1); assert.equal(buttons[0].props.children, '删除');
  buttons[0].props.onClick(); assert.deepEqual(selected, [writable]);
  assert.equal(descendants(renderRow({ ...writable, origin: 'work_record' }, 0), child => child.type === mui.Button).length, 0);
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
    const props = { rows: [writable, row('auxiliary_record'), { ...writable, origin: 'work_record' }], loading: false, total: 3, page: 1, pageSize: 50, onPageChange: () => {}, onPageSizeChange: () => {}, onDelete: record => selected.push(record), deleting };
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
