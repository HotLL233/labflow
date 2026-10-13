/** 列表与弹窗共用累计数量口径，部分录入仍允许继续。 */
export function workloadLabel(record: { workload_recorded?: boolean; recorded_quantity?: number; quantity: number }): string {
  if (record.workload_recorded) return '已录入';
  return (record.recorded_quantity ?? 0) > 0 ? `部分录入（${record.recorded_quantity}/${record.quantity}）` : '录入工作量';
}

export function workloadCoefficient(selected: { coefficient?: number } | undefined, fallback: number | undefined, requiresChoice: boolean): number | undefined {
  return requiresChoice ? selected?.coefficient : (fallback ?? 1);
}

export type SampleWorkloadTarget = {
  project_id: number; project_name: string; group_id: number; laboratory: string;
  division_id: number | null; department: string; coefficient: number;
};

export function findWorkloadTarget(targets: SampleWorkloadTarget[], projectId?: number | null, groupId?: number | null): SampleWorkloadTarget | undefined {
  return targets.find(target => target.project_id === projectId && target.group_id === groupId);
}

export function filterWorkloadInstruments<T extends { instrument_id: number }>(instruments: T[], instrumentId: number | ''): T[] {
  return instrumentId === '' ? instruments : instruments.filter(item => item.instrument_id === instrumentId);
}

/** 只保存明确的表单字段；待关联草稿不代表已经生成工作量。 */
export type SampleWorkloadDraft = {
  version: 1; quantity: number; multiplier: number; recorded_at: string; notes: string;
  custom_method_name: string; custom_instrument_name: string; custom: boolean;
  method_id?: number; project_id?: number; group_id?: number;
};

export function sampleWorkloadDraftKey(userId: number | undefined, sourceId: number): string | null {
  return Number.isSafeInteger(userId) && Number(userId) > 0 && Number.isSafeInteger(sourceId) && sourceId > 0
    ? `labflow:sample-workload-draft:${userId}:${sourceId}` : null;
}

export function parseSampleWorkloadDraft(raw: string | null): SampleWorkloadDraft | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw);
    if (!value || value.version !== 1 || !Number.isSafeInteger(value.quantity) || value.quantity <= 0
      || !Number.isFinite(value.multiplier) || value.multiplier < 0 || typeof value.custom !== 'boolean'
      || ['recorded_at', 'notes', 'custom_method_name', 'custom_instrument_name'].some(key => typeof value[key] !== 'string')
      || ['method_id', 'project_id', 'group_id'].some(key => value[key] !== undefined && (!Number.isSafeInteger(value[key]) || value[key] <= 0))) return null;
    return {
      version: 1, quantity: value.quantity, multiplier: value.multiplier, recorded_at: value.recorded_at, notes: value.notes,
      custom_method_name: value.custom_method_name, custom_instrument_name: value.custom_instrument_name, custom: value.custom,
      method_id: value.method_id, project_id: value.project_id, group_id: value.group_id,
    };
  } catch { return null; }
}

/** 与 rd_record_repo 的安全排序白名单一致。 */
export const RD_SORTABLE_FIELDS = new Set(['submitted_at', 'recorded_at', 'seq_no', 'created_at', 'status', 'business_no', 'batch_no', 'user_name', 'division_id', 'lab_name', 'project_name', 'method_name', 'detection_type', 'instrument_code', 'high_item', 'quantity']);
