import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('NotificationTemplateEditor.tsx', readFileSync(new URL('./NotificationTemplateEditor.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function action(name, environment) {
  let callback;
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) {
      callback = ts.isCallExpression(node.initializer) ? node.initializer.arguments[0] : node.initializer;
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(callback, `missing editor action ${name}`);
  const code = ts.transpileModule(`(${callback.getText(source)})`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `return ${code}`)(...Object.values(environment));
}
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

test('refresh and preview responses cannot replace later draft edits; refresh never discards a dirty template implicitly', async () => {
  const fetch = deferred(), writes = [], messages = [];
  const environment = {
    loadRequest: { current: 0 }, previewRequest: { current: 0 }, editRevision: { current: 0 }, busyRef: { current: false },
    getNotificationTemplates: () => fetch.promise,
    setLoading: value => writes.push(['loading', value]),
    setTemplates: value => writes.push(['templates', value]),
    setSelected: value => writes.push(['selected', value]),
    setDirtyKeys: value => writes.push(['dirty', value]),
    setPreview: value => writes.push(['preview', value]),
    onMessage: value => messages.push(value),
  };
  const pending = action('load', environment)();
  environment.editRevision.current++;
  fetch.resolve({ code: 0, data: [{ key: 'rd_work_record', title: '远端旧值' }] });
  await pending;
  assert.equal(writes.some(([key]) => key === 'templates' || key === 'dirty'), false);
  assert.match(messages[0].text, /保留草稿/);
  let reloads = 0, confirmed = false;
  action('requestRefresh', { busyRef: { current: false }, dirtyRef: { current: ['rd_work_record'] }, setRefreshConfirm: value => { confirmed = value; }, load: () => { reloads++; } })();
  assert.equal(confirmed, true);
  assert.equal(reloads, 0);

  const result = deferred(), previewRequest = { current: 0 }, previews = [];
  const preview = action('showPreview', {
    template: { key: 'rd_work_record' }, valid: () => true, busyRef: { current: false }, previewRequest,
    notificationTemplateInput: value => value, setPreviewing: () => {},
    previewBusinessNotificationTemplate: () => result.promise, setPreview: value => previews.push(value), onMessage: () => {},
  })();
  previewRequest.current++;
  result.resolve({ code: 0, data: { title: '旧预览', markdown: '旧内容' } });
  await preview;
  assert.deepEqual(previews, []);

  const saveResult = deferred(), revision = { current: 0 }, savedWrites = [];
  const saving = action('save', {
    template: { key: 'rd_work_record' }, valid: () => true, busyRef: { current: false }, editRevision: revision,
    notificationTemplateInput: () => ({ title: '提交时标题', fields: [], footer: '' }), setSaving: () => {},
    updateBusinessNotificationTemplate: () => saveResult.promise, setTemplates: value => savedWrites.push(value),
    setDirtyKeys: value => savedWrites.push(value), onMessage: () => {},
  })();
  revision.current++;
  saveResult.resolve({ code: 0 });
  await saving;
  assert.deepEqual(savedWrites, [], 'a successful old save cannot mark later edits clean');
});
