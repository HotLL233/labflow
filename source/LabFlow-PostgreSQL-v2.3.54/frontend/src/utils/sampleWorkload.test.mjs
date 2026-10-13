import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./sampleWorkload.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { workloadLabel, workloadCoefficient, RD_SORTABLE_FIELDS, findWorkloadTarget, filterWorkloadInstruments, filterWorkloadOptions, nextWorkloadSearchQuery, workloadSearchPending, workloadInstrumentLabel, workloadInstrumentOptions, workloadMethodSearchText, sampleWorkloadDraftKey, parseSampleWorkloadDraft } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('partial recording stays actionable until source quantity is fully recorded', () => {
  assert.equal(workloadLabel({ quantity: 10, recorded_quantity: 0, workload_recorded: false }), '录入工作量');
  assert.equal(workloadLabel({ quantity: 10, recorded_quantity: 3, workload_recorded: false }), '部分录入（3/10）');
  assert.equal(workloadLabel({ quantity: 10, recorded_quantity: 10, workload_recorded: true }), '已录入');
});

test('selected method coefficient replaces the first candidate and preserves zero', () => {
  assert.equal(workloadCoefficient(undefined, 1, true), undefined);
  assert.equal(workloadCoefficient({ coefficient: 3 }, 1, true), 3);
  assert.equal(workloadCoefficient({ coefficient: 0 }, 1, true), 0);
  assert.equal(workloadCoefficient(undefined, 2, false), 2);
  assert.equal(workloadCoefficient(undefined, undefined, false), 1);
});

test('standard binding requires the exact project and laboratory pair, even when names repeat', () => {
  const targets = [
    { project_id: 1, group_id: 10, project_name: 'Same name', coefficient: 0 },
    { project_id: 1, group_id: 20, project_name: 'Same name', coefficient: 3 },
    { project_id: 2, group_id: 10, project_name: 'Same name', coefficient: 5 },
  ];
  assert.equal(findWorkloadTarget(targets, 1, 10)?.coefficient, 0);
  assert.equal(findWorkloadTarget(targets, 1, 20)?.coefficient, 3);
  assert.equal(findWorkloadTarget(targets, 2, 20), undefined);
  assert.equal(findWorkloadTarget(targets, 1, undefined), undefined);
});

test('instrument selection exposes all linked methods and keeps unbound instruments distinct', () => {
  const methods = [{ method_id: 1, instrument_id: 10 }, { method_id: 2, instrument_id: 10 }, { method_id: 3, instrument_id: 20 }, { method_id: 4, instrument_id: 0 }];
  assert.deepEqual(filterWorkloadInstruments(methods, '').map(item => item.method_id), [1, 2, 3, 4]);
  assert.deepEqual(filterWorkloadInstruments(methods, 10).map(item => item.method_id), [1, 2]);
  assert.deepEqual(filterWorkloadInstruments(methods, 0).map(item => item.method_id), [4]);
  assert.deepEqual(filterWorkloadInstruments(methods, 99), []);
});

test('library search includes names, codes and real type labels with all case-insensitive keywords', () => {
  const methods = [
    { method_id: 1, instrument_id: 10, instrument: 'LC-01 · Full method', instrument_name: '高效液相色谱仪', instrument_code: 'LC-01', instrument_type: '液相', method_name: '含量测定', method_short_name: '含量', method_code: 'M-0001', method_types: ['液相', '定量检测'] },
    { method_id: 2, instrument_id: 20, instrument: 'GC-02 · Gas method', instrument_name: '气相色谱仪', instrument_code: 'GC-02', instrument_type: '气相', method_name: '残留溶剂', method_code: 'M-0002', method_types: ['气相'] },
  ];
  assert.deepEqual(filterWorkloadOptions(methods, '  液相 lc-01  ', workloadMethodSearchText).map(item => item.method_id), [1]);
  assert.deepEqual(filterWorkloadOptions(methods, 'M-0001 定量', workloadMethodSearchText).map(item => item.method_id), [1]);
  assert.deepEqual(filterWorkloadOptions(methods, '量测', workloadMethodSearchText).map(item => item.method_id), [1]);
  assert.deepEqual(filterWorkloadOptions(methods, '气相 lc-01', workloadMethodSearchText), []);
  assert.equal(filterWorkloadOptions(methods, '', workloadMethodSearchText), methods);
  assert.equal(workloadInstrumentLabel(methods[0]), 'LC-01 · 高效液相色谱仪');
  assert.equal(workloadInstrumentLabel({ instrument_id: 30, instrument: 'legacy label' }), 'legacy label');
  assert.equal(workloadInstrumentLabel({ instrument_id: 30, instrument: '', instrument_name: '具名仪器', instrument_code: '' }), '具名仪器');
});

test('instrument grouping searches every linked method, including multiple types without an instrument', () => {
  const grouped = workloadInstrumentOptions([
    { method_id: 1, instrument_id: 0, instrument: 'Physical method', method_name: '外观', method_code: 'M-0100', method_types: ['理化'] },
    { method_id: 2, instrument_id: 0, instrument: 'Balance method', method_name: '称量', method_code: 'M-0200', method_types: ['辅助工作'] },
    { method_id: 3, instrument_id: 10, instrument: 'LC first', method_name: 'First method', instrument_name: '液相色谱仪', instrument_code: 'LC-10', method_types: ['液相'] },
    { method_id: 4, instrument_id: 10, instrument: 'LC later', method_name: 'Later method', method_code: 'M-0400', instrument_type: '液相', method_types: ['液相', '含量检测'] },
  ]);
  assert.equal(grouped.length, 2);
  assert.equal(grouped[0].method_count, 2);
  assert.equal(grouped[1].method_count, 2);
  assert.deepEqual(grouped[0].method_types, ['理化', '辅助工作']);
  assert.equal(workloadInstrumentLabel(grouped[0]), '未关联仪器');
  assert.deepEqual(filterWorkloadOptions(grouped, '辅助 M-0200', item => item.search_text).map(item => item.instrument_id), [0]);
  assert.deepEqual(filterWorkloadOptions(grouped, '含量 M-0400', item => item.search_text).map(item => item.instrument_id), [10]);
  assert.deepEqual(filterWorkloadOptions(grouped, 'gc 气相', item => item.search_text), []);
  assert.equal(workloadMethodSearchText({ instrument_id: 0, instrument: '', method_types: ['理化'] }), '理化');
});

test('unconfirmed library keywords survive MUI reset and blur instead of submitting an old selection', () => {
  const oldLabel = 'M-0001 · 含量测定 · LC-01';
  let query = nextWorkloadSearchQuery(null, '残留溶剂', 'input');
  assert.equal(workloadSearchPending(query, oldLabel), true);
  for (const reason of ['reset', 'blur']) {
    query = nextWorkloadSearchQuery(query, oldLabel, reason);
    assert.equal(query, '残留溶剂');
    assert.equal(workloadSearchPending(query, oldLabel), true);
  }
  // 选择库候选时组件 onChange 才清空查询，随后显示新条目的标签。
  query = null;
  assert.equal(workloadSearchPending(query, 'M-0002 · 残留溶剂 · GC-02'), false);
});

test('explicit clear, exact selected labels and automatic sole candidates remain usable', () => {
  const label = 'LC-01 · 高效液相色谱仪';
  assert.equal(nextWorkloadSearchQuery('未知仪器', '', 'clear'), null);
  assert.equal(workloadSearchPending(nextWorkloadSearchQuery(null, label, 'input'), label), false);
  assert.equal(workloadSearchPending(nextWorkloadSearchQuery(null, '', 'input'), label), true);
  const followingSelection = nextWorkloadSearchQuery(null, '自动推荐方法', 'reset');
  assert.equal(followingSelection, null);
  assert.equal(workloadSearchPending(followingSelection, '自动推荐方法'), false);
  assert.equal(workloadSearchPending(followingSelection, '更换项目后的唯一方法'), false);
});

test('explicitly selecting the same library item confirms a short keyword even without onChange', () => {
  const selectedLabel = 'M-0001 · 含量测定 · LC-01';
  let query = nextWorkloadSearchQuery(null, '含量', 'input');
  query = nextWorkloadSearchQuery(query, selectedLabel, 'reset');
  assert.equal(workloadSearchPending(query, selectedLabel), true);
  for (const reason of ['blur', 'escape', 'toggleInput']) {
    assert.equal(nextWorkloadSearchQuery(query, '', reason), '含量');
  }
  // 同项点击不会触发 MUI onChange，但仍以 selectOption 关闭候选列表。
  query = nextWorkloadSearchQuery(query, '', 'selectOption');
  assert.equal(query, null);
  assert.equal(workloadSearchPending(query, selectedLabel), false);
});

test('local pending drafts isolate accounts and sources, retain form values, and drop unknown fields', () => {
  const draft = { version: 1, quantity: 3, multiplier: 0, recorded_at: '2024-02-29T12:03:04', notes: 'pending', custom_method_name: 'Free method', custom_instrument_name: '', custom: true, project_id: 1, group_id: 10 };
  const restored = parseSampleWorkloadDraft(JSON.stringify({ ...draft, token: 'must-not-copy', source_record_id: 55 }));
  assert.equal(restored.quantity, 3);
  assert.equal(restored.multiplier, 0);
  assert.equal(restored.recorded_at, draft.recorded_at);
  assert.equal(restored.notes, draft.notes);
  assert.equal('token' in restored, false);
  assert.equal('source_record_id' in restored, false);
  assert.notEqual(sampleWorkloadDraftKey(1, 55), sampleWorkloadDraftKey(2, 55));
  assert.notEqual(sampleWorkloadDraftKey(1, 55), sampleWorkloadDraftKey(1, 56));
  assert.equal(sampleWorkloadDraftKey(undefined, 55), null);
  assert.equal(sampleWorkloadDraftKey(1, -1), null);
  for (const invalid of [null, '{}', '{bad', JSON.stringify({ ...draft, quantity: 0 }), JSON.stringify({ ...draft, quantity: 1.5 }), JSON.stringify({ ...draft, multiplier: -1 }), JSON.stringify({ ...draft, group_id: 0 }), JSON.stringify({ ...draft, notes: null })]) assert.equal(parseSampleWorkloadDraft(invalid), null);
});

test('every offered RD sort field exists in the backend whitelist', () => {
  const backend = readFileSync(new URL('../../../src/repo/rd_record_repo.rs', import.meta.url), 'utf8');
  const match = /let field = match sort_by\.unwrap_or\("submitted_at"\) \{([\s\S]*?)=> \{/.exec(backend);
  assert.ok(match, 'the active RD list query must keep an explicit system-field sort whitelist');
  const fields = [...match[1].matchAll(/"([a-z_]+)"/g)].map(value => value[1]);
  assert.deepEqual([...RD_SORTABLE_FIELDS].sort(), [...new Set(fields)].sort());
  assert.equal(RD_SORTABLE_FIELDS.has('sampling_time'), false);
  assert.equal(RD_SORTABLE_FIELDS.has('custom_field'), false);
});
