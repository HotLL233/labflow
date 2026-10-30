import { client, requireApiSuccess } from './http';
import type { ApiResponse } from '../types';

export interface CoefficientDiagnosisRow {
  id: number; origin: 'work_record' | 'auxiliary_record'; business_no: string; user_name: string; recorded_at: string; project_name: string; lab_name: string;
  source_type: string; source_record_id: number | null; method_id: number | null; method_name: string;
  current_method_name: string; instrument_name: string; quantity: number; coefficient_snapshot: number | null;
  method_coefficient: number | null; before_workload: number | null; method_active: boolean | null;
  status: string; status_label: string; can_reassociate: boolean;
}
export interface CoefficientDiagnosis {
  rows: CoefficientDiagnosisRow[]; total: number; page: number; page_size: number;
  counts: { status: string; label: string; count: number }[];
}
export interface MethodAssociationPreview {
  record_id: number; business_no: string; old_method_name: string; new_method_name: string;
  old_method_id: number | null; new_method_id: number; coefficient_snapshot: number;
  method_coefficient: number; quantity: number; before_workload: number; after_workload: number;
  old_unit_price: number | null; new_unit_price: number | null; multiplier: number;
  before_amount: number | null; after_amount: number | null; amount_difference: number | null;
  confirmation_token: string; expires_at: string;
}
export interface HistoryQuery { start: string; end: string; keyword?: string; page: number; page_size: number }
async function data<T>(request: Promise<{ data: ApiResponse<T> }>): Promise<T> {
  const value = requireApiSuccess((await request).data);
  if (value == null) throw new Error('服务器未返回核对结果，请重新查询');
  return value;
}
export const diagnoseWorkloadCoefficients = (params: HistoryQuery & { status?: string }, signal?: AbortSignal) =>
  data(client.get<ApiResponse<CoefficientDiagnosis>>('/data-governance/work/coefficient/diagnose', { params, signal }));
export const previewMethodAssociation = (body: { record_id: number; method_id: number }, signal?: AbortSignal) =>
  data(client.post<ApiResponse<MethodAssociationPreview>>('/data-governance/work/coefficient/association/preview', body, { signal }));
export const applyMethodAssociation = (body: { confirmation_token: string; reason: string }) =>
  data(client.post<ApiResponse<{ updated_count: number }>>('/data-governance/work/coefficient/association/apply', body));
