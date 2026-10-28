import type { NotificationTemplate, NotificationTemplateField, NotificationTemplateInput } from '../types';

export function notificationTemplateFields(template: NotificationTemplate): NotificationTemplateField[] {
  const fields = new Map<string, NotificationTemplateField>();
  for (const field of [...template.fields, ...template.available_fields]) {
    if (!fields.has(field.key)) fields.set(field.key, {
      ...field,
      // An older response does not establish that an administrator intended to follow the source name.
      follow_source_label: field.follow_source_label ?? false,
      label: field.follow_source_label ? field.source_label || field.label : field.label,
    });
  }
  return [...fields.values()].sort((a, b) => a.sort_order - b.sort_order)
    .map((field, index) => ({ ...field, sort_order: index + 1 }));
}

function withFields(template: NotificationTemplate, fields: NotificationTemplateField[]): NotificationTemplate {
  return {
    ...template,
    fields: fields.filter(field => field.visible),
    available_fields: fields.filter(field => !field.visible),
  };
}

export function updateNotificationTemplateField(template: NotificationTemplate, key: string, patch: Partial<NotificationTemplateField>): NotificationTemplate {
  return withFields(template, notificationTemplateFields(template).map(field => {
    if (field.key !== key) return field;
    const updated = { ...field, ...patch };
    return { ...updated, label: updated.follow_source_label ? updated.source_label || updated.label : updated.label };
  }));
}

export function moveNotificationTemplateField(template: NotificationTemplate, key: string, position: number): NotificationTemplate {
  const fields = notificationTemplateFields(template);
  const index = fields.findIndex(field => field.key === key);
  if (index < 0 || !Number.isFinite(position)) return template;
  const [field] = fields.splice(index, 1);
  fields.splice(Math.max(0, Math.min(fields.length, Math.round(position) - 1)), 0, field);
  return withFields(template, fields.map((item, current) => ({ ...item, sort_order: current + 1 })));
}

export function notificationTemplateInput(template: NotificationTemplate): NotificationTemplateInput {
  return { title: template.title, footer: template.footer, fields: notificationTemplateFields(template) };
}
