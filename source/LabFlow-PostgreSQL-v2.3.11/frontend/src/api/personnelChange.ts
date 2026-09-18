import { client, downloadFile } from './http';
import type { ApiResponse } from '../types';

export interface PersonnelChangeLab { id: number; name: string; }
export interface PersonnelChangeSummaryRow { lab_name: string; leader_name: string; person_names: string[]; project_code: string; methods: string[]; }
export interface PersonnelChangeOptions { labs: PersonnelChangeLab[]; summary: PersonnelChangeSummaryRow[]; lab_projects: Record<string, string[]>; change_types: string[]; fields: PersonnelFeedbackField[]; summary_available: boolean; }
export interface PersonnelFeedbackField { key: string; label: string; field_type: string; required: boolean; enabled: boolean; notify: boolean; sort_order: number; change_types: string[]; }
export interface PersonnelFeedbackConfig { change_types: { name: string; enabled: boolean }[]; fields: PersonnelFeedbackField[]; }
export interface PersonnelChangeNotice {
  notice_no: string;
  created_at: string;
  actor_username: string;
  change_type: string;
  lab_name: string;
  target_lab_name: string;
  person_name: string;
  project_codes: string[];
  source_project_codes: string[];
  target_project_codes: string[];
  transfer_notice_no: string;
  method_names: string[];
  new_project_code: string;
  new_project_name: string;
  is_high_tech: boolean;
  high_tech_name: string;
  effective_at: string;
  notes: string;
  delivery_status: string;
  review_status: string;
  reviewed_by: string;
  reviewed_at: string;
  review_reason: string;
  updated_at: string;
  extra_fields: Record<string, string>;
}

export interface PersonnelChangeNoticeInput {
  change_type: string;
  lab_name: string;
  person_name?: string;
  project_codes: string[];
  source_project_codes?: string[];
  target_lab_name?: string;
  target_project_codes?: string[];
  transfer_notice_no?: string;
  method_names: string[];
  new_project_code?: string;
  is_high_tech?: boolean;
  high_tech_name?: string;
  effective_at: string;
  notes?: string;
  extra_fields?: Record<string, string>;
}

export const getPersonnelChangeOptions = (): Promise<ApiResponse<PersonnelChangeOptions>> => client.get('/personnel-change/options').then(response => response.data);
export const getPersonnelChangeNotices = (days = 7): Promise<ApiResponse<PersonnelChangeNotice[]>> => client.get('/personnel-change/notices', { params: { days } }).then(response => response.data);
export const createPersonnelChangeNotice = (data: PersonnelChangeNoticeInput): Promise<ApiResponse<PersonnelChangeNotice>> => client.post('/personnel-change/notices', data).then(response => response.data);
export const reviewPersonnelChangeNotice = (noticeNo: string, decision: 'approve' | 'reject', reason = ''): Promise<ApiResponse<PersonnelChangeNotice>> => client.post(`/personnel-change/notices/${encodeURIComponent(noticeNo)}/review`, { decision, reason }).then(response => response.data);
export const deletePersonnelChangeNotice = (noticeNo: string, reason: string): Promise<ApiResponse<null>> => client.delete(`/personnel-change/notices/${encodeURIComponent(noticeNo)}`, { data: { reason } }).then(response => response.data);
export const getPersonnelChangePersonProjects = (labName: string, personName: string): Promise<ApiResponse<string[]>> => client.get(`/personnel-change/labs/${encodeURIComponent(labName)}/person-projects`, { params: { person_name: personName } }).then(response => response.data);
export const importPersonnelChangeSummary = (file: File): Promise<ApiResponse<null>> => {
  const form = new FormData();
  form.append('file', file);
  return client.post('/personnel-change/summary/import', form, { headers: { 'Content-Type': 'multipart/form-data' } }).then(response => response.data);
};
export const exportPersonnelChangeSummary = (): Promise<void> => downloadFile('/api/personnel-change/summary/export', {}, '项目及人员汇总表.xlsx');
export const getPersonnelFeedbackConfig = (): Promise<ApiResponse<PersonnelFeedbackConfig>> => client.get('/personnel-change/config').then(response => response.data);
export const updatePersonnelFeedbackConfig = (data: PersonnelFeedbackConfig): Promise<ApiResponse<null>> => client.put('/personnel-change/config', data).then(response => response.data);
