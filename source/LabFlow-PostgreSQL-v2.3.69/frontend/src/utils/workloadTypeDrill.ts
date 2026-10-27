import type { WorkloadBreakdown, WorkloadReportQuery } from '../api/workloadReport';

export type WorkloadDrillDimension = 'type' | 'method' | 'instrument' | 'auxiliary_category' | 'auxiliary_method';
export interface WorkloadDrillStep { dimension: WorkloadDrillDimension; key: string; name: string; }

export function nextWorkloadDimension(path: WorkloadDrillStep[], preferred: WorkloadDrillDimension): WorkloadDrillDimension | null {
  if (!path.length) return 'type';
  const auxiliary = path[0].key === '辅助工作';
  if (path.length === 1) return auxiliary
    ? preferred === 'auxiliary_method' ? 'auxiliary_method' : 'auxiliary_category'
    : preferred === 'instrument' ? 'instrument' : 'method';
  const last = path[path.length - 1].dimension;
  if (auxiliary) return last === 'auxiliary_category' ? 'auxiliary_method' : null;
  return path.length === 2 ? last === 'method' ? 'instrument' : 'method' : null;
}

export function workloadDrillQuery(base: WorkloadReportQuery, path: WorkloadDrillStep[], page = 0, pageSize = 25): WorkloadReportQuery {
  const query = { ...base, page: page + 1, page_size: pageSize, compact_type_summary: true };
  for (const step of path) {
    if (step.dimension === 'type') { query.type = step.key; query.kind = step.key === '辅助工作' ? 'auxiliary' : 'detection'; }
    else if (step.dimension === 'method') { query.method_id = undefined; query.method_key = step.key; }
    else if (step.dimension === 'instrument') { query.instrument_id = undefined; query.instrument_key = step.key; }
    else if (step.dimension === 'auxiliary_category') query.auxiliary_category = step.key;
    else query.auxiliary_key = step.key;
  }
  return query;
}

// Personnel choices cover the authorized date range, independently of the selected person and drill path.
export function workloadPeopleQuery(base: WorkloadReportQuery): WorkloadReportQuery {
  return {
    start: base.start, end: base.end, view: base.view,
    include_pending_ownership: base.include_pending_ownership, include_zero_users: base.include_zero_users,
    compact_type_summary: true, include_people: true, page: 1, page_size: 1,
  };
}

export const workloadShare = (value: number, parentTotal: number): string =>
  parentTotal > 0 ? `${(value / parentTotal * 100).toLocaleString('zh-CN', { maximumFractionDigits: 2 })}%` : '—';

export function workloadDetailPersonLabel(person: { user_id: number | null; user_name: string }, peers: { user_id: number | null; user_name: string }[]): string {
  if (person.user_id === null) return `${person.user_name}（历史未绑定）`;
  const identities = new Set(peers.filter(row => row.user_name === person.user_name).map(row => row.user_id));
  const ids = [...identities].filter((id): id is number => id !== null).sort((a, b) => a - b);
  return identities.size > 1 ? `${person.user_name}（同名账号 ${ids.indexOf(person.user_id) + 1}）` : person.user_name;
}

export function workloadBreakdownLabel(dimension: WorkloadDrillDimension, row: WorkloadBreakdown, peers: WorkloadBreakdown[]): string {
  const name = row.name || '未分类';
  if (row.key.startsWith('legacy:')) return `${name}（历史未绑定）`;
  const sameName = peers.filter(other => other.name === row.name);
  if (sameName.length < 2) return name;
  const auxiliaryKind = row.key.startsWith('method:') ? 'method:' : row.key.startsWith('item:') ? 'item:' : null;
  const candidates = sameName.filter(other => !other.key.startsWith('legacy:') && (!auxiliaryKind || other.key.startsWith(auxiliaryKind)));
  const keys = candidates.map(other => other.key).sort((a, b) => a.localeCompare(b, 'en', { numeric: true }));
  const label = auxiliaryKind === 'method:' ? '辅助方法' : auxiliaryKind === 'item:' ? '辅助事项' : dimension === 'instrument' ? '仪器' : '方法';
  return `${name}（${auxiliaryKind && candidates.length === 1 ? label : `同名${label} ${keys.indexOf(row.key) + 1}`}）`;
}
