/** 列表与弹窗共用累计数量口径，部分录入仍允许继续。 */
export function workloadLabel(record: { workload_recorded?: boolean; recorded_quantity?: number; quantity: number }): string {
  if (record.workload_recorded) return '已录入';
  return (record.recorded_quantity ?? 0) > 0 ? `部分录入（${record.recorded_quantity}/${record.quantity}）` : '录入工作量';
}

export function workloadCoefficient(selected: { coefficient?: number } | undefined, fallback: number | undefined, requiresChoice: boolean): number | undefined {
  return requiresChoice ? selected?.coefficient : (fallback ?? 1);
}

/** 与 rd_record_repo 的安全排序白名单一致。 */
export const RD_SORTABLE_FIELDS = new Set(['submitted_at', 'recorded_at', 'seq_no', 'created_at', 'status', 'business_no', 'batch_no', 'user_name', 'division_id', 'lab_name', 'project_name', 'method_name', 'detection_type', 'instrument_code', 'high_item', 'quantity']);
