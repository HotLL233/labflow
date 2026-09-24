import { client } from './http';
import type {
  ApiResponse,
  PaginatedResponse,
  SampleRecord,
  SampleStats,
  BackupStatus,
  NotificationChannel,
  NotificationRule,
  NotificationDelivery,
  NotificationSummary,
  InAppNotification,
  NotificationTemplate,
} from '../types';

export const getServerTime = (): Promise<ApiResponse<{ unix_ms: number; beijing_time: string }>> =>
  client.get('/server-time').then((r) => r.data);

// --- Samples ---
export const getSamples = (params?: { group_id?: number; user_name?: string; page?: number; page_size?: number }): Promise<ApiResponse<PaginatedResponse<SampleRecord>>> =>
  client.get('/samples', { params }).then((r) => r.data);

export const getSample = (id: number): Promise<ApiResponse<SampleRecord>> =>
  client.get(`/samples/${id}`).then((r) => r.data);

export const createSample = (data: { project_id: number; user_name: string; sample_name: string; sample_count: number; submitted_at: string; unit?: string; batch_no?: string; notes?: string }): Promise<ApiResponse<SampleRecord>> =>
  client.post('/samples', data).then((r) => r.data);

export const updateSample = (id: number, data: { sample_name?: string; sample_count?: number; unit?: string; batch_no?: string; notes?: string; submitted_at?: string }): Promise<ApiResponse<SampleRecord>> =>
  client.put(`/samples/${id}`, data).then((r) => r.data);

export const deleteSample = (id: number): Promise<ApiResponse<null>> =>
  client.delete(`/samples/${id}`).then((r) => r.data);

export const restoreSample = (id: number): Promise<ApiResponse<null>> =>
  client.post(`/samples/${id}/restore`).then((r) => r.data);

export const getSampleStats = (params?: { start?: string; end?: string }): Promise<ApiResponse<SampleStats>> =>
  client.get('/samples/stats', { params }).then((r) => r.data);

// --- Auth ---
export const adminLogin = (data: { username: string; password: string; keep_signed_in?: boolean }): Promise<ApiResponse<{ token: string }>> =>
  client.post('/auth/login', data).then((r) => r.data);

// --- Backup ---
export const getBackupStatus = (): Promise<ApiResponse<BackupStatus>> => client.get('/backup/status').then((r) => r.data);
export const getNotificationSummary = (): Promise<ApiResponse<NotificationSummary>> => client.get('/notifications/summary').then(r => r.data);
export const getNotificationTemplates = (): Promise<ApiResponse<NotificationTemplate[]>> => client.get('/notifications/business-templates').then(r => r.data);
export const updateBusinessNotificationTemplate = (key: string, data: Pick<NotificationTemplate, 'title' | 'fields' | 'footer'>): Promise<ApiResponse<null>> => client.put(`/notifications/business-templates/${key}`, data).then(r => r.data);
export const getNotificationChannels = (): Promise<ApiResponse<NotificationChannel[]>> => client.get('/notifications/channels').then(r => r.data);
export const createNotificationChannel = (data: { name: string; webhook_url: string; secret?: string; is_active?: boolean }): Promise<ApiResponse<number>> => client.post('/notifications/channels', data).then(r => r.data);
export const updateNotificationChannel = (id: number, data: { name: string; webhook_url?: string; secret?: string; is_active?: boolean }): Promise<ApiResponse<null>> => client.put(`/notifications/channels/${id}`, data).then(r => r.data);
export const testNotificationChannel = (id: number): Promise<ApiResponse<string>> => client.post(`/notifications/channels/${id}/test`).then(r => r.data);
export const getNotificationRules = (): Promise<ApiResponse<NotificationRule[]>> => client.get('/notifications/rules').then(r => r.data);
type NotificationTargetKind = 'rd_work_record' | 'rd_work_record_rejected' | 'rd_work_record_resubmitted' | 'sample_info' | 'personnel_change_feedback' | 'personnel_change_feedback_rejected';
export const createNotificationRule = (data: { name: string; group_id?: number | null; group_ids?: number[]; project_name?: string; detection_type?: string; sample_info_type_key?: string; targets: { target_kind: NotificationTargetKind; sample_info_type_key: string }[]; channel_id?: number | null; recipient_user_ids: number[]; is_default?: boolean; is_active?: boolean }): Promise<ApiResponse<number>> => client.post('/notifications/rules', data).then(r => r.data);
export const updateNotificationRule = (id: number, data: { name: string; group_id?: number | null; group_ids?: number[]; project_name?: string; detection_type?: string; sample_info_type_key?: string; targets: { target_kind: NotificationTargetKind; sample_info_type_key: string }[]; channel_id?: number | null; recipient_user_ids: number[]; is_default?: boolean; is_active?: boolean }): Promise<ApiResponse<null>> => client.put(`/notifications/rules/${id}`, data).then(r => r.data);
export const getNotificationDeliveries = (): Promise<ApiResponse<NotificationDelivery[]>> => client.get('/notifications/deliveries').then(r => r.data);
export const processNotifications = (): Promise<ApiResponse<string>> => client.post('/notifications/process').then(r => r.data);
export const getNotificationContentTemplate = (eventKey: string): Promise<ApiResponse<{ event_key: string; title: string; footer: string; fields: any[]; available_fields: { key: string; label: string }[] }>> => client.get(`/notifications/content-template/${eventKey}`).then(r => r.data);
export const updateNotificationContentTemplate = (eventKey: string, data: { title: string; footer: string; fields: any[] }): Promise<ApiResponse<null>> => client.put(`/notifications/content-template/${eventKey}`, data).then(r => r.data);
export const getNotificationInbox = (): Promise<ApiResponse<InAppNotification[]>> => client.get('/notifications/inbox').then(r => r.data);
export const markNotificationRead = (id: number): Promise<ApiResponse<null>> => client.put(`/notifications/inbox/${id}/read`).then(r => r.data);
export const backupNow = (): Promise<ApiResponse<string>> => client.post('/backup/now').then((r) => r.data);
export const getBackupConfig = (): Promise<ApiResponse<{ enabled: boolean; interval_hours: number; max_backup_count: number; mode: 'database' | 'full'; sync_dir?: string | null }>> => client.get('/backup/config').then((r) => r.data);
export const updateBackupConfig = (data: { enabled: boolean; interval_hours: number; max_backup_count?: number; mode?: 'database' | 'full'; sync_dir?: string | null }): Promise<ApiResponse<string>> => client.put('/backup/config', data).then((r) => r.data);
export const testBackupSync = (sync_dir: string): Promise<ApiResponse<string>> => client.post('/backup/test-sync', { sync_dir }).then((r) => r.data);
export const deleteBackup = (filename: string, reason?: string): Promise<ApiResponse<string>> =>
  client.delete(`/backup/file/${encodeURIComponent(filename)}`, { params: { reason } }).then((r) => r.data);
export const restoreBackup = (file: File): Promise<ApiResponse<string>> => { const fd = new FormData(); fd.append('file', file); return client.post('/backup/restore', fd, { headers: { 'Content-Type': 'multipart/form-data' } }).then((r) => r.data); };
export const restoreBackupFile = (filename: string): Promise<ApiResponse<string>> => client.post(`/backup/restore/${encodeURIComponent(filename)}`).then((r) => r.data);
export const restartAfterRestore = (): Promise<ApiResponse<string>> => client.post('/backup/restart').then((r) => r.data);
