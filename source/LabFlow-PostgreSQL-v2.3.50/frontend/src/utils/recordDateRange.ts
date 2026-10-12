export type RecordDatePreset = 'all' | 'this-week' | 'last-week' | 'recent';
export interface RecordDateRange { start: string; end: string; }

const asDay = (value: string) => new Date(`${value}T00:00:00Z`);
const iso = (value: Date) => value.toISOString().slice(0, 10);
const addDays = (value: Date, days: number) => new Date(value.getTime() + days * 86_400_000);

export function validRecordDate(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const date = asDay(value);
  return Number.isFinite(date.getTime()) && iso(date) === value;
}

export function validateRecordDateRange(start: string, end: string): string {
  if (!start || !end) return '请选择开始日期和结束日期';
  if (!validRecordDate(start) || !validRecordDate(end)) return '日期格式无效，请选择有效日期';
  return start > end ? '开始日期不能晚于结束日期' : '';
}

export function recordDatePreset(preset: RecordDatePreset, today: string): RecordDateRange {
  if (preset === 'all') return { start: '', end: '' };
  if (!validRecordDate(today)) throw new Error('无效的当前日期');
  const date = asDay(today);
  if (preset === 'recent') return { start: iso(addDays(date, -6)), end: today };
  const monday = addDays(date, -((date.getUTCDay() + 6) % 7) + (preset === 'last-week' ? -7 : 0));
  return { start: iso(monday), end: iso(addDays(monday, 6)) };
}

export function shiftRecordWeek(start: string, weeks: number): RecordDateRange {
  if (!validRecordDate(start)) throw new Error('无效的周开始日期');
  const monday = addDays(asDay(start), weeks * 7);
  return { start: iso(monday), end: iso(addDays(monday, 6)) };
}
