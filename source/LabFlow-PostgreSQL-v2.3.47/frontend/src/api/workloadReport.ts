import { client, downloadFile } from './http';
import type { ApiResponse } from '../types';

export interface WorkloadMetrics {
  detection_quantity: number;
  detection_workload: number;
  auxiliary_quantity: number;
  auxiliary_workload: number;
  total_workload: number;
  record_count: number;
}
export interface WorkloadPerson extends WorkloadMetrics { user_id: number | null; user_name: string; }
export interface WorkloadMatrixRow extends WorkloadPerson { type: string; }
export interface WorkloadBreakdown extends WorkloadMetrics { key: string; name: string; }
export interface WorkloadDetail {
  id: number; kind: string; user_id: number | null; user_name: string; recorded_at: string;
  department: string; lab: string; project: string; method: string; instrument: string;
  type: string; source: string; quantity: number; coefficient: number; workload: number;
  source_record_id: number | null; ownership_status: string;
}
export interface WorkloadReport {
  scope: { view: 'mine' | 'scope'; can_view_scope: boolean; subject_user_id: number | null; allowed_division_ids: number[] | null; };
  summary: WorkloadMetrics;
  people: WorkloadPerson[];
  matrix: WorkloadMatrixRow[];
  breakdowns: Record<string, WorkloadBreakdown[]>;
  trend: (WorkloadMetrics & { period: string })[];
  details: { page: number; page_size: number; total: number; items: WorkloadDetail[]; };
}
export interface WorkloadReportQuery {
  start: string; end: string; view: 'mine' | 'scope'; subject_user_id?: number;
  division_id?: number; group_id?: number; project_id?: number; method_id?: number;
  instrument_id?: number; type?: string; source?: string; group_by?: 'day' | 'week' | 'month';
  include_pending_ownership?: boolean; include_zero_users?: boolean; page?: number; page_size?: number;
}
export const getWorkloadReport = (params: WorkloadReportQuery): Promise<ApiResponse<WorkloadReport>> =>
  client.get('/stats/workload-report', { params }).then(r => r.data);
export const exportWorkloadReport = (params: WorkloadReportQuery): Promise<void> =>
  downloadFile('/api/stats/workload-report.xlsx', params, `工作量汇总_${params.start}_${params.end}.xlsx`);
