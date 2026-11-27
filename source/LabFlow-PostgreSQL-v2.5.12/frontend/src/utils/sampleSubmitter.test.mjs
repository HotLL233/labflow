import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./sampleSubmitter.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { requiresActualSubmitter, defaultSubmitterName, isSubmitterPlaceholder, submitterNeedsConfirmation } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('shared submitter rules preserve personal defaults without guessing account names', () => {
  assert.equal(defaultSubmitterName({ username: '厂区送样人员' }), '厂区送样人员');
  for (const flag of ['is_public_account', 'is_rd_public_account', 'is_analysis_public_account', 'require_actual_submitter']) {
    const account = { username: 'shared', [flag]: true };
    assert.equal(requiresActualSubmitter(account), true);
    assert.equal(defaultSubmitterName(account), '');
    assert.equal(submitterNeedsConfirmation({ user_name: 'shared' }, account), true);
    assert.equal(submitterNeedsConfirmation({ user_name: 'shared', _submitterConfirmed: true }, account), false);
    assert.equal(submitterNeedsConfirmation({ user_name: '真实送样人' }, account), false);
  }
  assert.equal(defaultSubmitterName({ username: '张三' }), '张三');
  for (const value of ['', ' ', '未知', '请填写实际送样人', '请选择送样人']) assert.equal(isSubmitterPlaceholder(value), true);
  assert.equal(isSubmitterPlaceholder('张三'), false);
});
