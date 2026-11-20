import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./sampleColumnTypeRules.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { buildSampleColumnTypeRules } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('editing one detection type preserves the other type settings and all-hidden state', () => {
  const rules = [
    { type_key: 'icp', is_visible: false, is_required: false, show_in_form: false, show_in_list: false, show_in_export: false, sort_order: 3 },
    { type_key: 'thermal', is_visible: true, is_required: true, show_in_form: true, show_in_list: false, show_in_export: true, sort_order: 7 },
  ];
  const selected = { is_required: false, show_in_form: false, show_in_list: true, show_in_export: false, sort_order: 4 };
  const updated = buildSampleColumnTypeRules(rules, 'icp', selected, false);
  assert.deepEqual(updated[0], { ...rules[0], sort_order: 4 });
  assert.deepEqual(updated[1], rules[1]);
  assert.deepEqual(buildSampleColumnTypeRules(rules.map(rule => ({ ...rule, is_visible: false })), 'icp', selected, false)
    .map(rule => rule.is_visible), [false, false]);
});
