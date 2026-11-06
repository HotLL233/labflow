export type RecordSortState = { field: string; direction: 'asc' | 'desc' };

/** 在首次请求前读取排序；损坏或不可用的本机配置不阻塞记录页。 */
export function readRecordSort(key: string, allowedFields?: ReadonlySet<string>): RecordSortState {
  const fallback: RecordSortState = { field: 'submitted_at', direction: 'desc' };
  if (!key) return fallback;
  try {
    const saved = JSON.parse(localStorage.getItem(key) || 'null');
    return typeof saved?.field === 'string' && saved.field.length > 0
      && (!allowedFields || allowedFields.has(saved.field))
      && (saved.direction === 'asc' || saved.direction === 'desc')
      ? { field: saved.field, direction: saved.direction } : fallback;
  } catch { return fallback; }
}
