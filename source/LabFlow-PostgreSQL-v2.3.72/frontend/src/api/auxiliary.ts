import { client, requireApiSuccess } from './http';
import type { ApiResponse, PaginatedResponse } from '../types';

export interface AuxiliaryArea {
  id: number; name: string; enabled: boolean; sort_order: number; updated_at: string;
}
export interface AuxiliaryAreaWrite {
  name: string; enabled: boolean; sort_order: number; expected_updated_at?: string;
}
export interface AuxiliaryItem {
  id: number; name: string; coefficient: number; category: string; scope_type: string;
  area_type_id: number | null; area_name: string;
  group_id: number | null; group_name: string; enabled: boolean; sort_order: number; notes: string;
  updated_at: string; deleted_at: string | null;
}
export interface AuxiliaryContext {
  people: { id: number; username: string; division_id: number | null; division_name: string }[]; divisions: { id: number; name: string }[];
  record_people?: { id: number; username: string }[];
  groups: { id: number; name: string }[];
  areas: { id: number; name: string }[];
  can_create: boolean; can_manage_items: boolean; can_manage_records: boolean; can_edit_own: boolean;
}
export interface AuxiliaryRecord {
  id: number; origin: string; auxiliary_work_item_id: number | null; user_id: number | null; user_name: string;
  auxiliary_work_name: string; coefficient: number; category: string; quantity: number; recorded_at: string;
  area_type_id: number | null; area_name: string;
  notes: string; status: string; detection_division_id: number | null; detection_division_name: string;
  group_id: number | null; group_name: string; created_by: number | null; created_by_name: string;
  updated_at: string; deleted_at: string | null; can_edit: boolean; can_delete: boolean; can_restore: boolean;
  linked_work_record_id: number | null;
}
export interface AuxiliaryItemWrite {
  name: string; coefficient: number; category: string; enabled: boolean; sort_order: number; notes: string;
  expected_updated_at?: string;
  area_type_id?: number | null;
}
export interface AuxiliaryRecordWrite {
  auxiliary_work_item_id?: number; user_id?: number; detection_division_id?: number; group_id?: number;
  expected_item_updated_at?: string;
  quantity: number; recorded_at: string; notes: string; expected_updated_at?: string;
}
export interface AuxiliaryRecordQuery {
  start?: string; end?: string; user_id?: number; keyword?: string; item_id?: number;
  area_type_id?: number; category?: string;
  page: number; page_size: number; include_deleted?: boolean;
}
async function data<T>(request: Promise<{ data: ApiResponse<T> }>): Promise<T> {
  const result = requireApiSuccess((await request).data);
  if (result == null) throw new Error('服务器未返回数据，请刷新后重试');
  return result;
}
export const getAuxiliaryContext = () => data(client.get<ApiResponse<AuxiliaryContext>>('/auxiliary/context'));
export const getAuxiliaryAreas = () => data(client.get<ApiResponse<AuxiliaryArea[]>>('/auxiliary/areas'));
export const saveAuxiliaryArea = (area: AuxiliaryAreaWrite, id?: number) => data(id
  ? client.put<ApiResponse<AuxiliaryArea>>(`/auxiliary/areas/${id}`, area)
  : client.post<ApiResponse<AuxiliaryArea>>('/auxiliary/areas', area));
export const getAuxiliaryItems = (includeDeleted = false) => data(client.get<ApiResponse<AuxiliaryItem[]>>('/auxiliary/items', { params: { include_deleted: includeDeleted } }));
export const saveAuxiliaryItem = (item: AuxiliaryItemWrite, id?: number) => data(id
  ? client.put<ApiResponse<AuxiliaryItem>>(`/auxiliary/items/${id}`, item)
  : client.post<ApiResponse<AuxiliaryItem>>('/auxiliary/items', item));
export const getAuxiliaryRecords = (params: AuxiliaryRecordQuery) => data(client.get<ApiResponse<PaginatedResponse<AuxiliaryRecord>>>('/auxiliary/records', { params }));
export const saveAuxiliaryRecord = (record: AuxiliaryRecordWrite, id?: number) => data(id
  ? client.put<ApiResponse<AuxiliaryRecord>>(`/auxiliary/records/${id}`, record)
  : client.post<ApiResponse<AuxiliaryRecord>>('/auxiliary/records', record));
export async function changeAuxiliaryState(kind: 'items' | 'records', id: number, restore: boolean, expected_updated_at: string, reason: string) {
  const body = { reason, expected_updated_at };
  requireApiSuccess((await (restore ? client.post(`/auxiliary/${kind}/${id}/restore`, body) : client.delete(`/auxiliary/${kind}/${id}`, { data: body }))).data);
}
