import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./sampleWorkload.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { workloadLabel, workloadCoefficient, RD_SORTABLE_FIELDS } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

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

test('every offered RD sort field exists in the backend whitelist', () => {
  const backend = readFileSync(new URL('../../../src/repo/rd_record_repo.rs', import.meta.url), 'utf8');
  const match = backend.slice(backend.indexOf('let sort_expression = match'), backend.indexOf('let descending ='));
  const fields = [...match.matchAll(/"([a-z_]+)"/g)].map(value => value[1]);
  assert.deepEqual([...RD_SORTABLE_FIELDS].sort(), [...new Set(fields)].sort());
  assert.equal(RD_SORTABLE_FIELDS.has('sampling_time'), false);
  assert.equal(RD_SORTABLE_FIELDS.has('custom_field'), false);
});
