import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./sampleWorkload.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { workloadLabel, workloadCoefficient, RD_SORTABLE_FIELDS, findWorkloadTarget, filterWorkloadInstruments, sampleWorkloadDraftKey, parseSampleWorkloadDraft } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

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
