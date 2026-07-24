import axios from 'axios';
import type { PermissionDef } from '../constants/permissions';
import type {
  ApiResponse,
  PaginatedResponse,
  ProjectGroup,
  Project,
  Method,
  Instrument,
  WorkRecord,
  SampleRecord,
  SampleInfoRecord,
  SampleInfoColumn,
  SampleInfoColumnVisibility,
  SampleInfoAttachment,
  SampleInfoType,
  SampleStats,
  AuditLog,
  RecordEvent,
  StatsSummary,
  UserStats,
  ProjectStats,
  TypeStats,
  InstrumentStats,
  DivisionStats,
  BackupStatus,
  MethodType,
  ImportSummary,
  ImportMapping,
  HelpDocument,
  HelpArticle,
  Division,
  User,
  LoginRequest,
  LoginResponse,
  UserUpdate,
  UserSession,
  ColumnVisibilityItem,
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
  Role,
  RoleWithPermissions,
  RdRecordColumn,
  SystemSetting,
  MasterImportPreview,
  MasterImportResult,
  GovernanceModule,
  GovernanceSummary,
  GovernanceImportPreview,
  GovernancePurgePreview,
  GovernancePurgeResult,
  TrashEntry,
  TrashPrecheck,
  LogMaintenanceStatus,
  LogMaintenancePolicyView,
  MaintenanceJobResult,
  LogArchiveBatch,
  NotificationChannel,
  NotificationRule,
  NotificationDelivery,
  NotificationSummary,
  InAppNotification,
} from '../types';

const client = axios.create({ baseURL: '/api' });

const clearStoredSession = () => {
  localStorage.removeItem('workload_token');
  localStorage.removeItem('workload_user');
  localStorage.removeItem('workload_remember');
  sessionStorage.removeItem('workload_token');
  sessionStorage.removeItem('workload_user');
};

const getDownloadFilename = (disposition: string, fallback: string): string => {
  const encodedName = disposition.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  if (encodedName) {
    try { return decodeURIComponent(encodedName); } catch {}
  }
  return disposition.match(/filename="?([^";]+)"?/i)?.[1] || fallback;
};

client.interceptors.response.use(
  (res) => {
    const message = typeof res.data?.message === 'string' ? res.data.message : '';
    if (res.data?.code === 1003 && ['其他设备退出', '会话已撤销', '会话已过期'].some(text => message.includes(text))) {
      clearStoredSession();
      sessionStorage.setItem('workload_session_notice', message);
      if (window.location.pathname !== '/login') {
        window.location.replace('/login');
      }
      return Promise.reject(new Error(message));
    }
    return res;
  },
  (err) => {
    // v0.4.27-A: 401 鏃舵竻闄ょ櫥褰曟€?
    if (err.response?.status === 401) {
      clearStoredSession();
    }
    const msg = err.response?.data?.message || '缃戠粶閿欒';
    return Promise.reject(new Error(msg));
  }
);

// v0.4.27-A: 璇锋眰鎷︽埅鍣?鈥?鑷姩闄勫姞 JWT token
client.interceptors.request.use((config) => {
  const token =
    localStorage.getItem('workload_token') ||
    sessionStorage.getItem('workload_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// --- Groups ---
export const getGroups = (): Promise<ApiResponse<ProjectGroup[]>> =>
  client.get('/groups').then((r) => r.data);

export const createGroup = (data: { name: string; sort_order?: number; show_in_work?: boolean; show_in_rd?: boolean; division_id?: number | null }): Promise<ApiResponse<ProjectGroup>> =>
  client.post('/groups', data).then((r) => r.data);

export const updateGroup = (id: number, data: { name?: string; sort_order?: number; show_in_work?: boolean; show_in_rd?: boolean; division_id?: number | null }): Promise<ApiResponse<ProjectGroup>> =>
  client.put(`/groups/${id}`, data).then((r) => r.data);

export const deleteGroup = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/groups/${id}`, { params: { reason } }).then((r) => r.data);

// ========== v0.4.24: 浜嬩笟閮?CRUD ==========
export const getDivisions = (): Promise<ApiResponse<Division[]>> =>
  client.get('/divisions').then((r) => r.data);

export const createDivision = (data: { name: string; sort_order?: number; color?: string }): Promise<ApiResponse<Division>> =>
  client.post('/divisions', data).then((r) => r.data);

export const updateDivision = (id: number, data: { name?: string; sort_order?: number; color?: string }): Promise<ApiResponse<Division>> =>
  client.put(`/divisions/${id}`, data).then((r) => r.data);

export const deleteDivision = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/divisions/${id}`, { params: { reason } }).then((r) => r.data);

// --- Projects (v0.2.17 绠€鍖? ---
export const getProjects = (params?: { group_id?: number; active_only?: boolean; method_type?: string; status?: 'ongoing' | 'archived' | 'all' }): Promise<ApiResponse<Project[]>> =>
  client.get('/projects', { params }).then((r) => r.data);

export const createProject = (data: {
  name: string;
  notes?: string;
  full_name?: string;
  sort_order?: number;
  is_active?: boolean;
  high_item?: string | null;
  project_status?: 'ongoing' | 'archived';
  lab_ids?: number[];
  method_ids?: number[];
}): Promise<ApiResponse<Project>> =>
  client.post('/projects', data).then((r) => r.data);

export const updateProject = (
  id: number,
  data: { name?: string; full_name?: string; notes?: string; sort_order?: number; is_active?: boolean; lab_ids?: number[]; method_ids?: number[]; high_item?: string | null; project_status?: 'ongoing' | 'archived' }
): Promise<ApiResponse<Project>> =>
  client.put(`/projects/${id}`, data).then((r) => r.data);

export const deleteProject = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/projects/${id}`, { params: { reason } }).then((r) => r.data);

export const batchProjectCoefficient = (data: { group_id: number; coefficient: number }): Promise<ApiResponse<number>> =>
  client.put('/projects/batch-coefficient', data).then((r) => r.data);

// --- Methods (v0.2.17 新增) ---
export const getMethods = (params?: { type_id?: number }): Promise<ApiResponse<Method[]>> =>
  client.get('/methods', { params }).then((r) => r.data);

export const createMethod = (data: { name: string; instrument_id: number; full_name?: string; coefficient?: number; multiplier?: number; amount?: number; notes?: string; type_ids?: number[] }): Promise<ApiResponse<Method>> =>
  client.post('/methods', data).then((r) => r.data);

export const updateMethod = (id: number, data: { method_code?: string; name?: string; instrument_id?: number | null; full_name?: string; coefficient?: number; multiplier?: number; amount?: number; notes?: string; is_active?: boolean; type_ids?: number[] }): Promise<ApiResponse<Method>> =>
  client.put(`/methods/${id}`, data).then((r) => r.data);

export const deleteMethod = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/methods/${id}`, { params: { reason } }).then((r) => r.data);

export const getInstruments = (): Promise<ApiResponse<Instrument[]>> =>
  client.get('/instruments').then((r) => r.data);

export const createInstrument = (data: { code: string; name?: string; instrument_type: string; is_active?: boolean; notes?: string }): Promise<ApiResponse<Instrument>> =>
  client.post('/instruments', data).then((r) => r.data);

export const updateInstrument = (id: number, data: { code?: string; name?: string; instrument_type?: string; is_active?: boolean; notes?: string }): Promise<ApiResponse<Instrument>> =>
  client.put(`/instruments/${id}`, data).then((r) => r.data);

export const deleteInstrument = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/instruments/${id}`, { params: { reason } }).then((r) => r.data);

export const methodImport = (file: File): Promise<ApiResponse<ImportSummary>> => {
  const fd = new FormData();
  fd.append('file', file);
  return client.post('/methods/import', fd, { headers: { 'Content-Type': 'multipart/form-data' } }).then((r) => r.data);
};

// v0.2.8: 方法类型（路由移动到 /api/method-types）
export const getMethodTypes = (): Promise<ApiResponse<MethodType[]>> =>
  client.get('/method-types').then((r) => r.data);

export const createMethodType = (data: { name: string; sort_order?: number }): Promise<ApiResponse<MethodType>> =>
  client.post('/method-types', data).then((r) => r.data);

export const updateMethodType = (id: number, data: { name?: string; sort_order?: number }): Promise<ApiResponse<MethodType>> =>
  client.put(`/method-types/${id}`, data).then((r) => r.data);

export const deleteMethodType = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/method-types/${id}`, { params: { reason } }).then((r) => r.data);

// v0.3.0: 瀵煎叆鏄犲皠閰嶇疆
export const getImportMappings = (): Promise<ApiResponse<ImportMapping[]>> =>
  client.get('/import/mappings').then(r => r.data);

// --- Master data import (v0.4.83) ---
export const downloadMasterImportTemplate = async (): Promise<void> => {
  const response = await client.get('/master-import/template', { responseType: 'blob' });
  const url = URL.createObjectURL(response.data);
  const anchor = document.createElement('a');
  anchor.href = url;
  const disposition = String(response.headers['content-disposition'] || '');
  anchor.download = getDownloadFilename(disposition, '主数据一键导入模板_v0.4.94-trace-preview.xlsx');
  anchor.click();
  URL.revokeObjectURL(url);
};

const masterImportForm = (file: File, mode: 'upsert' | 'skip') => {
  const form = new FormData();
  form.append('file', file);
  form.append('mode', mode);
  return form;
};

export const precheckMasterImport = (file: File, mode: 'upsert' | 'skip'): Promise<ApiResponse<MasterImportPreview>> =>
  client.post('/master-import/precheck', masterImportForm(file, mode), {
    headers: { 'Content-Type': 'multipart/form-data' },
  }).then(r => r.data);

export const executeMasterImport = (file: File, mode: 'upsert' | 'skip'): Promise<ApiResponse<MasterImportResult>> =>
  client.post('/master-import/execute', masterImportForm(file, mode), {
    headers: { 'Content-Type': 'multipart/form-data' },
  }).then(r => r.data);

// --- Records ---
export const getRecords = (params: { start?: string; end?: string; group_id?: number; page?: number; page_size?: number; include_deleted?: boolean; user_name?: string }): Promise<ApiResponse<PaginatedResponse<WorkRecord>>> =>
  client.get('/records', { params }).then((r) => r.data);

export const createRecord = (data: { project_id: number; method_id?: number; user_name: string; quantity: number; recorded_at: string; group_id?: number; division_id?: number | null; multiplier?: number }): Promise<ApiResponse<WorkRecord>> =>
  client.post('/records', data).then((r) => r.data);

export const deleteRecord = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/records/${id}`, { params: { reason } }).then((r) => r.data);

export const restoreRecord = (id: number): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/records/restore/${id}`).then((r) => r.data);

export const updateRecord = (id: number, data: { user_name?: string; quantity?: number; recorded_at?: string; multiplier?: number; high_item?: string | null }): Promise<ApiResponse<WorkRecord>> =>
  client.put(`/records/${id}`, data).then((r) => r.data);

export const getRecordUsers = (params: { start: string; end: string }): Promise<ApiResponse<string[]>> =>
  client.get('/records/users', { params }).then((r) => r.data);

export const deleteRecordsByUser = (user_name: string, params: { start: string; end: string; group_id?: number; reason?: string }): Promise<ApiResponse<{ deleted_count: number }>> =>
  client.delete(`/records/by-user/${encodeURIComponent(user_name)}`, { params }).then((r) => r.data);

// --- Stats ---
export const getStatsSummary = (params?: { start?: string; end?: string; group_id?: number; group_by?: string }): Promise<ApiResponse<StatsSummary>> =>
  client.get('/stats/summary', { params }).then((r) => r.data);

export const getStatsByUser = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<UserStats[]>> =>
  client.get('/stats/by-user', { params }).then((r) => r.data);

export const getStatsByProject = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<ProjectStats[]>> =>
  client.get('/stats/by-project', { params }).then((r) => r.data);

export const getStatsByType = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<TypeStats[]>> =>
  client.get('/stats/by-type', { params }).then((r) => r.data);

export const getStatsByInstrument = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<InstrumentStats[]>> =>
  client.get('/stats/by-instrument', { params }).then((r) => r.data);

// v0.4.28: 浜嬩笟閮ㄧ粺璁?
export const getStatsByDivision = (params?: { start?: string; end?: string; division_id?: number }): Promise<ApiResponse<DivisionStats[]>> =>
  client.get('/stats/by-division', { params }).then((r) => r.data);

// --- Export ---
async function downloadFile(url: string, params: Record<string, any>, filename: string): Promise<void> {
  const qs = new URLSearchParams();
  Object.entries(params).forEach(([k, v]) => { if (v !== undefined && v !== null) qs.set(k, String(v)); });
  const token = localStorage.getItem('workload_token') || sessionStorage.getItem('workload_token') || '';
  const res = await fetch(`${url}?${qs.toString()}`, {
    credentials: 'include',
    headers: token ? { 'Authorization': `Bearer ${token}` } : {},
  });
  if (!res.ok) {
    const txt = await res.text().catch(() => '');
    let msg = `导出失败 (HTTP ${res.status})`;
    try { const j = JSON.parse(txt); if (j.message) msg = j.message; } catch {}
    throw new Error(msg);
  }
  const blob = await res.blob();
  if (blob.size === 0) throw new Error('导出文件为空');
  const u = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = u;
  a.download = getDownloadFilename(res.headers.get('content-disposition') || '', filename);
  document.body.appendChild(a); a.click();
  document.body.removeChild(a);
  setTimeout(() => URL.revokeObjectURL(u), 1000);
}

export const exportExcel = (params: { start?: string; end?: string; group_id?: number }): Promise<void> =>
  downloadFile('/api/export/excel', params, `分析检测统计_${params.start?.substring(0, 10) ?? ''}_${params.end?.substring(0, 10) ?? ''}.xlsx`);

// --- Audit ---
export const getAuditLogs = (params?: { page?: number; page_size?: number; module?: 'work' | 'rd' | 'sample_info' | 'shared'; action?: string; user_name?: string; business_no?: string }): Promise<ApiResponse<PaginatedResponse<AuditLog>>> =>
  client.get('/audit-logs', { params }).then((r) => r.data);

export const getRecordTrace = (module: 'work' | 'rd' | 'sample-info', id: number): Promise<ApiResponse<RecordEvent[]>> =>
  client.get(`/trace/${module}/${id}`).then((r) => r.data);

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
export const adminLogin = (data: { username: string; password: string }): Promise<ApiResponse<{ token: string }>> =>
  client.post('/auth/login', data).then((r) => r.data);

// --- Backup ---
export const getBackupStatus = (): Promise<ApiResponse<BackupStatus>> => client.get('/backup/status').then((r) => r.data);
export const getNotificationSummary = (): Promise<ApiResponse<NotificationSummary>> => client.get('/notifications/summary').then(r => r.data);
export const getNotificationChannels = (): Promise<ApiResponse<NotificationChannel[]>> => client.get('/notifications/channels').then(r => r.data);
export const createNotificationChannel = (data: { name: string; webhook_url: string; secret?: string; is_active?: boolean }): Promise<ApiResponse<number>> => client.post('/notifications/channels', data).then(r => r.data);
export const updateNotificationChannel = (id: number, data: { name: string; webhook_url: string; secret?: string; is_active?: boolean }): Promise<ApiResponse<null>> => client.put(`/notifications/channels/${id}`, data).then(r => r.data);
export const testNotificationChannel = (id: number): Promise<ApiResponse<string>> => client.post(`/notifications/channels/${id}/test`).then(r => r.data);
export const getNotificationRules = (): Promise<ApiResponse<NotificationRule[]>> => client.get('/notifications/rules').then(r => r.data);
export const createNotificationRule = (data: { name: string; group_id?: number | null; project_name?: string; detection_type?: string; channel_id?: number | null; recipient_user_ids: number[]; is_default?: boolean; is_active?: boolean }): Promise<ApiResponse<number>> => client.post('/notifications/rules', data).then(r => r.data);
export const updateNotificationRule = (id: number, data: { name: string; group_id?: number | null; project_name?: string; detection_type?: string; channel_id?: number | null; recipient_user_ids: number[]; is_default?: boolean; is_active?: boolean }): Promise<ApiResponse<null>> => client.put(`/notifications/rules/${id}`, data).then(r => r.data);
export const getNotificationDeliveries = (): Promise<ApiResponse<NotificationDelivery[]>> => client.get('/notifications/deliveries').then(r => r.data);
export const processNotifications = (): Promise<ApiResponse<string>> => client.post('/notifications/process').then(r => r.data);
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

// ========== v0.3.7: 瀵煎嚭棰勮 API ==========

// Sheet 1 闇€瑕?group_id锛堝彲閫夛級
export const getPreviewSheet1 = (params: { start: string; end: string; group_id?: number }): Promise<ApiResponse<Sheet1Data>> =>
  client.get('/export/preview/sheet1', { params }).then((r) => r.data);

export const getPreviewSheet2 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet2Row[]>> =>
  client.get('/export/preview/sheet2', { params }).then((r) => r.data);

export const getPreviewSheet3 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet3Row[]>> =>
  client.get('/export/preview/sheet3', { params }).then((r) => r.data);

export const getPreviewSheet4 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet4Row[]>> =>
  client.get('/export/preview/sheet4', { params }).then((r) => r.data);

export const getPreviewSheet5 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet5Row[]>> =>
  client.get('/export/preview/sheet5', { params }).then((r) => r.data);

export const getPreviewSheet6 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet6Row[]>> =>
  client.get('/export/preview/sheet6', { params }).then((r) => r.data);

export const getPreviewSheet7 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet7Row[]>> =>
  client.get('/export/preview/sheet7', { params }).then((r) => r.data);

export const getPreviewSheet8 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet8Row[]>> =>
  client.get('/export/preview/sheet8', { params }).then((r) => r.data);

export const getPreviewSheet9 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet9Row[]>> =>
  client.get('/export/preview/sheet9', { params }).then((r) => r.data);

export const getPreviewSheet10 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet10Row[]>> =>
  client.get('/export/preview/sheet10', { params }).then((r) => r.data);

// v0.4.28: 浜嬩笟閮ㄩ瑙?
export const getPreviewSheet11 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet11Row[]>> =>
  client.get('/export/preview/sheet11', { params }).then((r) => r.data);

// ========== 鐮斿彂閫佹牱 (rd) 鈥?涓庡垎鏋愭娴嬪畬鍏ㄧ嫭绔嬪瓨鍌紝鍏辩敤涓绘暟鎹?==========

// --- RD Records ---
export const getRdRecords = (params: { start?: string; end?: string; group_id?: number; page?: number; page_size?: number; include_deleted?: boolean; user_name?: string }): Promise<ApiResponse<PaginatedResponse<WorkRecord>>> =>
  client.get('/rd-records', { params }).then((r) => r.data);

export const createRdRecord = (data: { project_id: number; method_id?: number; user_name: string; quantity: number; recorded_at: string; group_id?: number; division_id?: number | null }): Promise<ApiResponse<WorkRecord>> =>
  client.post('/rd-records', data).then((r) => r.data);

export const deleteRdRecord = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/rd-records/${id}`, { params: { reason } }).then((r) => r.data);

export const restoreRdRecord = (id: number): Promise<ApiResponse<WorkRecord>> =>
  client.post(`/rd-records/restore/${id}`).then((r) => r.data);

export const updateRdRecord = (id: number, data: { user_name?: string; quantity?: number; recorded_at?: string; multiplier?: number; project_id?: number; method_id?: number | null; group_id?: number | null; division_id?: number | null; batch_no?: string; notes?: string; high_item?: string | null }): Promise<ApiResponse<WorkRecord>> =>
  client.put(`/rd-records/${id}`, data).then((r) => r.data);

export const deleteRdRecordsByUser = (user_name: string, params: { start: string; end: string; group_id?: number; reason?: string }): Promise<ApiResponse<{ deleted_count: number }>> =>
  client.delete(`/rd-records/by-user/${encodeURIComponent(user_name)}`, { params }).then((r) => r.data);

export const sampleRdRecord = (id: number): Promise<ApiResponse<WorkRecord>> =>
  client.put(`/rd-records/${id}/sample`, {}).then(r => r.data);

export const getRdRecordUsers = (params: { start: string; end: string }): Promise<ApiResponse<string[]>> =>
  client.get('/rd-records/users', { params }).then((r) => r.data);

// --- RD Stats ---
export const getRdStatsSummary = (params?: { start?: string; end?: string; group_id?: number; group_by?: string }): Promise<ApiResponse<StatsSummary>> =>
  client.get('/rd-stats/summary', { params }).then((r) => r.data);

export const getRdStatsByUser = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<UserStats[]>> =>
  client.get('/rd-stats/by-user', { params }).then((r) => r.data);

export const getRdStatsByProject = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<ProjectStats[]>> =>
  client.get('/rd-stats/by-project', { params }).then((r) => r.data);

export const getRdStatsByType = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<TypeStats[]>> =>
  client.get('/rd-stats/by-type', { params }).then((r) => r.data);

export const getRdStatsByInstrument = (params?: { start?: string; end?: string; group_id?: number }): Promise<ApiResponse<InstrumentStats[]>> =>
  client.get('/rd-stats/by-instrument', { params }).then((r) => r.data);

export const getRdStatsByDivision = (params?: { start?: string; end?: string; division_id?: number }): Promise<ApiResponse<DivisionStats[]>> =>
  client.get('/rd-stats/by-division', { params }).then((r) => r.data);

// --- RD Export ---
export const exportRdExcel = (params: { start?: string; end?: string; group_id?: number }): Promise<void> =>
  downloadFile('/api/rd-export/excel', params, `研发送样统计_${params.start?.substring(0, 10) ?? ''}_${params.end?.substring(0, 10) ?? ''}.xlsx`);

// --- RD Export Preview ---
export const getRdPreviewSheet1 = (params: { start: string; end: string; group_id?: number }): Promise<ApiResponse<Sheet1Data>> =>
  client.get('/rd-export/preview/sheet1', { params }).then((r) => r.data);

export const getRdPreviewSheet2 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet2Row[]>> =>
  client.get('/rd-export/preview/sheet2', { params }).then((r) => r.data);

export const getRdPreviewSheet3 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet3Row[]>> =>
  client.get('/rd-export/preview/sheet3', { params }).then((r) => r.data);

export const getRdPreviewSheet4 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet4Row[]>> =>
  client.get('/rd-export/preview/sheet4', { params }).then((r) => r.data);

export const getRdPreviewSheet5 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet5Row[]>> =>
  client.get('/rd-export/preview/sheet5', { params }).then((r) => r.data);

export const getRdPreviewSheet6 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet6Row[]>> =>
  client.get('/rd-export/preview/sheet6', { params }).then((r) => r.data);

export const getRdPreviewSheet7 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet7Row[]>> =>
  client.get('/rd-export/preview/sheet7', { params }).then((r) => r.data);

export const getRdPreviewSheet8 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet8Row[]>> =>
  client.get('/rd-export/preview/sheet8', { params }).then((r) => r.data);

export const getRdPreviewSheet9 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet9Row[]>> =>
  client.get('/rd-export/preview/sheet9', { params }).then((r) => r.data);

export const getRdPreviewSheet10 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet10Row[]>> =>
  client.get('/rd-export/preview/sheet10', { params }).then((r) => r.data);

export const getRdPreviewSheet11 = (params: { start: string; end: string }): Promise<ApiResponse<Sheet11Row[]>> =>
  client.get('/rd-export/preview/sheet11', { params }).then((r) => r.data);

// ========== v0.4.11: 甯姪鏂囨。 API ==========

export const getHelpDocuments = (visibleOnly?: boolean): Promise<ApiResponse<HelpDocument[]>> =>
  client.get('/help-documents', { params: { visible_only: visibleOnly ?? true } }).then((r) => r.data);

export const uploadHelpDocument = (formData: FormData): Promise<ApiResponse<HelpDocument>> =>
  client.post('/help-documents', formData, { headers: { 'Content-Type': 'multipart/form-data' } }).then((r) => r.data);

export const updateHelpDocument = (id: number, data: { title?: string; is_visible?: boolean; sort_order?: number }): Promise<ApiResponse<HelpDocument>> =>
  client.put(`/help-documents/${id}`, data).then((r) => r.data);

export const deleteHelpDocument = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/help-documents/${id}`, { params: { reason } }).then((r) => r.data);

export const getHelpDocumentFileUrl = (id: number): string =>
  `/api/help-documents/${id}/file`;

export const getHelpDocumentPageUrl = (id: number, page: number): string =>
  `/api/help-documents/${id}/pages/${page}`;

export const getHelpDocumentFileBlob = (id: number): Promise<Blob> =>
  client.get(`/help-documents/${id}/file`, { responseType: 'blob' }).then(r => r.data);

export const getHelpDocumentPageBlob = (id: number, page: number): Promise<Blob> =>
  client.get(`/help-documents/${id}/pages/${page}`, { responseType: 'blob' }).then(r => r.data);

export const getHelpArticleImageBlob = (url: string): Promise<Blob> =>
  client.get(url.replace(/^\/api/, ''), { responseType: 'blob' }).then(r => r.data);

// v0.4.19: 缁撴瀯鍖栨枃绔?
export const getHelpArticles = (visibleOnly?: boolean): Promise<ApiResponse<HelpArticle[]>> =>
  client.get('/help-articles', { params: { visible_only: visibleOnly ?? true } }).then(r => r.data);

export const getHelpArticle = (id: number): Promise<ApiResponse<HelpArticle>> =>
  client.get(`/help-articles/${id}`).then(r => r.data);

export const deleteHelpArticle = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/help-articles/${id}`, { params: { reason } }).then(r => r.data);

export const updateHelpArticle = (id: number, data: { title?: string; content_html?: string; is_visible?: boolean; sort_order?: number }): Promise<ApiResponse<HelpArticle>> =>
  client.put(`/help-articles/${id}`, data).then(r => r.data);

export const reorderHelpDocuments = (ids: { id: number; sort_order: number }[]): Promise<ApiResponse<null>> =>
  client.put('/help-documents/sort', { ids }).then(r => r.data);

export const reorderHelpArticles = (ids: { id: number; sort_order: number }[]): Promise<ApiResponse<null>> =>
  client.put('/help-articles/sort', { ids }).then(r => r.data);

export interface PersonnelFeedback {
  id: number; notice_no: string; lab_id: number; lab_name: string;
  responsible_user_id?: number | null; responsible_name: string;
  change_type: string; person_name: string; effective_at: string; notes: string; status: string;
  project_ids: number[]; project_names: string[]; created_by_user_id: number;
  created_by_username: string; created_at: string; updated_at: string;
}

export const getPersonnelFeedback = (): Promise<ApiResponse<PersonnelFeedback[]>> =>
  client.get('/personnel-feedback').then(r => r.data);
export const createPersonnelFeedback = (data: { lab_id: number; responsible_user_id?: number | null; change_type: string; person_name: string; effective_at: string; notes?: string; project_ids: number[] }): Promise<ApiResponse<PersonnelFeedback>> =>
  client.post('/personnel-feedback', data).then(r => r.data);
export const updatePersonnelFeedback = (id: number, data: Partial<{ responsible_user_id: number | null; change_type: string; person_name: string; effective_at: string; notes: string; project_ids: number[] }>): Promise<ApiResponse<PersonnelFeedback>> =>
  client.put(`/personnel-feedback/${id}`, data).then(r => r.data);
export const withdrawPersonnelFeedback = (id: number): Promise<ApiResponse<PersonnelFeedback>> =>
  client.post(`/personnel-feedback/${id}/withdraw`).then(r => r.data);

// ========== v0.4.22: 鏍峰搧淇℃伅鐧昏 API ==========

export const getSampleInfoRecords = (params?: { detection_type?: string; type_key?: string; status?: string; user_name?: string; lab_name?: string; project_name?: string; start?: string; end?: string; page?: number; page_size?: number; include_deleted?: boolean }): Promise<ApiResponse<PaginatedResponse<SampleInfoRecord>>> =>
  client.get('/sample-info', { params }).then(r => r.data);

export const createSampleInfo = (data: { batch_no: string; user_name: string; lab_name: string; project_name: string; submitted_at?: string; detection_date?: string; main_components: string; detection_type: string; type_key: string; division_id?: number | null; quantity?: number; notes?: string; extra_fields?: Record<string, any> }): Promise<ApiResponse<SampleInfoRecord>> =>
  client.post('/sample-info', data).then(r => r.data);

export const createSampleInfoWithAttachments = (data: { batch_no: string; user_name: string; lab_name: string; project_name: string; submitted_at?: string; detection_date?: string; main_components: string; detection_type: string; type_key: string; division_id?: number | null; quantity?: number; notes?: string; extra_fields?: Record<string, any> }, files: File[]): Promise<ApiResponse<SampleInfoRecord>> => {
  const form = new FormData();
  form.append('payload', JSON.stringify(data));
  files.forEach(file => form.append('files', file));
  return client.post('/sample-info/with-attachments', form).then(r => r.data);
};

export const updateSampleInfo = (id: number, data: { status?: string; batch_no?: string; user_name?: string; lab_name?: string; project_name?: string; submitted_at?: string; detection_date?: string; main_components?: string; type_key?: string; division_id?: number | null; quantity?: number; notes?: string; extra_fields?: Record<string, any> }): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}`, data).then(r => r.data);

export const deleteSampleInfo = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/${id}`, { params: { reason } }).then(r => r.data);

export const restoreSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.post(`/sample-info/${id}/restore`).then(r => r.data);

export const updateSampleInfoStatus = (id: number, status: string): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/status`, { status }).then(r => r.data);

export const sampleSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/sample`, {}).then(r => r.data);

export const completeSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/complete`, {}).then(r => r.data);

// 鐙珛缁熻锛堜笉鎺ュ垎鏋愭娴?/stats锛?
export const getSampleInfoStats = (params?: { start?: string; end?: string; type_key?: string; status?: string }): Promise<ApiResponse<any>> =>
  client.get('/sample-info/stats', { params }).then(r => r.data);

// ========== v0.4.23: 妫€娴嬬被鍨?CRUD ==========
export const getSampleInfoTypes = (): Promise<ApiResponse<SampleInfoType[]>> =>
  client.get('/sample-info-types').then(r => r.data);

export const getSampleInfoTypesAll = (): Promise<ApiResponse<SampleInfoType[]>> =>
  client.get('/sample-info-types/all').then(r => r.data);

export const createSampleInfoType = (data: { type_key: string; label: string; description?: string; color?: string; sort_order?: number }): Promise<ApiResponse<SampleInfoType>> =>
  client.post('/sample-info-types', data).then(r => r.data);

export const updateSampleInfoType = (id: number, data: { type_key?: string; label?: string; description?: string; color?: string; sort_order?: number; is_active?: number }): Promise<ApiResponse<SampleInfoType>> =>
  client.put(`/sample-info-types/${id}`, data).then(r => r.data);

export const deleteSampleInfoType = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info-types/${id}`, { params: { reason } }).then(r => r.data);

// ========== v0.4.23: 鏍峰搧淇℃伅鐧昏瀵煎嚭锛堢嫭绔嬫帴鍙ｏ級 ==========
export const exportSampleInfo = (params: { start?: string; end?: string }): Promise<void> =>
  downloadFile('/api/sample-info/export', params, `样品信息登记_${params.start?.substring(0, 10) ?? ''}_${params.end?.substring(0, 10) ?? ''}.xlsx`);

// ========== v0.4.26: 鍒楄嚜瀹氫箟 API ==========
export const getSampleInfoColumns = (typeKey?: string): Promise<ApiResponse<SampleInfoColumn[]>> =>
  client.get('/sample-info/columns', { params: { type_key: typeKey || undefined } }).then(r => r.data);

export const getActiveSampleInfoColumns = (typeKey?: string): Promise<ApiResponse<SampleInfoColumn[]>> =>
  client.get('/sample-info/columns/active', { params: { type_key: typeKey || undefined } }).then(r => r.data);

// v0.4.27-A: 绠＄悊椤典笓鐢?鈥?鍒?+ 鍙鎬т俊鎭?
export const getSampleInfoColumnsManage = (typeKey: string): Promise<ApiResponse<Array<SampleInfoColumn & { is_visible_in_type: boolean }>>> =>
  client.get('/sample-info/columns/manage', { params: { type_key: typeKey } }).then(r => r.data);

// v0.4.27-A: 鎵归噺鏇存柊棰勭疆鍒楀彲瑙佹€?
export const updateSampleInfoColumnVisibility = (data: { type_key: string; items: ColumnVisibilityItem[] }): Promise<ApiResponse<null>> =>
  client.put('/sample-info/columns/visibility', data).then(r => r.data);

export const updateSampleInfoColumnTypes = (id: number, typeKeys: string[]): Promise<ApiResponse<SampleInfoColumn>> =>
  client.put('/sample-info/columns/' + id + '/types', { type_keys: typeKeys }).then(r => r.data);

export const createSampleInfoColumn = (data: {
  field_key: string;
  label: string;
  data_type: string;
  width?: number;
  sort_order?: number;
  options?: string;
  is_required?: boolean;
  show_in_list?: boolean;
  show_in_export?: boolean;
  show_in_form?: boolean;
}): Promise<ApiResponse<SampleInfoColumn>> =>
  client.post('/sample-info/columns', data).then(r => r.data);

export const updateSampleInfoColumn = (id: number, data: {
  label?: string;
  data_type?: string;
  is_active?: boolean;
  is_required?: boolean;
  width?: number;
  options?: string;
  show_in_list?: boolean;
  show_in_export?: boolean;
  show_in_form?: boolean;
}): Promise<ApiResponse<SampleInfoColumn>> =>
  client.put(`/sample-info/columns/${id}`, data).then(r => r.data);

export const deleteSampleInfoColumn = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/columns/${id}`, { params: { reason } }).then(r => r.data);

export const reorderSampleInfoColumns = (ids: { id: number; sort_order: number }[]): Promise<ApiResponse<SampleInfoColumn[]>> =>
  client.put('/sample-info/columns/sort', { ids }).then(r => r.data);

// ========== v0.4.27-A: 闄勪欢 API ==========
export const getSampleInfoAttachments = (recordId: number): Promise<ApiResponse<SampleInfoAttachment[]>> =>
  client.get(`/sample-info/${recordId}/attachments`).then(r => r.data);

export const uploadSampleInfoAttachment = (recordId: number, file: File): Promise<ApiResponse<SampleInfoAttachment>> => {
  const fd = new FormData();
  fd.append('file', file);
  return client.post(`/sample-info/${recordId}/attachments`, fd).then(r => r.data);
};

export const getSampleInfoAttachmentUrl = (attId: number): string =>
  `/api/sample-info/attachments/${attId}/file`;

export const deleteSampleInfoAttachment = (attId: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/attachments/${attId}`, { params: { reason } }).then(r => r.data);

// v0.4.28: 鎵归噺鑾峰彇闄勪欢
export const batchGetSampleInfoAttachments = (recordIds: number[]): Promise<ApiResponse<Record<number, SampleInfoAttachment[]>>> => {
  if (recordIds.length === 0) return Promise.resolve({ code: 0, message: 'ok', data: {} } as any);
  return client.get('/sample-info/attachments/batch', { params: { record_ids: recordIds.join(',') } }).then(r => r.data);
};

// ========== v0.4.27-A: 鐢ㄦ埛 API ==========
export const userRegister = (data: { username: string; password: string; division_id?: number | null; group_id?: number | null; role_ids?: number[] }): Promise<ApiResponse<User>> =>
  client.post('/users/register', data).then(r => r.data);

export const createUser = (data: { username: string; password: string; division_id?: number | null; group_id?: number | null; role_id?: number | null; role_ids?: number[] }): Promise<ApiResponse<User>> =>
  client.post('/users', data).then(r => r.data);

export const userLogin = (data: LoginRequest): Promise<ApiResponse<LoginResponse>> =>
  client.post('/users/login', data).then(r => r.data);

export const userMe = (): Promise<ApiResponse<User>> =>
  client.get('/users/me').then(r => r.data);

export const userList = (): Promise<ApiResponse<User[]>> =>
  client.get('/users').then(r => r.data);

export const updateUser = (id: number, data: UserUpdate): Promise<ApiResponse<User>> =>
  client.put(`/users/${id}`, data).then(r => r.data);

export const deleteUser = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/users/${id}`, { params: { reason } }).then(r => r.data);

// ========== v0.4.32: 瑙掕壊锛堢敤鎴峰垎绾э級API ==========
export const getRoles = (): Promise<ApiResponse<RoleWithPermissions[]>> =>
  client.get('/roles').then((r) => r.data);

export const createRole = (data: { name: string; description?: string; sort_order?: number; permissions?: string[] }): Promise<ApiResponse<RoleWithPermissions>> =>
  client.post('/roles', data).then((r) => r.data);

export const updateRole = (id: number, data: { name?: string; description?: string; sort_order?: number }): Promise<ApiResponse<RoleWithPermissions>> =>
  client.put(`/roles/${id}`, data).then((r) => r.data);

export const deleteRole = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/roles/${id}`, { params: { reason } }).then((r) => r.data);

export const setRolePermissions = (id: number, permissions: string[]): Promise<ApiResponse<RoleWithPermissions>> =>
  client.put(`/roles/${id}/permissions`, { permissions }).then((r) => r.data);

export const getPermissionWhitelist = (): Promise<ApiResponse<PermissionDef[]>> =>
  client.get('/roles/permissions').then((r) => r.data);

export const userLogout = (): Promise<ApiResponse<null>> =>
  client.post('/users/logout').then(r => r.data);

export const getUserSessions = (): Promise<ApiResponse<UserSession[]>> =>
  client.get('/sessions').then(r => r.data);

export const cleanupExpiredSessions = (): Promise<ApiResponse<number>> =>
  client.delete('/sessions/expired').then(r => r.data);

// v0.4.28: 淇敼瀵嗙爜
export const changePassword = (data: { old_password: string; new_password: string }): Promise<ApiResponse<null>> =>
  client.put('/users/change-password', data).then(r => r.data);

// ========== v0.4.27-A: 閮ㄩ棬鍏宠仈瀹為獙瀹?==========
export const setDivisionLabs = (divisionId: number, groupIds: number[]): Promise<ApiResponse<null>> =>
  client.put(`/divisions/${divisionId}/labs`, { group_ids: groupIds }).then(r => r.data);

// ========== v0.4.33: 鐮斿彂閫佹牱鍒楅厤缃?API ==========
export const getRdRecordColumns = (): Promise<ApiResponse<RdRecordColumn[]>> =>
  client.get('/rd-record-columns').then(r => r.data);

export const updateRdRecordColumn = (id: number, data: { width?: number; show_in_list?: boolean; show_in_form?: boolean }): Promise<ApiResponse<RdRecordColumn>> =>
  client.put(`/rd-record-columns/${id}`, data).then(r => r.data);

// ========== v0.4.35: 鍏?UI 鑷畾涔夌郴缁?API ==========
export const getSettings = (): Promise<ApiResponse<SystemSetting[]>> =>
  client.get('/settings').then(r => r.data);

export const getSetting = (key: string): Promise<ApiResponse<SystemSetting>> =>
  client.get(`/settings/${key}`).then(r => r.data);

export const updateSetting = (key: string, value: any): Promise<ApiResponse<SystemSetting>> =>
  client.put(`/settings/${key}`, { value }).then(r => r.data);

// ========== v0.4.63: 瀵煎叆鐢ㄦ埛 API ==========
export const downloadUserImportTemplate = (): Promise<void> =>
  downloadFile('/api/users/import/template', {}, '用户批量导入模板_v0.4.106.xlsx');

export const importUsers = (file: File, updateExisting = false): Promise<ApiResponse<any>> => {
  const fd = new FormData();
  fd.append('file', file);
  fd.append('update_existing', String(updateExisting));
  return client.post('/users/import', fd).then(r => r.data);
};

// ========== v0.4.103: 业务数据治理 ==========
export const getGovernanceSummary = (): Promise<ApiResponse<GovernanceSummary>> =>
  client.get('/data-governance/summary').then((r) => r.data);

export const downloadGovernanceTemplate = (module: GovernanceModule): Promise<void> =>
  downloadFile(`/api/data-governance/${module}/template`, {}, `${module}_数据治理导入模板_v0.4.105.xlsx`);

const governanceImport = (
  module: GovernanceModule,
  file: File,
  endpoint: 'import/precheck' | 'import',
): Promise<ApiResponse<GovernanceImportPreview>> => {
  const form = new FormData();
  form.append('file', file);
  return client.post(`/data-governance/${module}/${endpoint}`, form).then((r) => r.data);
};

export const precheckGovernanceImport = (module: GovernanceModule, file: File): Promise<ApiResponse<GovernanceImportPreview>> =>
  governanceImport(module, file, 'import/precheck');

export const executeGovernanceImport = (module: GovernanceModule, file: File): Promise<ApiResponse<GovernanceImportPreview>> =>
  governanceImport(module, file, 'import');

export const exportGovernanceData = (module: GovernanceModule, start: string, end: string): Promise<void> =>
  downloadFile(`/api/data-governance/${module}/export`, { start, end }, `${module}_原始记录_${start}_${end}.zip`);

export const precheckGovernancePurge = (module: GovernanceModule, start: string, end: string): Promise<ApiResponse<GovernancePurgePreview>> =>
  client.post(`/data-governance/${module}/purge/precheck`, { start, end }).then((r) => r.data);

export const executeGovernancePurge = (
  module: GovernanceModule,
  data: { start: string; end: string; confirmation_token: string; admin_username: string; admin_password: string },
): Promise<ApiResponse<GovernancePurgeResult>> =>
  client.post(`/data-governance/${module}/purge`, data).then((r) => r.data);

// ========== v0.4.105: 统一回收站 ==========
export const getTrashEntries = (params: {
  category?: string;
  module?: string;
  keyword?: string;
  page?: number;
  page_size?: number;
} = {}): Promise<ApiResponse<PaginatedResponse<TrashEntry>>> =>
  client.get('/trash', { params }).then((r) => r.data);

export const getTrashPrecheck = (
  tableName: string,
  recordId: number,
): Promise<ApiResponse<TrashPrecheck>> =>
  client.get(`/trash/precheck/${encodeURIComponent(tableName)}/${recordId}`).then((r) => r.data);

export const restoreTrashEntry = (id: number): Promise<ApiResponse<TrashEntry>> =>
  client.post(`/trash/${id}/restore`).then((r) => r.data);

export const purgeTrashEntry = (
  id: number,
  data: { admin_username: string; admin_password: string },
): Promise<ApiResponse<TrashEntry>> =>
  client.post(`/trash/${id}/purge`, data).then((r) => r.data);

// ========== v1.0.1: 日志留存与数据库维护 ==========
export const getLogMaintenanceStatus = (): Promise<ApiResponse<LogMaintenanceStatus>> =>
  client.get('/log-maintenance/status').then((r) => r.data);

export const getLogMaintenancePolicy = (): Promise<ApiResponse<LogMaintenancePolicyView>> =>
  client.get('/log-maintenance/policy').then((r) => r.data);

export const updateLogMaintenancePolicy = (data: LogMaintenancePolicyView): Promise<ApiResponse<LogMaintenancePolicyView>> =>
  client.put('/log-maintenance/policy', data).then((r) => r.data);

export const runLogMaintenance = (job: 'session-cleanup' | 'runtime-log-archive' | 'audit-archive' | 'database-maintenance'): Promise<ApiResponse<MaintenanceJobResult>> =>
  client.post(`/log-maintenance/run/${job}`).then((r) => r.data);

export const verifyLogArchive = (id: number): Promise<ApiResponse<LogArchiveBatch>> =>
  client.post(`/log-maintenance/archives/${id}/verify`).then((r) => r.data);

export default client;

