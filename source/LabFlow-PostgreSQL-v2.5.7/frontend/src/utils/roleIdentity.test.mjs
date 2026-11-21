import assert from 'node:assert/strict';
import { test } from 'node:test';
import { rolePurpose, roleBusinessIdentity } from './roleIdentity.ts';

test('explicit role purpose is authoritative even with an old public name or key', () => {
  assert.equal(rolePurpose({ role_purpose: 'personal', system_key: 'analysis_public_account', name: '分析检测公共账号' }), 'personal');
  assert.equal(rolePurpose({ role_purpose: 'public_analysis', name: '自定义共享账号' }), 'public_analysis');
  assert.equal(rolePurpose({ role_purpose: 'public_rd', system_key: 'analysis_public_account' }), 'public_rd');
  assert.equal(rolePurpose({ role_purpose: 'invalid', system_key: 'rd_public_account' }), 'personal');
});

test('legacy purpose fallback uses exact stable keys and built-in names only', () => {
  assert.equal(rolePurpose({ system_key: 'public_account' }), 'public_rd');
  assert.equal(rolePurpose({ system_key: 'rd_public_account' }), 'public_rd');
  assert.equal(rolePurpose({ system_key: 'analysis_public_account' }), 'public_analysis');
  assert.equal(rolePurpose({ name: '分析检测公共账号' }), 'public_analysis');
  assert.equal(rolePurpose({ name: '研发送样公共账号' }), 'public_rd');
  assert.equal(rolePurpose({ name: '仿分析检测公共账号' }), 'personal');
  assert.equal(rolePurpose({ system_key: 'ordinary', name: '分析检测公共账号' }), 'personal');
});

test('stable business identity survives renaming and overrides legacy template names', () => {
  assert.equal(roleBusinessIdentity({ identity_key: 'analysis_leader', name: '自定义名称', template_name: '新名称' }), 'analysis_leader');
  assert.equal(roleBusinessIdentity({ identity_key: 'ordinary', name: '分析检测组长', template_name: '分析检测组长模板' }), 'ordinary');
  assert.equal(roleBusinessIdentity({ system_key: 'rd_sender' }), 'rd_sender');
  assert.equal(roleBusinessIdentity({ name: '自定义角色', template_name: '研发送样组长模板' }), 'rd_leader');
  assert.equal(roleBusinessIdentity({ name: '组长助理' }), 'ordinary');
});
