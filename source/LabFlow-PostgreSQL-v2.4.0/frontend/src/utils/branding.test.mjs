import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const exports = {};
new Function('exports', ts.transpileModule(readFileSync(new URL('./branding.ts', import.meta.url), 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
}).outputText)(exports);

test('labflow replaces legacy defaults while preserving custom brand configuration', () => {
  for (const value of [undefined, null, 0, {}, '', '知微', '样品管理系统']) {
    assert.equal(exports.brandName(value), 'labflow');
  }
  assert.equal(exports.brandName('我的实验室'), '我的实验室');
  assert.equal(exports.APP_PUBLISHER, 'HotLL');
});
