import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./notificationTemplate.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { notificationTemplateFields, updateNotificationTemplateField, moveNotificationTemplateField, notificationTemplateInput } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('template edits preserve hidden custom settings, source names and independent same-label keys across save and reload', () => {
  const original = {
    key: 'sample_info:icp', name: 'ICP', title: '当前模板', footer: '提示',
    fields: [
      { key: 'notes', label: '管理员自定义注意事项', source_label: '注意事项', visible: true, bold: true, sort_order: 1 },
      { key: 'quantity', label: '旧数量名称', source_label: '送样数量', follow_source_label: true, visible: true, bold: false, sort_order: 2 },
    ],
    available_fields: [{ key: 'custom_notes', label: '注意事项', source_label: '注意事项', follow_source_label: true, visible: false, bold: false, sort_order: 3 }],
  };
  assert.deepEqual(notificationTemplateFields(original).map(field => field.label), ['管理员自定义注意事项', '送样数量', '注意事项']);
  let edited = updateNotificationTemplateField(original, 'notes', { visible: false });
  assert.equal(edited.available_fields.find(field => field.key === 'notes').bold, true);
  assert.equal(edited.available_fields.find(field => field.key === 'notes').label, '管理员自定义注意事项');
  edited = moveNotificationTemplateField(edited, 'custom_notes', 1);
  edited = updateNotificationTemplateField(edited, 'custom_notes', { visible: true, follow_source_label: false, label: '新增补充要求' });
  const payload = notificationTemplateInput(edited);
  assert.deepEqual(payload.fields.map(field => [field.key, field.sort_order, field.visible]), [['custom_notes', 1, true], ['notes', 2, false], ['quantity', 3, true]]);
  assert.equal(payload.fields[0].follow_source_label, false);
  assert.equal(payload.fields[1].follow_source_label, false, 'legacy custom label does not silently become inherited');
  const reloaded = { ...original, ...payload, fields: payload.fields.filter(field => field.visible), available_fields: payload.fields.filter(field => !field.visible) };
  assert.deepEqual(notificationTemplateInput(reloaded), payload);
  const restored = updateNotificationTemplateField(reloaded, 'notes', { visible: true });
  assert.equal(restored.fields.find(field => field.key === 'notes').label, '管理员自定义注意事项');
  assert.equal(restored.fields.find(field => field.key === 'notes').bold, true);
  assert.equal(updateNotificationTemplateField(restored, 'notes', { follow_source_label: true }).fields.find(field => field.key === 'notes').label, '注意事项');
  assert.deepEqual(original.fields.map(field => field.label), ['管理员自定义注意事项', '旧数量名称'], 'editing never mutates the loaded source');
});

test('explicit order changes immediately move the field without duplicate positions or losing hidden candidates', () => {
  const template = { key: 'rd_work_record', name: '研发', title: '标题', footer: '', fields: [
    { key: 'a', label: 'A', visible: true, bold: false, sort_order: 1 },
    { key: 'b', label: 'B', visible: true, bold: true, sort_order: 2 },
  ], available_fields: [{ key: 'c', label: 'C', visible: false, bold: false, sort_order: 3 }] };
  assert.deepEqual(notificationTemplateFields(moveNotificationTemplateField(template, 'b', 1)).map(field => field.key), ['b', 'a', 'c']);
  assert.deepEqual(notificationTemplateFields(moveNotificationTemplateField(template, 'a', 999)).map(field => field.key), ['b', 'c', 'a']);
  assert.deepEqual(notificationTemplateFields(moveNotificationTemplateField(template, 'c', -10)).map(field => field.sort_order), [1, 2, 3]);
  assert.equal(moveNotificationTemplateField(template, 'missing', 1), template);
});
