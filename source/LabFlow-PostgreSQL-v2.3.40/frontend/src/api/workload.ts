import { client, downloadFile } from './http';
import type {
  ApiResponse,
  PaginatedResponse,
  WorkRecord,
  AuditLog,
  RecordEvent,
  StatsSummary,
  UserStats,
  ProjectStats,
  TypeStats,
  InstrumentStats,
  DivisionStats,
  User,
} from '../types';

// --- Records ---
export const getRecords = (params: { start?: string; end?: string; group_id?: number; subject_user_id?: number; page?: number; page_size?: number; include_deleted?: boolean; user_name?: string; detection_division_ids?: string; sending_division_ids?: string; include_pending_ownership?: boolean }): Promise<ApiResponse<PaginatedResponse<WorkRecord>>> =>
  client.get('/records', { params }).then((r) => r.data);

export const getWorkDetectors = (groupId: number): Promise<ApiResponse<User[]>> =>
  client.get('/users/work-detectors', { params: { group_id: groupId } }).then((r) => r.data);

export interface AnalysisPublicAccountCandidate {
  id: number;
  username: string;
  role_names: string[];
}

export interface AnalysisPublicAccountScope {
  accounts: AnalysisPublicAccountCandidate[];
}

export const getAnalysisPublicAccountScope = (): Promise<ApiResponse<AnalysisPublicAccountScope>> =>
  client.get('/users/analysis-public-scope').then((r) => r.data);

export const createRecord = (data: { project_id: number; method_id?: number; user_name: string; sender_user_id?: number; quantity: number; recorded_at: string; group_id?: number; division_id?: number | null; multiplier?: number }): Promise<ApiResponse<WorkRecord>> =>
  client.post('/records', data).then((r) => r.data);

export const deleteRecord = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/records/${id}`, { params: { reason } }).then((r) => r.data);

export const restoreRecord = (id: number): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/records/restore/${id}`).then((r) => r.data);

export const updateRecord = (id: number, data: { user_name?: string; quantity?: number; recorded_at?: string; multiplier?: number; high_item?: string | null }): Promise<ApiResponse<WorkRecord>> =>
  client.put(`/records/${id}`, data).then((r) => r.data);

export const getRecordUsers = (params: { start: string; end: string; detection_division_ids?: string; sending_division_ids?: string; include_pending_ownership?: boolean }): Promise<ApiResponse<string[]>> =>
  client.get('/records/users', { params }).then((r) => r.data);

export const deleteRecordsByUser = (user_name: string, params: { start: string; end: string; group_id?: number; reason?: string }): Promise<ApiResponse<{ deleted_count: number }>> =>
  client.delete(`/records/by-user/${encodeURIComponent(user_name)}`, { params }).then((r) => r.data);

// --- Stats ---
export type OwnershipBasis = 'execution' | 'submitted' | 'project';
export type OwnershipQuery = {
  division_id?: number;
  ownership_basis?: OwnershipBasis;
  include_pending_ownership?: boolean;
  detection_division_ids?: number[] | string;
  sending_division_ids?: number[] | string;
  sheet_ids?: string;
};
export type PreviewQuery = { start: string; end: string; group_id?: number } & OwnershipQuery;
export type WorkStatsQuery = { start?: string; end?: string; group_id?: number; group_by?: string } & OwnershipQuery;
export type SampleInfoStatsQuery = { start?: string; end?: string; type_key?: string; status?: string } & OwnershipQuery;

export const getStatsSummary = (params?: WorkStatsQuery): Promise<ApiResponse<StatsSummary>> =>
  client.get('/stats/summary', { params }).then((r) => r.data);

export const getStatsByUser = (params?: WorkStatsQuery): Promise<ApiResponse<UserStats[]>> =>
  client.get('/stats/by-user', { params }).then((r) => r.data);

export const getStatsByProject = (params?: WorkStatsQuery): Promise<ApiResponse<ProjectStats[]>> =>
  client.get('/stats/by-project', { params }).then((r) => r.data);

export const getStatsByType = (params?: WorkStatsQuery): Promise<ApiResponse<TypeStats[]>> =>
  client.get('/stats/by-type', { params }).then((r) => r.data);

export const getStatsByInstrument = (params?: WorkStatsQuery): Promise<ApiResponse<InstrumentStats[]>> =>
  client.get('/stats/by-instrument', { params }).then((r) => r.data);

// v0.4.28: 浜嬩笟閮ㄧ粺璁?
export const getStatsByDivision = (params?: WorkStatsQuery): Promise<ApiResponse<DivisionStats[]>> =>
  client.get('/stats/by-division', { params }).then((r) => r.data);

export type ExportFilterDivision = { id: number; name: string };
export type ExportFilterOptions = {
  detection_departments: ExportFilterDivision[];
  sending_departments: ExportFilterDivision[];
};

export const getExportFilterOptions = (params: {
  start: string;
  end: string;
  detection_division_ids?: number[] | string;
  include_pending_ownership?: boolean;
}): Promise<ApiResponse<ExportFilterOptions>> =>
  client.get('/export/filter-options', { params }).then((r) => r.data);

// --- Export ---
export const exportExcel = (params: WorkStatsQuery): Promise<void> =>
  downloadFile('/api/export/excel', params, `分析检测统计_${params.start?.substring(0, 10) ?? ''}_${params.end?.substring(0, 10) ?? ''}.xlsx`);

// --- Audit ---
export const getAuditLogs = (params?: { page?: number; page_size?: number; module?: 'work' | 'rd' | 'sample_info' | 'shared'; action?: string; user_name?: string; business_no?: string }): Promise<ApiResponse<PaginatedResponse<AuditLog>>> =>
  client.get('/audit-logs', { params }).then((r) => r.data);

export const getRecordTrace = (module: 'work' | 'rd' | 'sample-info', id: number): Promise<ApiResponse<RecordEvent[]>> =>
  client.get(`/trace/${module}/${id}`).then((r) => r.data);
