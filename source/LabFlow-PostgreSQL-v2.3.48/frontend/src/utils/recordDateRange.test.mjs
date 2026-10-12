import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
const source = readFileSync(new URL('./recordDateRange.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { validRecordDate, validateRecordDateRange, recordDatePreset, shiftRecordWeek } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('records default to all time and ISO weeks include Sunday before the Monday boundary', () => {
  assert.deepEqual(recordDatePreset('all', '2026-10-10'), { start: '', end: '' });
  assert.deepEqual(recordDatePreset('this-week', '2026-10-10'), { start: '2026-10-05', end: '2026-10-11' });
  assert.deepEqual(recordDatePreset('last-week', '2026-10-10'), { start: '2026-09-28', end: '2026-10-04' });
  assert.deepEqual(recordDatePreset('this-week', '2026-10-04'), { start: '2026-09-28', end: '2026-10-04' });
  assert.deepEqual(recordDatePreset('recent', '2026-10-10'), { start: '2026-10-04', end: '2026-10-10' });
  assert.deepEqual(shiftRecordWeek('2026-10-05', -1), { start: '2026-09-28', end: '2026-10-04' });
});
test('custom ranges reject impossible dates and inversions, including leap-year boundaries', () => {
  assert.equal(validRecordDate('2024-02-29'), true);
  assert.equal(validRecordDate('2026-02-29'), false);
  assert.equal(validRecordDate('2026-02-31'), false);
  assert.equal(validRecordDate('2026-10-10T10:00:00'), false);
  assert.equal(validateRecordDateRange('2026-10-10', '2026-10-10'), '');
  assert.equal(validateRecordDateRange('2026-10-11', '2026-10-10'), '开始日期不能晚于结束日期');
  assert.notEqual(validateRecordDateRange('', '2026-10-10'), '');
});
