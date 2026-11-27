import { client } from './http';
import type { ApiResponse, PaginatedResponse, WorkRecord } from '../types';
import type { WorkloadDetail } from './workloadReport';
import { canDeleteRecordTarget, canEditRecordTarget, type RecordMaintenanceTarget } from './recordMaintenance';

export interface PersonRecordRow extends WorkloadDetail {
  normal_record: WorkRecord | null;
  notes: string;
  status: string;
  created_by_name: string;
  auxiliary_maintenance?: { updated_at: string; can_delete: boolean } | null;
  maintenance_target?: RecordMaintenanceTarget | null;
}

export interface PersonRecordQuery {
  start: string;
  end: string;
  subject_user_id?: number;
  detection_division_ids?: string;
  sending_division_ids?: string;
  include_pending_ownership?: boolean;
  kind?: 'detection' | 'auxiliary';
  page: number;
  page_size: number;
}

export const getPersonRecords = (params: PersonRecordQuery): Promise<ApiResponse<PaginatedResponse<PersonRecordRow>>> =>
  client.get('/stats/person-records', { params }).then(response => response.data);

export const personRecordKey = (row: Pick<PersonRecordRow, 'origin' | 'id'>): string => `${row.origin}:${row.id}`;

export const canDeleteAuxiliaryRecord = (row: PersonRecordRow): boolean =>
  row.kind === 'auxiliary' && canDeleteRecordTarget(row.maintenance_target);

export const canEditAuxiliaryRecord = (row: PersonRecordRow): boolean =>
  row.kind === 'auxiliary' && canEditRecordTarget(row.maintenance_target);

export function patchDetectionRecord(rows: PersonRecordRow[], id: number, patch: Partial<WorkRecord>): PersonRecordRow[] {
  return rows.map(row => row.origin === 'work_record' && row.kind === 'detection' && row.id === id && row.normal_record
    ? { ...row, normal_record: { ...row.normal_record, ...patch } }
    : row);
}
