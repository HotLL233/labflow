import type { WorkRecord } from '../types';

export const RD_OPERATION_LABELS: Record<string, string> = {
  pending_sample: '待取样', sampled_unrecorded: '已取样未录入', partial: '部分录入', recorded: '全部录入',
  return_pending_edit: '退回待修改', returned: '已退回', return_confirmed: '退回已确认', voided: '已作废', detected: '已完成检测',
};

/** 人数仅按稳定账号 ID 去重，历史缺失身份单独提示，不能猜成取样人。 */
export function workloadRecorderLabel(record: Pick<WorkRecord, 'workload_recorders' | 'workload_unknown_recorder_entries' | 'recorded_quantity' | 'workload_recorded'>): string {
  const people = [...new Map((record.workload_recorders || []).map(person => [person.user_id, person])).values()];
  const unknown = (record.workload_unknown_recorder_entries || 0) > 0;
  if (!people.length) return record.workload_recorded || (record.recorded_quantity || 0) > 0 ? '历史账号未关联' : '';
  const first = people[0].username || `账号 #${people[0].user_id}`;
  return `${first}${people.length > 1 ? `等 ${people.length} 人` : ''}${unknown ? ' · 含历史账号未关联' : ''}`;
}

export function rdFilterQuery(filters: Record<string, string[]>, operations: string[], recorders: string[]) {
  const active = Object.fromEntries(Object.entries(filters).filter(([, values]) => values.length));
  return {
    column_filters: Object.keys(active).length ? JSON.stringify(active) : undefined,
    operation_states: operations.length ? operations.join(',') : undefined,
    recorder_ids: recorders.length ? recorders.join(',') : undefined,
  };
}

/** 日期快捷含末日全天；datetime-local 输入保留精确时刻和毫秒。 */
export function recordTimeDraft(value: string, end = false): string {
  return /^\d{4}-\d{2}-\d{2}$/.test(value) ? `${value}T${end ? '23:59:59.999' : '00:00:00'}` : value.replace(' ', 'T');
}

export function validateRecordTimeRange(start: string, end: string): string {
  const valid = (value: string) => {
    const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/.exec(value);
    if (!match) return false;
    const [, y, m, d, hh, mm, ss = '0'] = match;
    const date = new Date(0); date.setUTCFullYear(Number(y), Number(m) - 1, Number(d));
    return date.getUTCFullYear() === Number(y) && date.getUTCMonth() + 1 === Number(m) && date.getUTCDate() === Number(d)
      && Number(hh) < 24 && Number(mm) < 60 && Number(ss) < 60;
  };
  if ((start && !valid(start)) || (end && !valid(end))) return '请输入有效日期与时刻';
  if (start && end && Date.parse(`${start}+08:00`) > Date.parse(`${end}+08:00`)) return '开始时刻不能晚于结束时刻';
  return '';
}
