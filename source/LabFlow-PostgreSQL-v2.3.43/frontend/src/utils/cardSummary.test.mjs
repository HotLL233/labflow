import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./recordTableStyle.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { normalizeRecordTableStyle, cardSummaryGroups, resolveRecordTableStyle, cardContainerSx, cardFieldSx } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('card outline and field borders remain independently configurable, including no border', () => {
  const spec = normalizeRecordTableStyle({ card: { borderColor: '#123456', borderWidth: 3, fieldBorderColor: '#abcdef', fieldBorderWidth: 2 } });
  assert.equal(cardContainerSx(spec).border, '3px solid #123456');
  assert.equal(cardFieldSx(spec).border, '2px solid #abcdef');
  const noInner = normalizeRecordTableStyle({ ...spec, card: { ...spec.card, fieldBorderWidth: 0 } });
  assert.equal(cardFieldSx(noInner).border, '0px solid #abcdef');
  assert.equal(cardContainerSx(noInner).border, cardContainerSx(spec).border);
});

test('legacy settings use four default groups; three rows merge without losing or mutating fields', () => {
  const legacy = normalizeRecordTableStyle({ card: { borderWidth: 2 } });
  const defaults = [['batch', 'status'], ['project'], ['method', 'quantity'], ['time', 'workload']];
  assert.equal(legacy.card.summaryRows, 4);
  assert.equal(legacy.card.summaryFields, null);
  assert.deepEqual(cardSummaryGroups(legacy.card, defaults), defaults);
  const groups = cardSummaryGroups({ ...legacy.card, summaryRows: 3 }, defaults);
  assert.deepEqual(groups, [['batch', 'status'], ['project'], ['method', 'quantity', 'time', 'workload']]);
  assert.deepEqual(defaults[2], ['method', 'quantity']);
});

test('custom ordering survives normalization, while malformed and duplicate fields are removed', () => {
  const style = normalizeRecordTableStyle({ card: { summaryRows: 9, summaryFields: [['custom_b', 'custom_a', ''], ['custom_b', 2], null, ['time']] } });
  assert.equal(style.card.summaryRows, 4);
  assert.deepEqual(style.card.summaryFields, [['custom_b', 'custom_a'], [], [], ['time']]);
  assert.deepEqual(cardSummaryGroups(style.card, [['ignored']]), style.card.summaryFields);
});

test('global summary updates reach existing personal views while preserving their visual preferences', () => {
  const global = normalizeRecordTableStyle({ card: { summaryRows: 3, summaryFields: [['shared'], [], [], ['time']] } });
  const personal = normalizeRecordTableStyle({ card: { borderColor: '#ff0000', summaryRows: 4, summaryFields: [['stale']] }, columns: { batch: { width: 220 } } });
  const merged = resolveRecordTableStyle(global, personal, false);
  assert.equal(merged.card.summaryRows, 3);
  assert.deepEqual(merged.card.summaryFields, global.card.summaryFields);
  assert.equal(merged.card.borderColor, personal.card.borderColor);
  assert.equal(merged.columns.batch.width, 220);
  assert.deepEqual(personal.card.summaryFields, [['stale'], [], [], []]);
  assert.equal(resolveRecordTableStyle(global, personal, true), personal);
  assert.equal(resolveRecordTableStyle(global, null, false), global);
});
