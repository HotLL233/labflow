import type { PersonnelAction, PersonnelChangeNoticeInput, PersonnelChangeSummaryRow, PersonnelFeedbackField } from '../api/personnelChange';
import { beijingRecordToday, validRecordDate } from './recordDateRange';

export const PERSONNEL_ACTIONS: { key: PersonnelAction; label: string }[] = [
  { key: 'employee_join', label: '员工加入' }, { key: 'employee_leave', label: '员工离职' },
  { key: 'internal_transfer', label: '实验室内人员调动' }, { key: 'transfer_out', label: '跨实验室人员调出' },
  { key: 'transfer_in', label: '跨实验室人员调入' }, { key: 'project_add', label: '实验室新增项目' },
  { key: 'method_add', label: '实验室新增方法' }, { key: 'notice_only', label: '仅通知，不同步汇总表' },
];
export const PERSONNEL_BUILTIN_FIELDS: [string, string, string][] = [
  ['change_type', '变动类型', 'select'], ['lab_name', '实验室', 'text'], ['person_name', '人员', 'text'],
  ['project_codes', '关联项目', 'multi-select'], ['source_project_codes', '来源项目', 'multi-select'],
  ['target_lab_name', '目标实验室', 'text'], ['target_project_codes', '目标项目', 'multi-select'],
  ['transfer_notice_no', '关联调出通知编号', 'text'], ['new_project_code', '新增项目代号', 'text'],
  ['method_names', '检测方法', 'multi-text'], ['is_high_tech', '高新项目', 'boolean'],
  ['high_tech_name', '高新项目名称', 'text'], ['effective_at', '记录日期', 'date'], ['notes', '备注', 'textarea'],
];
export const PERSONNEL_LOCKED_FIELDS = new Set(['change_type', 'lab_name', 'person_name', 'project_codes', 'method_names', 'effective_at']);
export const initialPersonnelForm = (now?: Date): PersonnelChangeNoticeInput => ({
  action_key: 'employee_join', change_type: '员工加入', summary_revision: '', lab_name: '', person_name: '',
  project_codes: [], source_project_codes: [], target_lab_name: '', target_project_codes: [], transfer_notice_no: '',
  method_names: [], new_project_code: '', is_high_tech: false, high_tech_name: '',
  effective_at: beijingRecordToday(now), notes: '', extra_fields: {},
});
export const personnelIdentity = (form: PersonnelChangeNoticeInput) => JSON.stringify([form.action_key, form.lab_name, form.person_name?.trim() || '']);
export function changePersonnelForm(form: PersonnelChangeNoticeInput, patch: Partial<PersonnelChangeNoticeInput>): PersonnelChangeNoticeInput {
  const next = { ...form, ...patch };
  if (personnelIdentity(next) !== personnelIdentity(form)) {
    next.project_codes = []; next.source_project_codes = []; next.target_project_codes = [];
    if (next.action_key !== form.action_key) { next.method_names = []; next.extra_fields = {}; next.transfer_notice_no = ''; next.new_project_code = ''; next.is_high_tech = false; next.high_tech_name = ''; }
  } else if (next.target_lab_name !== form.target_lab_name) next.target_project_codes = [];
  return next;
}
export function applyPersonnelProjectDefaults(form: PersonnelChangeNoticeInput, identity: string, projects: string[], touched: boolean): PersonnelChangeNoticeInput {
  if (touched || personnelIdentity(form) !== identity) return form;
  if (form.action_key === 'employee_leave' || form.action_key === 'transfer_out') return { ...form, project_codes: [...projects] };
  if (form.action_key === 'internal_transfer') return { ...form, source_project_codes: [...projects] };
  return form;
}
export function personnelRequiredFields(form: PersonnelChangeNoticeInput): Set<string> {
  const required = new Set(['change_type', 'lab_name', 'effective_at']);
  if (['employee_join', 'employee_leave', 'internal_transfer', 'transfer_out', 'transfer_in'].includes(form.action_key)) required.add('person_name');
  if (form.action_key === 'employee_join' || form.action_key === 'method_add') required.add('project_codes');
  if (form.action_key === 'internal_transfer') { required.add('source_project_codes'); required.add('target_project_codes'); }
  if (form.action_key === 'transfer_in') { required.add('target_lab_name'); required.add('target_project_codes'); required.add('transfer_notice_no'); }
  if (form.action_key === 'project_add' || form.action_key === 'method_add') required.add('method_names');
  if (form.action_key === 'project_add') required.add('new_project_code');
  if (form.is_high_tech && form.action_key === 'project_add') required.add('high_tech_name');
  return required;
}
export function personnelFormFields(form: PersonnelChangeNoticeInput, config: PersonnelFeedbackField[]): PersonnelFeedbackField[] {
  if (config.find(field => field.key === 'is_high_tech')?.enabled === false) form = { ...form, is_high_tech: false };
  const required = personnelRequiredFields(form);
  const keys = new Set(['change_type', 'lab_name', 'effective_at', 'notes']);
  if (['employee_join', 'employee_leave', 'internal_transfer', 'transfer_out', 'transfer_in', 'project_add'].includes(form.action_key)) keys.add('person_name');
  if (['employee_join', 'employee_leave', 'transfer_out', 'method_add'].includes(form.action_key)) keys.add('project_codes');
  if (form.action_key === 'internal_transfer') { keys.add('source_project_codes'); keys.add('target_project_codes'); }
  if (form.action_key === 'transfer_in') { keys.add('target_project_codes'); keys.add('transfer_notice_no'); }
  if (form.action_key === 'project_add') { keys.add('new_project_code'); keys.add('method_names'); keys.add('is_high_tech'); if (form.is_high_tech) keys.add('high_tech_name'); }
  if (form.action_key === 'method_add') keys.add('method_names');
  // Explicit administrator-required fields must be fillable even outside the usual action layout.
  for (const field of config) if (field.enabled && field.required && (!field.change_types.length || field.change_types.includes(form.action_key) || field.change_types.includes(form.change_type))) keys.add(field.key);
  if (form.action_key === 'transfer_in') keys.delete('target_lab_name'); // The lab selector already edits the target and submitting lab together.
  const builtins = new Set(PERSONNEL_BUILTIN_FIELDS.map(([key]) => key));
  const fields = PERSONNEL_BUILTIN_FIELDS.filter(([key]) => keys.has(key)).map(([key, label, field_type], index) => {
    const custom = config.find(field => field.key === key);
    const field = { key, label, field_type, required: false, enabled: true, notify: true, sort_order: index, change_types: [], options: [], ...custom };
    return { ...field, enabled: required.has(key) || PERSONNEL_LOCKED_FIELDS.has(key) || field.enabled, required: required.has(key) || field.required };
  });
  return [...fields, ...config.filter(field => !builtins.has(field.key))]
    .filter(field => field.enabled && (required.has(field.key) || !field.change_types.length || field.change_types.includes(form.action_key) || field.change_types.includes(form.change_type)))
    .sort((a, b) => a.sort_order - b.sort_order);
}
export function personnelFieldValue(form: PersonnelChangeNoticeInput, key: string): string | string[] | boolean {
  if (PERSONNEL_BUILTIN_FIELDS.some(([builtin]) => builtin === key)) return (form as unknown as Record<string, string | string[] | boolean>)[key] ?? '';
  return form.extra_fields?.[key] ?? '';
}
export function parsePersonnelChoices(value: string): string[] {
  if (!value) return [];
  try { const parsed: unknown = JSON.parse(value); return Array.isArray(parsed) && parsed.every(item => typeof item === 'string') ? parsed : []; } catch { return []; }
}
export function validatePersonnelForm(form: PersonnelChangeNoticeInput, config: PersonnelFeedbackField[]): string {
  if (form.action_key === 'transfer_in' && !form.target_lab_name) return '请选择目标实验室';
  for (const field of personnelFormFields(form, config)) {
    const value = personnelFieldValue(form, field.key);
    const choices = Array.isArray(value) ? value : field.field_type === 'multi-select' || field.field_type === 'multi-text' ? parsePersonnelChoices(String(value)) : null;
    if (field.required && (choices ? choices.length === 0 || choices.some(item => !item.trim()) : typeof value !== 'boolean' && !String(value).trim())) return `请填写${field.label}`;
    if (field.field_type === 'date' && value && !validRecordDate(String(value))) return `${field.label}不是有效日期`;
    const builtin = PERSONNEL_BUILTIN_FIELDS.some(([key]) => key === field.key);
    if (!builtin && value && field.field_type === 'boolean' && !['true', 'false'].includes(String(value))) return `请选择${field.label}的是或否`;
    if (!builtin && value && field.field_type === 'select' && !field.options.includes(String(value))) return `${field.label}包含不可用选项，请重新选择`;
    if (!builtin && choices && field.field_type === 'multi-select' && choices.some(item => !field.options.includes(item))) return `${field.label}包含不可用选项，请重新选择`;
  }
  if (form.method_names.some(method => /[，,、；;\r\n]/.test(method))) return '检测方法请逐条填写，不要在一条内使用逗号、顿号、分号或换行';
  return '';
}
export function personnelSubmission(form: PersonnelChangeNoticeInput, config: PersonnelFeedbackField[]): PersonnelChangeNoticeInput {
  const visible = new Set(personnelFormFields(form, config).map(field => field.key));
  const builtinKeys = new Set(PERSONNEL_BUILTIN_FIELDS.map(([key]) => key));
  return { ...form, person_name: visible.has('person_name') ? form.person_name : '', project_codes: visible.has('project_codes') ? form.project_codes : [],
    source_project_codes: visible.has('source_project_codes') ? form.source_project_codes : [], target_project_codes: visible.has('target_project_codes') ? form.target_project_codes : [],
    target_lab_name: form.action_key === 'transfer_in' || visible.has('target_lab_name') ? form.target_lab_name : '', transfer_notice_no: visible.has('transfer_notice_no') ? form.transfer_notice_no : '',
    new_project_code: visible.has('new_project_code') ? form.new_project_code : '', method_names: visible.has('method_names') ? form.method_names : [],
    notes: visible.has('notes') ? form.notes : '', is_high_tech: visible.has('is_high_tech') ? form.is_high_tech : false,
    high_tech_name: visible.has('high_tech_name') ? form.high_tech_name : '',
    extra_fields: Object.fromEntries(Object.entries(form.extra_fields || {}).filter(([key]) => visible.has(key) && !builtinKeys.has(key))) };
}

export function personnelSummaryChanges(beforeRows: PersonnelChangeSummaryRow[], afterRows: PersonnelChangeSummaryRow[]) {
  const before = new Map(beforeRows.map(row => [row.row_key, row]));
  const changes: { kind: string; before?: PersonnelChangeSummaryRow; after?: PersonnelChangeSummaryRow }[] = [];
  const content = (row: PersonnelChangeSummaryRow) => JSON.stringify([row.lab_name, row.project_code, row.leader_name, row.person_names, row.methods]);
  for (const row of afterRows) {
    const previous = before.get(row.row_key); before.delete(row.row_key);
    if (!previous || content(previous) !== content(row)) changes.push({ kind: previous ? '修改' : '新增', before: previous, after: row });
  }
  for (const row of before.values()) changes.push({ kind: '删除', before: row });
  return changes;
}
