import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
const compiled = ts.transpileModule(readFileSync(new URL('./rdRecordFilters.ts', import.meta.url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { workloadRecorderLabel, rdFilterQuery, recordTimeDraft, validateRecordTimeRange } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('recorder labels preserve distinct identities and never count unknown historical entries as people', () => {
  assert.equal(workloadRecorderLabel({ workload_recorded: true }), '历史账号未关联');
  assert.equal(workloadRecorderLabel({ recorded_quantity: 0 }), '');
  assert.equal(workloadRecorderLabel({ workload_recorders: [{ user_id: 1, username: '林舟' }, { user_id: 1, username: '林舟' }, { user_id: 2, username: '林舟' }], workload_unknown_recorder_entries: 3 }), '林舟等 2 人 · 含历史账号未关联');
});
test('server filter serialization preserves blank values and stable person IDs', () => {
  assert.deepEqual(rdFilterQuery({ notes: [''], project_name: [] }, ['partial'], ['2', 'unknown']), { column_filters: '{"notes":[""]}', operation_states: 'partial', recorder_ids: '2,unknown' });
  assert.deepEqual(rdFilterQuery({}, [], []), { column_filters: undefined, operation_states: undefined, recorder_ids: undefined });
});
test('time drafts preserve end-of-day and precise boundaries and reject impossible or inverted dates', () => {
  assert.equal(recordTimeDraft('2026-10-11', true), '2026-10-11T23:59:59.999');
  assert.equal(recordTimeDraft('2026-10-11T12:00:00.100', true), '2026-10-11T12:00:00.100');
  assert.equal(validateRecordTimeRange('', '2024-02-29T23:59:59.999'), '');
  assert.equal(validateRecordTimeRange('2026-10-11T12:00:00.100', '2026-10-11T12:00:00.100'), '');
  assert.notEqual(validateRecordTimeRange('2026-02-29T12:00', ''), '');
  assert.notEqual(validateRecordTimeRange('2026-10-11T24:00', ''), '');
  assert.notEqual(validateRecordTimeRange('2026-10-11T12:00:00.101', '2026-10-11T12:00:00.100'), '');
});
