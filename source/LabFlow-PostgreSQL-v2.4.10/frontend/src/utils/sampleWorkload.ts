/** 列表与弹窗共用累计数量口径，部分录入仍允许继续。 */
export function workloadLabel(record: { workload_recorded?: boolean; recorded_quantity?: number; quantity: number }): string {
  if (record.workload_recorded) return '已录入';
  return (record.recorded_quantity ?? 0) > 0 ? `部分录入（${record.recorded_quantity}/${record.quantity}）` : '录入工作量';
}

export function workloadCoefficient(selected: { coefficient?: number } | undefined, fallback: number | undefined, requiresChoice: boolean, customMode = false): number | undefined {
  return requiresChoice && !customMode ? selected?.coefficient : (fallback ?? 1);
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

type WorkloadSearchFields = {
  instrument_id: number; instrument: string; instrument_name?: string; instrument_code?: string; instrument_type?: string;
  method_name?: string; method_code?: string; method_short_name?: string; method_types?: string[];
};

/** 搜索只缩小有效库候选；关键词不用于创建或猜测标准 ID。 */
export function filterWorkloadOptions<T>(options: T[], query: string, searchText: (option: T) => string): T[] {
  const words = query.trim().toLowerCase().split(/\s+/u).filter(Boolean);
  return words.length === 0 ? options : options.filter(option => {
    const text = searchText(option).toLowerCase();
    return words.every(word => text.includes(word));
  });
}

/** MUI 的 reset 也会在失焦发生；仅输入、显式清空或选中库候选改变关键词。 */
export function nextWorkloadSearchQuery(current: string | null, input: string, reason: string): string | null {
  return reason === 'input' ? input : reason === 'clear' || reason === 'selectOption' ? null : current;
}

export function workloadSearchPending(query: string | null, selectedLabel: string): boolean {
  return query !== null && query !== selectedLabel;
}

export function workloadMethodSearchText(item: WorkloadSearchFields): string {
  return [item.method_name, item.method_short_name, item.method_code, item.instrument, item.instrument_name,
    item.instrument_code, item.instrument_type, ...(item.method_types ?? [])].filter(Boolean).join(' ');
}

export function workloadInstrumentLabel(item: WorkloadSearchFields): string {
  if (item.instrument_id === 0) return '未关联仪器';
  return item.instrument_name || '未命名仪器';
}

export function workloadMethodLabel(item: WorkloadSearchFields): string {
  const name = item.method_name || item.method_short_name || '未命名方法';
  return item.instrument_id === 0 ? name : `${name} · ${workloadInstrumentLabel(item)}`;
}

/** 同一仪器的所有方法共同提供搜索信息，不能只保留第一种检测类型。 */
export function workloadInstrumentOptions<T extends WorkloadSearchFields>(items: T[]): Array<T & { search_text: string; method_types: string[]; method_count: number }> {
  const options = new Map<number, T & { search_text: string; method_types: string[]; method_count: number }>();
  for (const item of items) {
    const previous = options.get(item.instrument_id);
    options.set(item.instrument_id, {
      ...(previous ?? item),
      method_types: [...new Set([...(previous?.method_types ?? []), ...(item.method_types ?? [])])],
      method_count: (previous?.method_count ?? 0) + 1,
      search_text: [previous?.search_text, workloadMethodSearchText(item)].filter(Boolean).join(' '),
    });
  }
  return [...options.values()];
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
