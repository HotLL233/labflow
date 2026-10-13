// Run against a disposable test deployment with an administrator and a non-admin
// account that has manage:settings and manage:sampleinfo permissions.
// Required env: LABFLOW_QA_URL, QA_ADMIN_USER, QA_ADMIN_PASSWORD, QA_USER, QA_PASSWORD.
import assert from 'node:assert/strict';
const base = process.env.LABFLOW_QA_URL;
assert.ok(base, 'LABFLOW_QA_URL must point to a disposable test deployment');
async function request(path, token, method = 'GET', body) {
  const response = await fetch(`${base}/api${path}`, {
    method, headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return { status: response.status, body: await response.json() };
}
async function login(username, password) {
  assert.ok(username && password, 'QA account credentials are required');
  const result = await request('/users/login', null, 'POST', { username, password });
  assert.equal(result.body.code, 0, result.body.message);
  return result.body.data;
}
const admin = await login(process.env.QA_ADMIN_USER, process.env.QA_ADMIN_PASSWORD);
const member = await login(process.env.QA_USER, process.env.QA_PASSWORD);
assert.equal(member.user.is_admin, false);
assert.ok(member.user.permissions.includes('manage:settings'));
assert.ok(member.user.permissions.includes('manage:sampleinfo'));
for (const key of ['record_actions_rd', 'record_actions_sample_info']) {
  const previous = await request('/settings', admin.token);
  assert.equal(previous.body.code, 0);
  const original = previous.body.data.find(item => item.key === key)?.value;
  const value = { before_column: 'user_name', button_order: key.endsWith('_rd') ? ['edit', 'return'] : ['action_return', 'action_edit'] };
  try {
    const saved = await request(`/settings/${key}`, admin.token, 'PUT', { value });
    assert.equal(saved.body.code, 0, saved.body.message);
    const read = await request(`/settings/${key}`, member.token);
    assert.deepEqual(JSON.parse(read.body.data.value), value);
    const denied = await request(`/settings/${key}`, member.token, 'PUT', { value: {} });
    assert.equal(denied.body.code, 1003);
    assert.deepEqual(JSON.parse((await request(`/settings/${key}`, admin.token)).body.data.value), value);
  } finally {
    assert.equal((await request(`/settings/${key}`, admin.token, 'PUT', { value: original ? JSON.parse(original) : {} })).body.code, 0);
  }
}
for (const path of ['/rd-record-columns', '/sample-info/columns']) {
  const columns = await request(path, admin.token);
  const column = columns.body.data[0];
  assert.ok(column, `${path} requires seeded columns`);
  assert.equal((await request(`${path}/${column.id}`, member.token, 'PUT', { label: column.label })).body.code, 1003);
}
console.log('PASS: admin save, cross-user read, non-admin writes denied, saved values preserved.');
