import { validateRecordTimeRange } from './rdRecordFilters';

/** datetime-local 使用北京时间墙上时刻，不能按浏览器所在时区或截断 UTC ISO。 */
export function formatBeijingDateTimeInput(unixMs = Date.now()): string {
  const date = new Date(unixMs);
  if (!Number.isFinite(date.getTime())) return '';
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23',
  }).formatToParts(date);
  const value = (type: string) => parts.find(part => part.type === type)?.value || '';
  return `${value('year').padStart(4, '0')}-${value('month')}-${value('day')}T${value('hour')}:${value('minute')}:${value('second')}`;
}

/** 原样保存选择的北京时间；原生控件省略零秒时补齐为秒精度。 */
export function normalizeWorkloadDateTime(value: string): string | null {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2})?$/.test(value)
    || value.startsWith('0000-') || validateRecordTimeRange(value, '')) return null;
  return value.length === 16 ? `${value}:00` : value;
}
