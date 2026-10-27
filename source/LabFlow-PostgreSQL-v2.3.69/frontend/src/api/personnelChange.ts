import { client, downloadFile } from './http';
import type { ApiResponse } from '../types';

export type PersonnelAction = 'employee_join' | 'employee_leave' | 'internal_transfer' | 'transfer_out' | 'transfer_in' | 'project_add' | 'method_add' | 'notice_only';
export interface PersonnelChangeLab { id: number; name: string; }
export interface PersonnelChangeCapabilities {
  can_view_summary: boolean; can_edit_summary: boolean; can_view_notices: boolean; can_submit: boolean; can_review: boolean;
  can_import_summary: boolean; can_export_summary: boolean; can_edit_config: boolean; view_all_summary: boolean; edit_all_summary: boolean;
}
export interface PersonnelChangeType { name?: string; label: string; action_key: PersonnelAction; enabled: boolean; }
export interface PersonnelChangeSummaryRow { row_key: string; lab_name: string; leader_name: string; person_names: string[]; project_code: string; methods: string[]; }
export interface PersonnelChangeOptions {
  labs: PersonnelChangeLab[]; summary_labs: PersonnelChangeLab[]; summary: PersonnelChangeSummaryRow[]; summary_revision: string;
  lab_projects: Record<string, string[]>; change_types: string[]; change_type_configs: PersonnelChangeType[];
  fields: PersonnelFeedbackField[]; summary_available: boolean; capabilities: PersonnelChangeCapabilities;
}
export interface PersonnelFeedbackField {
  key: string; label: string; field_type: string; required: boolean; enabled: boolean; notify: boolean;
  sort_order: number; change_types: string[]; options: string[];
}
export interface PersonnelFeedbackConfig { change_types: PersonnelChangeType[]; fields: PersonnelFeedbackField[]; capabilities: PersonnelChangeCapabilities; }
export interface PersonnelChangeNotice {
  notice_no: string; created_at: string; actor_username: string; actor_user_id: number | null;
  action_key: PersonnelAction; change_type: string; lab_name: string; target_lab_name: string; person_name: string;
  project_codes: string[]; source_project_codes: string[]; target_project_codes: string[]; transfer_notice_no: string;
  method_names: string[]; new_project_code: string; new_project_name: string; is_high_tech: boolean; high_tech_name: string;
  effective_at: string; notes: string; delivery_status: string; review_status: string; reviewed_by: string;
  reviewed_at: string; review_reason: string; updated_at: string; extra_fields: Record<string, string>;
  revision: number; summary_revision: string; can_review: boolean; can_revise: boolean; can_withdraw: boolean;
}
export interface PersonnelChangeNoticeInput {
  action_key: PersonnelAction; change_type: string; summary_revision: string; lab_name: string; person_name?: string;
  project_codes: string[]; source_project_codes?: string[]; target_lab_name?: string; target_project_codes?: string[];
  transfer_notice_no?: string; method_names: string[]; new_project_code?: string; is_high_tech?: boolean;
  high_tech_name?: string; effective_at: string; notes?: string; extra_fields?: Record<string, string>;
}
export interface PersonnelNoticeQuery { days?: number; start?: string; end?: string; status?: string; keyword?: string; notice_no?: string; }
export interface PersonnelSummaryResult { summary_revision: string; rows: PersonnelChangeSummaryRow[]; }
export interface PersonnelSummaryImportPreview extends PersonnelSummaryResult { preview_id: string; added: number; changed: number; removed: number; expires_at: string; }
export interface PersonnelSummaryEdit { summary_revision: string; reason: string; row: Omit<PersonnelChangeSummaryRow, 'row_key'>; }

export const getPersonnelChangeOptions = (signal?: AbortSignal): Promise<ApiResponse<PersonnelChangeOptions>> => client.get('/personnel-change/options', { signal }).then(response => response.data);
export const getPersonnelChangeNotices = (params: PersonnelNoticeQuery = {}, signal?: AbortSignal): Promise<ApiResponse<PersonnelChangeNotice[]>> => client.get('/personnel-change/notices', { params, signal }).then(response => response.data);
export const createPersonnelChangeNotice = (data: PersonnelChangeNoticeInput): Promise<ApiResponse<PersonnelChangeNotice>> => client.post('/personnel-change/notices', data).then(response => response.data);
export const reviewPersonnelChangeNotice = (noticeNo: string, data: { decision: 'approve' | 'reject'; reason: string; summary_revision: string; notice_revision: number }): Promise<ApiResponse<PersonnelChangeNotice>> => client.post(`/personnel-change/notices/${encodeURIComponent(noticeNo)}/review`, data).then(response => response.data);
export const withdrawPersonnelChangeNotice = (noticeNo: string, data: { reason: string; notice_revision: number }): Promise<ApiResponse<PersonnelChangeNotice>> => client.post(`/personnel-change/notices/${encodeURIComponent(noticeNo)}/withdraw`, data).then(response => response.data);
export const revisePersonnelChangeNotice = (noticeNo: string, data: PersonnelChangeNoticeInput & { reason: string; notice_revision: number }): Promise<ApiResponse<PersonnelChangeNotice>> => client.post(`/personnel-change/notices/${encodeURIComponent(noticeNo)}/revise`, data).then(response => response.data);
export const deletePersonnelChangeNotice = (noticeNo: string, data: { reason: string; notice_revision: number }): Promise<ApiResponse<null>> => client.delete(`/personnel-change/notices/${encodeURIComponent(noticeNo)}`, { data }).then(response => response.data);
export const getPersonnelChangePersonProjects = (labName: string, personName: string, signal?: AbortSignal): Promise<ApiResponse<string[]>> => client.get(`/personnel-change/labs/${encodeURIComponent(labName)}/person-projects`, { params: { person_name: personName }, signal }).then(response => response.data);
export const createPersonnelSummaryRow = (data: PersonnelSummaryEdit): Promise<ApiResponse<PersonnelSummaryResult>> => client.post('/personnel-change/summary/rows', data).then(response => response.data);
export const updatePersonnelSummaryRow = (key: string, data: PersonnelSummaryEdit): Promise<ApiResponse<PersonnelSummaryResult>> => client.put(`/personnel-change/summary/rows/${encodeURIComponent(key)}`, data).then(response => response.data);
export const deletePersonnelSummaryRow = (key: string, data: { summary_revision: string; reason: string }): Promise<ApiResponse<PersonnelSummaryResult>> => client.delete(`/personnel-change/summary/rows/${encodeURIComponent(key)}`, { data }).then(response => response.data);
export const previewPersonnelSummaryImport = (file: File, revision: string): Promise<ApiResponse<PersonnelSummaryImportPreview>> => {
  const data = new FormData(); data.append('file', file); data.append('summary_revision', revision);
  return client.post('/personnel-change/summary/import/preview', data).then(response => response.data);
};
export const confirmPersonnelSummaryImport = (data: { preview_id: string; summary_revision: string; reason: string }): Promise<ApiResponse<PersonnelSummaryResult>> => client.post('/personnel-change/summary/import/confirm', data).then(response => response.data);
export const exportPersonnelChangeSummary = (): Promise<void> => downloadFile('/api/personnel-change/summary/export', {}, '项目及人员汇总表.xlsx');
export const getPersonnelFeedbackConfig = (): Promise<ApiResponse<PersonnelFeedbackConfig>> => client.get('/personnel-change/config').then(response => response.data);
export const updatePersonnelFeedbackConfig = (data: Pick<PersonnelFeedbackConfig, 'change_types' | 'fields'>): Promise<ApiResponse<null>> => client.put('/personnel-change/config', data).then(response => response.data);
