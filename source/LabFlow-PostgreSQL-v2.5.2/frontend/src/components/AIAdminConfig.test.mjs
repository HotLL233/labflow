import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('AIAdminConfig.tsx', readFileSync(new URL('./AIAdminConfig.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function expression(name) {
  let found;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) found = node.initializer; ts.forEachChild(node, visit); };
  visit(source); assert.ok(found, name); return found;
}
function evaluate(node, environment) {
  const code = ts.transpileModule(`const value = (${node.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), code + '\nreturn value;')(...Object.values(environment));
}
const fixture = () => ({ provider_id: 'service', model: 'model', kind: 'hi', passed: true, checked_at: '2026-11-14 10:00:00', message: 'hello' });

test('closing a test alert changes only local visibility and retains configuration and test qualification', () => {
  let close;
  const visit = node => { if (ts.isJsxAttribute(node) && node.name.getText(source) === 'onClose') close = node.initializer.expression; ts.forEachChild(node, visit); };
  visit(expression('renderResult')); assert.ok(close);
  const result = fixture(), config = { tests: [result], providers: [{ secret: 'write-only-value' }] }, before = structuredClone(config);
  let dismissedTests = [];
  evaluate(close, { result, setDismissedTests: update => { dismissedTests = update(dismissedTests); } })();
  assert.deepEqual(evaluate(expression('visibleResults'), { results: config.tests, dismissedTests }), []);
  assert.deepEqual(config, before); assert.equal(config.tests[0].passed, true);
});

test('retesting displays a new result even with identical content, and re-entry restores stored results', () => {
  const old = fixture(), fresh = fixture(), other = { ...fixture(), kind: 'tools' };
  assert.deepEqual(evaluate(expression('visibleResults'), { results: [fresh, other], dismissedTests: [old] }), [fresh, other]);
  assert.deepEqual(evaluate(expression('visibleResults'), { results: [old], dismissedTests: [] }), [old]);
});
