import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const module = { exports: {} };
const compiled = ts.transpileModule(readFileSync(new URL('./notificationRecord.ts', import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
}).outputText;
new Function('exports', compiled)(module.exports);

test('notification record IDs accept only positive safe integer identifiers', () => {
  const parse = module.exports.parseNotificationRecordId;
  assert.equal(parse('123'), 123);
  for (const value of [null, '', '0', '-1', '1.5', '1e3', ' 1', '01', 'NaN', '9007199254740992']) {
    assert.equal(parse(value), undefined);
  }
});
