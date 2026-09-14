import { client, downloadFile } from './http';
import type { PermissionDef } from '../constants/permissions';
import type {
  ApiResponse,
  PaginatedResponse,
  User,
  LoginRequest,
  LoginResponse,
  UserUpdate,
  UserSession,
  RoleTemplate,
  RoleWithPermissions,
  CurrentRoleDataScopeSummary,
  RdRecordColumn, RdRecordColumnInput,
  SystemSetting,
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
} from '../types';

export const createUser = (data: { username: string; password: string; division_id?: number | null; primary_division_id?: number | null; group_id?: number | null; division_ids?: number[]; business_division_ids?: number[]; group_ids?: number[]; role_id?: number | null; role_ids?: number[] }): Promise<ApiResponse<User>> =>
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

export const getRoleTemplates = (): Promise<ApiResponse<RoleTemplate[]>> =>
  client.get('/role-templates').then((r) => r.data);

export const createRole = (data: { name: string; description?: string; sort_order?: number; permissions?: string[]; template_id?: number | null }): Promise<ApiResponse<RoleWithPermissions>> =>
  client.post('/roles', data).then((r) => r.data);

export const updateRole = (id: number, data: { name?: string; description?: string; sort_order?: number }): Promise<ApiResponse<RoleWithPermissions>> =>
  client.put(`/roles/${id}`, data).then((r) => r.data);

export const deleteRole = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/roles/${id}`, { params: { reason } }).then((r) => r.data);

export const setRolePermissions = (id: number, permissions: string[]): Promise<ApiResponse<RoleWithPermissions>> =>
  client.put(`/roles/${id}/permissions`, { permissions }).then((r) => r.data);

export const setRoleDataScopes = (id: number, data: { division_ids: number[]; work_division_ids: number[]; sample_info_type_keys: string[] }): Promise<ApiResponse<RoleWithPermissions>> =>
  client.put(`/roles/${id}/data-scopes`, data).then((r) => r.data);

export const getCurrentRoleDataScopes = (): Promise<ApiResponse<CurrentRoleDataScopeSummary>> =>
  client.get('/roles/current-data-scopes').then((r) => r.data);

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

export const createRdRecordColumn = (data: RdRecordColumnInput): Promise<ApiResponse<RdRecordColumn>> =>
  client.post('/rd-record-columns', data).then(r => r.data);

export const updateRdRecordColumn = (id: number, data: Partial<RdRecordColumnInput>): Promise<ApiResponse<RdRecordColumn>> =>
  client.put(`/rd-record-columns/${id}`, data).then(r => r.data);

export const reorderRdRecordColumns = (ids: number[]): Promise<ApiResponse<RdRecordColumn[]>> =>
  client.put('/rd-record-columns/reorder', ids).then(r => r.data);

export const deleteRdRecordColumn = (id: number): Promise<ApiResponse<null>> =>
  client.delete(`/rd-record-columns/${id}`).then(r => r.data);

// ========== v0.4.35: 鍏?UI 鑷畾涔夌郴缁?API ==========
export const getSettings = (): Promise<ApiResponse<SystemSetting[]>> =>
  client.get('/settings').then(r => r.data);

export const getSetting = (key: string): Promise<ApiResponse<SystemSetting>> =>
  client.get(`/settings/${key}`).then(r => r.data);

export const updateSetting = (key: string, value: any): Promise<ApiResponse<SystemSetting>> =>
  client.put(`/settings/${key}`, { value }).then(r => r.data);

// ========== v0.4.63: 瀵煎叆鐢ㄦ埛 API ==========
export const downloadUserImportTemplate = async (): Promise<void> => {
  let version = 'current';
  try {
    const response = await client.get<{ version?: string }>('/version');
    if (response.data?.version) version = response.data.version;
  } catch {
    // The server response supplies the authoritative filename when available.
  }
  await downloadFile('/api/users/import/template', {}, `用户导入模板_v${version}.xlsx`);
};
export const downloadUsers = (): Promise<void> =>
  downloadFile('/api/users/export', {}, `用户列表_v${new Date().toISOString().slice(0, 10)}.xlsx`);

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
export const purgeTrashEntries = (ids: number[], data: { admin_username: string; admin_password: string }): Promise<ApiResponse<TrashEntry[]>> =>
  client.post('/trash/purge-batch', { ids, ...data }).then((r) => r.data);

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
