import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
const source = readFileSync(new URL('./workloadTypeDrill.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { nextWorkloadDimension, workloadDrillQuery, workloadPeopleQuery, workloadShare, workloadDetailPersonLabel, workloadBreakdownLabel } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
const step = (dimension, key) => ({ dimension, key, name: key });

test('detection drill preserves authorized filters and historical identities through both orders', () => {
  const base = { start: '2026-10-01', end: '2026-10-12', view: 'scope', subject_user_id: 7, detection_division_ids: '1,2', sending_division_ids: '3', include_pending_ownership: true, method_id: 42 };
  const type = step('type', '液相');
  const method = step('method', 'legacy:同名方法');
  const instrument = step('instrument', '12');
  assert.equal(nextWorkloadDimension([], 'method'), 'type');
  assert.equal(nextWorkloadDimension([type], 'instrument'), 'instrument');
  assert.equal(nextWorkloadDimension([type, method], 'method'), 'instrument');
  assert.equal(nextWorkloadDimension([type, instrument], 'method'), 'method');
  assert.equal(nextWorkloadDimension([type, instrument, method], 'method'), null);
  const query = workloadDrillQuery(base, [type, method, instrument], 2, 50);
  assert.deepEqual(query, { ...base, kind: 'detection', type: '液相', method_id: undefined, method_key: 'legacy:同名方法', instrument_id: undefined, instrument_key: '12', page: 3, page_size: 50, compact_type_summary: true });
  assert.equal(workloadDrillQuery(base, []).method_key, undefined);
});

test('auxiliary branches use their own categories and item keys without instrument allocation', () => {
  const type = step('type', '辅助工作');
  const category = step('auxiliary_category', '未分类');
  const item = step('auxiliary_method', 'legacy:item:前处理');
  assert.equal(nextWorkloadDimension([type], 'method'), 'auxiliary_category');
  assert.equal(nextWorkloadDimension([type], 'auxiliary_method'), 'auxiliary_method');
  assert.equal(nextWorkloadDimension([type, category], 'auxiliary_category'), 'auxiliary_method');
  assert.equal(nextWorkloadDimension([type, item], 'auxiliary_method'), null);
  const query = workloadDrillQuery({ start: '2026-10-01', end: '2026-10-12', view: 'mine' }, [type, category, item]);
  assert.equal(query.kind, 'auxiliary');
  assert.equal(query.auxiliary_category, '未分类');
  assert.equal(query.auxiliary_key, 'legacy:item:前处理');
  assert.equal(query.instrument_id, undefined);
  assert.equal(workloadShare(3, 12), '25%');
  assert.equal(workloadShare(0, 0), '—');
});

test('detail person labels distinguish stable accounts from same-name and unbound history', () => {
  const peers = [{ user_id: 7, user_name: '张三' }, { user_id: 7, user_name: '张三' }, { user_id: 8, user_name: '张三' }, { user_id: null, user_name: '张三' }];
  assert.equal(workloadDetailPersonLabel(peers[0], peers), '张三（同名账号 1）');
  assert.equal(workloadDetailPersonLabel(peers[2], peers), '张三（同名账号 2）');
  assert.equal(workloadDetailPersonLabel(peers[3], peers), '张三（历史未绑定）');
  assert.equal(workloadDetailPersonLabel(peers[0], peers.slice(0, 2)), '张三');
});

test('same-name classifications distinguish IDs without exposing internal IDs', () => {
  const rows = [{ key: 'method:20', name: '准备' }, { key: 'method:3', name: '准备' }, { key: 'item:4', name: '准备' }, { key: 'legacy:method:准备', name: '准备' }];
  assert.equal(workloadBreakdownLabel('auxiliary_method', rows[0], rows), '准备（同名辅助方法 2）');
  assert.equal(workloadBreakdownLabel('auxiliary_method', rows[1], rows), '准备（同名辅助方法 1）');
  assert.equal(workloadBreakdownLabel('auxiliary_method', rows[2], rows), '准备（辅助事项）');
  assert.equal(workloadBreakdownLabel('auxiliary_method', rows[3], rows), '准备（历史未绑定）');
  const instruments = [{ key: '200', name: '液相仪' }, { key: '30', name: '液相仪' }];
  assert.equal(workloadBreakdownLabel('instrument', instruments[0], instruments), '液相仪（同名仪器 2）');
  assert.equal(workloadBreakdownLabel('instrument', instruments[1], instruments), '液相仪（同名仪器 1）');
});

test('type-view personnel choices keep the authorized range and do not shrink with selected person or drill conditions', () => {
  const range = { start: '2026-09-01', end: '2026-09-30', view: 'scope', include_pending_ownership: true, include_zero_users: false };
  const filtered = { ...range, subject_user_id: 77, type: '液相', kind: 'detection', method_key: '12', instrument_key: 'legacy:旧仪器', division_id: 1, auxiliary_category: '维护', auxiliary_key: 'item:9', page: 3, page_size: 50 };
  assert.deepEqual(workloadPeopleQuery(filtered), { ...range, compact_type_summary: true, include_people: true, page: 1, page_size: 1 });
  assert.deepEqual(workloadPeopleQuery({ ...filtered, subject_user_id: 88, type: '辅助工作', kind: 'auxiliary' }), workloadPeopleQuery(filtered));
  const movedRange = workloadPeopleQuery({ ...filtered, start: '2026-10-01', end: '2026-10-12', view: 'mine' });
  assert.equal(movedRange.start, '2026-10-01');
  assert.equal(movedRange.view, 'mine');
  assert.equal(movedRange.subject_user_id, undefined);
});
