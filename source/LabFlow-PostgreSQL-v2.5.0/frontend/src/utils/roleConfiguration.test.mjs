import test from 'node:test';
import assert from 'node:assert/strict';
import { configurationFingerprint, roleConfigurationPayload, permissionCategory } from './roleConfiguration.mjs';

const draft = () => ({ name: '角色', description: '', permissions: ['legacy:unknown', 'sample:view'], purpose: 'personal', divisions: [999, 2], workDivisions: [3], sampleTypes: ['old-type'], active: true, base: { configuration_revision: 'revision-1' } });
test('原子保存保留隐藏权限、停用范围和服务端修订', () => {
  const value = draft(); const payload = roleConfigurationPayload(value);
  assert.deepEqual(payload.permissions, ['legacy:unknown', 'sample:view']);
  assert.deepEqual(payload.division_ids, [999, 2]);
  assert.deepEqual(payload.work_division_ids, [3]);
  assert.deepEqual(payload.sample_info_type_keys, ['old-type']);
  assert.equal(payload.expected_revision, 'revision-1');
  payload.permissions.pop(); assert.equal(value.permissions.length, 2);
});
test('公共用途不自动新增普通权限或范围，明确个人用途不省略', () => {
  const value = draft(); value.purpose = 'public_analysis';
  assert.deepEqual(roleConfigurationPayload(value).permissions, value.permissions);
  assert.deepEqual(roleConfigurationPayload(value).division_ids, value.divisions);
  value.purpose = 'personal'; assert.equal(roleConfigurationPayload(value).role_purpose, 'personal');
});
test('排序不产生虚假修改，模板启停和未知权限修改可检测', () => {
  const value = draft(); const reordered = {...value, permissions: [...value.permissions].reverse(), divisions: [2, 999]};
  assert.equal(configurationFingerprint(value), configurationFingerprint(reordered));
  assert.notEqual(configurationFingerprint(value), configurationFingerprint({...value, active: false}));
  assert.notEqual(configurationFingerprint(value), configurationFingerprint({...value, permissions: ['sample:view']}));
});
test('内部必需、未知历史项进入高级分组，常用业务维持独立分组', () => {
  assert.equal(permissionCategory({key: 'records:work:portal-scoped'}), '高级与历史');
  assert.equal(permissionCategory({key: 'legacy:unknown', legacy: true}), '高级与历史');
  assert.equal(permissionCategory({key: 'entry:auxiliary'}), '辅助工作');
});
