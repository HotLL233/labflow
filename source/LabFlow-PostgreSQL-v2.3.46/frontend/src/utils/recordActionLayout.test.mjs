import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
const source = readFileSync(new URL('./recordActionLayout.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { normalizeActionLayout, placeActionColumn, sortActionItems, RD_ACTIONS } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
test('action position and order retain hidden anchors, new actions and permissions', () => {
  assert.deepEqual(placeActionColumn(['name', 'status'], 'name'), ['_action', 'name', 'status']);
  assert.deepEqual(placeActionColumn(['name', 'status'], 'status'), ['name', '_action', 'status']);
  assert.deepEqual(placeActionColumn(['name'], 'hidden'), ['name', '_action']);
  const actions = [{ key: 'edit' }, { key: 'workload' }, { key: 'new' }];
  assert.deepEqual(sortActionItems(actions, ['workload', 'forbidden', 'edit']).map(x => x.key), ['workload', 'edit', 'new']);
  assert.deepEqual(actions.map(x => x.key), ['edit', 'workload', 'new']);
  assert.deepEqual(normalizeActionLayout({ before_column: 1, button_order: ['edit', 2, 'edit'] }), { before_column: '', button_order: ['edit'] });
});

test('collection joins custom ordering while unavailable actions stay absent', () => {
  const pending = RD_ACTIONS.filter(item => ['collect', 'edit', 'return'].includes(item.key));
  assert.deepEqual(sortActionItems(pending, ['edit', 'workload', 'return', 'collect']).map(item => item.key), ['edit', 'return', 'collect']);
  const taken = RD_ACTIONS.filter(item => ['workload', 'withdraw'].includes(item.key));
  assert.deepEqual(sortActionItems(taken, ['collect', 'return', 'workload', 'withdraw']).map(item => item.key), ['workload', 'withdraw']);
});
