import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

function action(file, name, environment) {
  const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer;
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(expression, name);
  const code = ts.transpileModule(`const callback = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), code + '\nreturn callback;')(...Object.values(environment));
}

test('save and leave waits for every configuration, keeps a failed one open and ignores an old account response', async () => {
  for (const scenario of ['success', 'rejected', 'account-changed']) {
    const owner = { current: 'account-a' }, calls = [], errors = [], deleted = [];
    let navigated = false;
    const configs = [
      { key: 'layout', save: async () => { calls.push('layout'); return true; } },
      { key: 'export', save: async () => {
        calls.push('export');
        if (scenario === 'account-changed') owner.current = 'account-b';
        return scenario !== 'rejected';
      } },
    ];
    await action('./ManagementContext.tsx', 'saveAndLeave', {
      pending: { action: () => { navigated = true; } }, saving: false,
      currentOwner: owner, setSaving() {}, setSaveError: error => errors.push(error),
      readPending: () => configs, drafts: { current: { delete: key => deleted.push(key) } }, setPending() {},
    })();
    assert.deepEqual(calls, ['layout', 'export']);
    assert.equal(navigated, scenario === 'success');
    assert.deepEqual(deleted, scenario === 'success' ? ['layout', 'export'] : ['layout']);
    assert.equal(errors.some(Boolean), scenario === 'rejected');
  }
});

test('leaving notifications saves all dirty templates and stops at the first rejection', async () => {
  for (const rejected of ['', 'sample']) {
    const calls = [];
    const saved = await action('./NotificationTemplateEditor.tsx', 'saveAll', {
      dirtyRef: { current: ['rd', 'sample', 'personnel'] },
      templates: [{ key: 'rd' }, { key: 'sample' }, { key: 'personnel' }],
      save: async item => { calls.push(item.key); return item.key !== rejected; },
    })();
    assert.equal(saved, !rejected);
    assert.deepEqual(calls, rejected ? ['rd', 'sample'] : ['rd', 'sample', 'personnel']);
  }
});

test('project and laboratory inline saves reject business errors without a false success or persisted local value', async () => {
  const source = ts.createSourceFile('ManagePage.tsx', readFileSync(new URL('../pages/ManagePage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const fields = [];
  const visit = node => {
    if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(source) === 'TextField') fields.push(node);
    ts.forEachChild(node, visit);
  };
  visit(source);
  const attribute = (field, name) => field.attributes.properties.find(node => ts.isJsxAttribute(node) && node.name.getText(source) === name)?.initializer?.expression;
  for (const [object, api, setter] of [['p', 'updateProject', 'setProjectOverviewData'], ['g', 'updateGroup', 'setGroupOverviewData']]) {
    const field = fields.find(node => attribute(node, 'defaultValue')?.getText(source) === `${object}.name`);
    const callback = attribute(field, 'onBlur');
    const code = ts.transpileModule(`const callback = (${callback.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
    const messages = [], writes = [], localChanges = [];
    const env = {
      [object]: { id: 1, name: '原名称' },
      [api]: async (id, patch) => { writes.push({ id, patch }); return { code: 1, message: '关联冲突，原配置保留' }; },
      [setter]: update => localChanges.push(update),
      requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); return response.data; },
      showMessage: (text, error) => messages.push({ text, error }), lp() {}, lg() {},
    };
    await new Function(...Object.keys(env), code + '\nreturn callback;')(...Object.values(env))({ target: { value: '待保存名称' } });
    assert.deepEqual(writes, [{ id: 1, patch: { name: '待保存名称' } }]);
    assert.deepEqual(localChanges, []);
    assert.deepEqual(messages, [{ text: '关联冲突，原配置保留', error: true }]);
  }
});

test('a created division keeps its identity when its separate laboratory relation is rejected', async () => {
  let form = { id: 0, name: '验收部门', code: 'DEMO', group_ids: [1] };
  const messages = [];
  let closed = false;
  await action('../pages/ManagePage.tsx', 'hdiv', {
    divForm: form, createDivision: async () => ({ code: 0, data: { id: 42 } }),
    setDivForm: update => { form = update(form); },
    setDivisionLabs: async () => ({ code: 1, message: '实验室关联冲突' }),
    requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); },
    showMessage: (text, error) => messages.push({ text, error }),
    ldiv() {}, setDivEditOpen: () => { closed = true; },
  })();
  assert.equal(form.id, 42, 'retry must update the created division rather than create a duplicate');
  assert.deepEqual(form.group_ids, [1]);
  assert.equal(closed, false);
  assert.deepEqual(messages, [{ text: '实验室关联冲突', error: true }]);
});
