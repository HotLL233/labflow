import { changeAuxiliaryState, saveAuxiliaryRecord } from './auxiliary';
import { requireApiSuccess } from './http';
import { deleteRecord, updateRecord } from './workload';

export interface RecordMaintenanceTarget {
  origin: 'work_record' | 'auxiliary_record';
  id: number;
  revision: string;
  can_edit: boolean;
  can_delete: boolean;
}

const validRecordTarget = (target: RecordMaintenanceTarget | null | undefined): target is RecordMaintenanceTarget =>
  !!target && (target.origin === 'work_record' || target.origin === 'auxiliary_record') &&
  Number.isSafeInteger(target.id) && target.id > 0 &&
  typeof target.revision === 'string' && !!target.revision.trim();

export const canDeleteRecordTarget = (target: RecordMaintenanceTarget | null | undefined): target is RecordMaintenanceTarget =>
  validRecordTarget(target) && target.can_delete === true;

export const canEditRecordTarget = (target: RecordMaintenanceTarget | null | undefined): target is RecordMaintenanceTarget =>
  validRecordTarget(target) && target.can_edit === true;

export interface RecordMetadataWrite {
  quantity: number;
  recorded_at: string;
  notes: string;
}

export async function updateRecordMetadata(target: RecordMaintenanceTarget | null | undefined, values: RecordMetadataWrite): Promise<void> {
  if (!canEditRecordTarget(target)) throw new Error('该记录不可编辑，请刷新后重试');
  const body = { quantity: values.quantity, recorded_at: values.recorded_at, notes: values.notes };
  if (target.origin === 'auxiliary_record') {
    await saveAuxiliaryRecord({ ...body, expected_updated_at: target.revision }, target.id);
  } else {
    requireApiSuccess(await updateRecord(target.id, body, target.revision));
  }
}

export async function deleteRecordTarget(target: RecordMaintenanceTarget | null | undefined, reason?: string): Promise<void> {
  if (!canDeleteRecordTarget(target)) throw new Error('该记录不可删除，请刷新后重试');
  if (!reason?.trim()) throw new Error('请填写删除原因');
  if (target.origin === 'auxiliary_record') {
    await changeAuxiliaryState('records', target.id, false, target.revision, reason.trim());
  } else {
    requireApiSuccess(await deleteRecord(target.id, reason.trim(), target.revision));
  }
}
