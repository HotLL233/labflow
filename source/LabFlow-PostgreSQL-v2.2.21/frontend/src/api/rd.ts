import { client, downloadFile } from './http';
import type { PreviewQuery, WorkStatsQuery } from './workload';
import type {
  ApiResponse,
  PaginatedResponse,
  WorkRecord,
  StatsSummary,
  UserStats,
  ProjectStats,
  TypeStats,
  InstrumentStats,
  DivisionStats,
  User,
  Sheet1Data,
  Sheet2Row,
  Sheet3Row,
  Sheet4Row,
  Sheet5Row,
  Sheet6Row,
  Sheet7Row,
  Sheet8Row,
  Sheet9Row,
  Sheet10Row,
  Sheet11Row,
} from '../types';

// --- RD Records ---
export const getRdRecords = (params: { start?: string; end?: string; group_id?: number; page?: number; page_size?: number; include_deleted?: boolean; user_name?: string; sort_by?: string; sort_dir?: 'asc' | 'desc' }): Promise<ApiResponse<PaginatedResponse<WorkRecord>>> =>
  client.get('/rd-records', { params }).then((r) => r.data);

export const getRdSenders = (groupId: number): Promise<ApiResponse<User[]>> =>
  client.get('/users/rd-senders', { params: { group_id: groupId } }).then((r) => r.data);

export const createRdRecord = (data: { project_id: number; method_id: number; detection_type: string; user_name: string; sender_user_id?: number; quantity: number; recorded_at: string; group_id?: number; division_id?: number | null; batch_no?: string; notes?: string; extra_fields?: Record<string, any> }): Promise<ApiResponse<WorkRecord>> =>
  client.post('/rd-records', data).then((r) => r.data);

export const deleteRdRecord = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/rd-records/${id}`, { params: { reason } }).then((r) => r.data);

export const restoreRdRecord = (id: number): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/rd-records/restore/${id}`).then((r) => r.data);

export const updateRdRecord = (id: number, data: { user_name?: string; quantity?: number; recorded_at?: string; multiplier?: number; project_id?: number; method_id?: number | null; group_id?: number | null; division_id?: number | null; batch_no?: string; notes?: string; high_item?: string | null; extra_fields?: Record<string, any> }): Promise<ApiResponse<WorkRecord>> =>
  client.put(`/rd-records/${id}`, data).then((r) => r.data);

export const deleteRdRecordsByUser = (user_name: string, params: { start: string; end: string; group_id?: number; reason?: string }): Promise<ApiResponse<{ deleted_count: number }>> =>
  client.delete(`/rd-records/by-user/${encodeURIComponent(user_name)}`, { params }).then((r) => r.data);

export const sampleRdRecord = (id: number, subjectUserId?: number): Promise<ApiResponse<WorkRecord>> =>
  client.put(`/rd-records/${id}/sample`, subjectUserId ? { subject_user_id: subjectUserId } : {}).then(r => r.data);

export const withdrawRdRecordSample = (id: number, reason: string, subjectUserId?: number): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/rd-records/${id}/withdraw-sample`, { reason, subject_user_id: subjectUserId }).then(r => r.data);

export const returnRdRecord = (id: number, reason: string): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/rd-records/${id}/return`, { reason }).then(r => r.data);

export const confirmRdRecordReturn = (id: number): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/rd-records/${id}/confirm-return`).then(r => r.data);

export const getRdRecordUsers = (params: { start: string; end: string }): Promise<ApiResponse<string[]>> =>
  client.get('/rd-records/users', { params }).then((r) => r.data);

// --- RD Stats ---
export const getRdStatsSummary = (params?: WorkStatsQuery): Promise<ApiResponse<StatsSummary>> =>
  client.get('/rd-stats/summary', { params }).then((r) => r.data);

export const getRdStatsByUser = (params?: WorkStatsQuery): Promise<ApiResponse<UserStats[]>> =>
  client.get('/rd-stats/by-user', { params }).then((r) => r.data);

export const getRdStatsByProject = (params?: WorkStatsQuery): Promise<ApiResponse<ProjectStats[]>> =>
  client.get('/rd-stats/by-project', { params }).then((r) => r.data);

export const getRdStatsByType = (params?: WorkStatsQuery): Promise<ApiResponse<TypeStats[]>> =>
  client.get('/rd-stats/by-type', { params }).then((r) => r.data);

export const getRdStatsByInstrument = (params?: WorkStatsQuery): Promise<ApiResponse<InstrumentStats[]>> =>
  client.get('/rd-stats/by-instrument', { params }).then((r) => r.data);

export const getRdStatsByDivision = (params?: WorkStatsQuery): Promise<ApiResponse<DivisionStats[]>> =>
  client.get('/rd-stats/by-division', { params }).then((r) => r.data);

// --- RD Export ---
export const exportRdExcel = (params: WorkStatsQuery): Promise<void> =>
  downloadFile('/api/rd-export/excel', params, `研发送样统计_${params.start?.substring(0, 10) ?? ''}_${params.end?.substring(0, 10) ?? ''}.xlsx`);

// --- RD Export Preview ---
export const getRdPreviewSheet1 = (params: PreviewQuery): Promise<ApiResponse<Sheet1Data>> =>
  client.get('/rd-export/preview/sheet1', { params }).then((r) => r.data);

export const getRdPreviewSheet2 = (params: PreviewQuery): Promise<ApiResponse<Sheet2Row[]>> =>
  client.get('/rd-export/preview/sheet2', { params }).then((r) => r.data);

export const getRdPreviewSheet3 = (params: PreviewQuery): Promise<ApiResponse<Sheet3Row[]>> =>
  client.get('/rd-export/preview/sheet3', { params }).then((r) => r.data);

export const getRdPreviewSheet4 = (params: PreviewQuery): Promise<ApiResponse<Sheet4Row[]>> =>
  client.get('/rd-export/preview/sheet4', { params }).then((r) => r.data);

export const getRdPreviewSheet5 = (params: PreviewQuery): Promise<ApiResponse<Sheet5Row[]>> =>
  client.get('/rd-export/preview/sheet5', { params }).then((r) => r.data);

export const getRdPreviewSheet6 = (params: PreviewQuery): Promise<ApiResponse<Sheet6Row[]>> =>
  client.get('/rd-export/preview/sheet6', { params }).then((r) => r.data);

export const getRdPreviewSheet7 = (params: PreviewQuery): Promise<ApiResponse<Sheet7Row[]>> =>
  client.get('/rd-export/preview/sheet7', { params }).then((r) => r.data);

export const getRdPreviewSheet8 = (params: PreviewQuery): Promise<ApiResponse<Sheet8Row[]>> =>
  client.get('/rd-export/preview/sheet8', { params }).then((r) => r.data);

export const getRdPreviewSheet9 = (params: PreviewQuery): Promise<ApiResponse<Sheet9Row[]>> =>
  client.get('/rd-export/preview/sheet9', { params }).then((r) => r.data);

export const getRdPreviewSheet10 = (params: PreviewQuery): Promise<ApiResponse<Sheet10Row[]>> =>
  client.get('/rd-export/preview/sheet10', { params }).then((r) => r.data);

export const getRdPreviewSheet11 = (params: PreviewQuery): Promise<ApiResponse<Sheet11Row[]>> =>
  client.get('/rd-export/preview/sheet11', { params }).then((r) => r.data);

