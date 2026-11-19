import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
const source = readFileSync(new URL('./workloadMatrix.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { buildWorkloadMatrix } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
test('matrix preserves stable identity, unbound history, zero users and both totals', () => {
  const people = [{ user_id: 1, user_name: '同名', total_workload: 8 }, { user_id: 2, user_name: '同名', total_workload: 4 }, { user_id: null, user_name: '同名', total_workload: 2 }, { user_id: 3, user_name: '零量', total_workload: 0 }];
  const report = buildWorkloadMatrix(people, [
    { ...people[0], type: '化学', total_workload: 3 }, { ...people[0], type: '化学', total_workload: 2 },
    { ...people[0], type: '辅助工作', total_workload: 3 }, { ...people[1], type: '化学', total_workload: 4 },
    { ...people[2], type: '化学', total_workload: 2 },
  ]);
  assert.deepEqual(report.rows.map(row => row.key), ['id:1', 'id:2', 'legacy:同名', 'id:3']);
  assert.equal(report.rows[0].cells.get('化学'), 5);
  assert.equal(report.rows[1].cells.get('化学'), 4);
  assert.equal(report.rows[2].cells.get('化学'), 2);
  assert.equal(report.rows[3].cells.size, 0);
  assert.equal(report.columns.get('化学'), 11);
  assert.equal(report.columns.get('辅助工作'), 3);
  assert.equal(report.total, 14);
});
