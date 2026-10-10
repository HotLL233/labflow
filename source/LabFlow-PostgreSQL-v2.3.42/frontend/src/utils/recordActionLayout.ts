export interface RecordActionLayout {
  before_column: string;
  button_order: string[];
}

export function normalizeActionLayout(value: unknown): RecordActionLayout {
  const source = value && typeof value === 'object' ? value as Partial<RecordActionLayout> : {};
  return {
    before_column: typeof source.before_column === 'string' ? source.before_column : '',
    button_order: Array.isArray(source.button_order)
      ? [...new Set(source.button_order.filter((key): key is string => typeof key === 'string'))] : [],
  };
}

/** A hidden/deleted anchor falls back to the end; never drop the action column. */
export function placeActionColumn(keys: string[], before: string): string[] {
  const result = keys.filter(key => key !== '_action');
  const index = result.indexOf(before);
  result.splice(index < 0 ? result.length : index, 0, '_action');
  return result;
}

/** New actions keep their configured/default order after explicitly ordered actions. */
export function sortActionItems<T extends { key: string }>(items: T[], order: string[]): T[] {
  return [...items].sort((a, b) => {
    const rank = (key: string) => { const index = order.indexOf(key); return index < 0 ? order.length : index; };
    return rank(a.key) - rank(b.key);
  });
}

export const RD_ACTIONS = [
  { key: 'return', label: '退回送样' }, { key: 'withdraw', label: '撤回取样' },
  { key: 'workload', label: '录入工作量' }, { key: 'confirm', label: '确认退回并修改' },
  { key: 'edit', label: '编辑' },
];
