import { client, downloadFile } from './http';
import type {
  ApiResponse,
  Division,
  ImportMapping,
  ImportSummary,
  Instrument,
  MasterDataPreview,
  MasterImportPreview,
  MasterImportResult,
  Method,
  MethodType,
  MethodTypeVisibility,
  Project,
  ProjectGroup,
} from '../types';

// --- Groups ---
export type MasterDataPortal = 'work' | 'rd' | 'sample_info';

export const getGroups = (params?: { portal?: MasterDataPortal }): Promise<ApiResponse<ProjectGroup[]>> =>
  client.get('/groups', { params }).then((r) => r.data);

export const createGroup = (data: { name: string; sort_order?: number; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; division_id?: number | null }): Promise<ApiResponse<ProjectGroup>> =>
  client.post('/groups', data).then((r) => r.data);

export const updateGroup = (id: number, data: { name?: string; sort_order?: number; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; division_id?: number | null }): Promise<ApiResponse<ProjectGroup>> =>
  client.put(`/groups/${id}`, data).then((r) => r.data);

export const deleteGroup = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/groups/${id}`, { params: { reason } }).then((r) => r.data);

// ========== v0.4.24: 浜嬩笟閮?CRUD ==========
export const getDivisions = (params?: { portal?: MasterDataPortal }): Promise<ApiResponse<Division[]>> =>
  client.get('/divisions', { params }).then((r) => r.data);

export const createDivision = (data: { name: string; code: string; manager_user_id?: number | null; sort_order?: number; color?: string; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; is_active?: boolean }): Promise<ApiResponse<Division>> =>
  client.post('/divisions', data).then((r) => r.data);

export const updateDivision = (id: number, data: { name?: string; code?: string; manager_user_id?: number | null; sort_order?: number; color?: string; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; is_active?: boolean }): Promise<ApiResponse<Division>> =>
  client.put(`/divisions/${id}`, data).then((r) => r.data);

export const deleteDivision = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/divisions/${id}`, { params: { reason } }).then((r) => r.data);

// --- Projects (v0.2.17 绠€鍖? ---
export const getProjects = (params?: { group_id?: number; active_only?: boolean; method_type?: string; status?: 'ongoing' | 'archived' | 'all'; portal?: MasterDataPortal }): Promise<ApiResponse<Project[]>> =>
  client.get('/projects', { params }).then((r) => r.data);

export const createProject = (data: {
  name: string;
  notes?: string;
  full_name?: string;
  sort_order?: number;
  is_active?: boolean;
  show_in_work?: boolean;
  show_in_rd?: boolean;
  show_in_sample_info?: boolean;
  high_item?: string | null;
  project_status?: 'ongoing' | 'archived';
  lab_ids?: number[];
  method_ids?: number[];
  project_division_id?: number | null;
  collaboration_division_ids?: number[];
}): Promise<ApiResponse<Project>> =>
  client.post('/projects', data).then((r) => r.data);

export const updateProject = (
  id: number,
  data: { name?: string; full_name?: string; notes?: string; sort_order?: number; is_active?: boolean; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; lab_ids?: number[]; method_ids?: number[]; high_item?: string | null; project_status?: 'ongoing' | 'archived'; project_division_id?: number | null; collaboration_division_ids?: number[] }
): Promise<ApiResponse<Project>> =>
  client.put(`/projects/${id}`, data).then((r) => r.data);

export const deleteProject = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/projects/${id}`, { params: { reason } }).then((r) => r.data);

export const batchProjectCoefficient = (data: { group_id: number; coefficient: number }): Promise<ApiResponse<number>> =>
  client.put('/projects/batch-coefficient', data).then((r) => r.data);

// --- Methods (v0.2.17 新增) ---
export const getMethods = (params?: { type_id?: number; portal?: MasterDataPortal; group_id?: number }): Promise<ApiResponse<Method[]>> =>
  client.get('/methods', { params }).then((r) => r.data);

export const createMethod = (data: { name: string; instrument_id: number; full_name?: string; coefficient?: number; multiplier?: number; amount?: number; notes?: string; type_ids?: number[]; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; is_common?: boolean; common_division_ids?: number[] }): Promise<ApiResponse<Method>> =>
  client.post('/methods', data).then((r) => r.data);

export const updateMethod = (id: number, data: { method_code?: string; name?: string; instrument_id?: number | null; full_name?: string; coefficient?: number; multiplier?: number; amount?: number; notes?: string; is_active?: boolean; type_ids?: number[]; show_in_work?: boolean; show_in_rd?: boolean; show_in_sample_info?: boolean; is_common?: boolean; common_division_ids?: number[] }): Promise<ApiResponse<Method>> =>
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
export const getMethodTypes = (params?: { portal?: MasterDataPortal; group_id?: number }): Promise<ApiResponse<MethodType[]>> =>
  client.get('/method-types', { params }).then((r) => r.data);

export const createMethodType = (data: { name: string; sort_order?: number }): Promise<ApiResponse<MethodType>> =>
  client.post('/method-types', data).then((r) => r.data);

export const updateMethodType = (id: number, data: { name?: string; sort_order?: number }): Promise<ApiResponse<MethodType>> =>
  client.put(`/method-types/${id}`, data).then((r) => r.data);

export const deleteMethodType = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/method-types/${id}`, { params: { reason } }).then((r) => r.data);

export const getMethodTypeVisibility = (id: number, portal: 'work' | 'rd' | 'sample_info'): Promise<ApiResponse<MethodTypeVisibility[]>> =>
  client.get(`/method-types/${id}/visibility`, { params: { portal } }).then((r) => r.data);

export const updateMethodTypeVisibility = (id: number, portal: 'work' | 'rd' | 'sample_info', visible_group_ids: number[]): Promise<ApiResponse<MethodTypeVisibility[]>> =>
  client.put(`/method-types/${id}/visibility`, { portal, visible_group_ids }).then((r) => r.data);

// v0.3.0: 瀵煎叆鏄犲皠閰嶇疆
export const getImportMappings = (): Promise<ApiResponse<ImportMapping[]>> =>
  client.get('/import/mappings').then(r => r.data);

// --- Master data import (v0.4.83) ---
export const downloadMasterImportTemplate = async (): Promise<void> => {
  await downloadFile('/api/master-import/template', {}, '主数据一键导入模板_v1.1.6-hotfix.8-beta.xlsx');
};

export const downloadMasterDataExport = (): Promise<void> =>
  downloadFile('/api/master-data/export', {}, `主数据导出_${new Date().toISOString().slice(0, 10)}.xlsx`);

export const getMasterDataPreview = (): Promise<ApiResponse<MasterDataPreview>> =>
  client.get('/master-data/preview').then((r) => r.data);

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

