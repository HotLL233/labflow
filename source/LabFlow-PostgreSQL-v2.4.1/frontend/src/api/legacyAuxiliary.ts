import { client, requireApiSuccess } from './http';
import type { ApiResponse } from '../types';
import type { HistoryQuery } from './workloadGovernance';

export interface LegacyAuxiliaryRow {
  id: number; business_no: string; user_name: string; recorded_at: string; method_name: string;
  source_type: string; source_record_id: number | null; quantity: number; coefficient: number; workload: number;
  linked_item_id: number | null; linked_item_name: string; linked_batch_id: string | null; evidence: string;
}
export interface LegacyAuxiliaryPage { rows: LegacyAuxiliaryRow[]; total: number; page: number; page_size: number }
export interface LegacyAuxiliaryPreview {
  rows: (LegacyAuxiliaryRow & { item_id: number; item_name: string; area_name: string; category: string })[];
  record_count: number; total_quantity: number; total_workload: number; confirmation_token: string; expires_at: string;
}
export interface LegacyAuxiliaryBatch {
  id: string; reason: string; created_by_name: string; created_at: string; record_count: number;
  total_quantity: number; total_workload: number; revoked_at: string | null;
}
export interface LegacyAuxiliaryBatches { rows: LegacyAuxiliaryBatch[]; total: number; page: number; page_size: number }
export interface LegacyAuxiliaryResult { batch_id: string; record_count: number; total_quantity: number; total_workload: number; replayed: boolean }
async function data<T>(request: Promise<{ data: ApiResponse<T> }>): Promise<T> {
  const value = requireApiSuccess((await request).data);
  if (value == null) throw new Error('服务器未返回关联结果，请重新查询');
  return value;
}
export const getLegacyAuxiliaryRecords = (params: HistoryQuery, signal?: AbortSignal) =>
  data(client.get<ApiResponse<LegacyAuxiliaryPage>>('/data-governance/legacy-auxiliary/records', { params, signal }));
export const previewLegacyAuxiliary = (assignments: { record_id: number; item_id: number }[], signal?: AbortSignal) =>
  data(client.post<ApiResponse<LegacyAuxiliaryPreview>>('/data-governance/legacy-auxiliary/preview', { assignments }, { signal }));
export const applyLegacyAuxiliary = (body: { confirmation_token: string; reason: string }) =>
  data(client.post<ApiResponse<LegacyAuxiliaryResult>>('/data-governance/legacy-auxiliary/apply', body));
export const getLegacyAuxiliaryBatches = (params: { page: number; page_size: number }, signal?: AbortSignal) =>
  data(client.get<ApiResponse<LegacyAuxiliaryBatches>>('/data-governance/legacy-auxiliary/batches', { params, signal }));
export const previewLegacyAuxiliaryUndo = (id: string, signal?: AbortSignal) =>
  data(client.get<ApiResponse<LegacyAuxiliaryPreview>>(`/data-governance/legacy-auxiliary/batches/${encodeURIComponent(id)}/undo-preview`, { signal }));
export const undoLegacyAuxiliary = (id: string, body: { confirmation_token: string; reason: string }) =>
  data(client.post<ApiResponse<LegacyAuxiliaryResult>>(`/data-governance/legacy-auxiliary/batches/${encodeURIComponent(id)}/undo`, body));
