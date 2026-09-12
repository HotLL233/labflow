export interface AdaptiveColumnConfig<T> {
  key: string;
  header: string;
  min?: number;
  max?: number;
  fixed?: number;
  padding?: number;
  wrap?: 'normal' | 'anywhere' | 'nowrap';
  getValue: (row: T) => unknown;
}

export interface AdaptiveColumnWidth {
  desktop: string;
  mobile: number;
}

const textUnits = (value: unknown): number => {
  const text = String(value ?? '');
  if (!text || text === '-') return 1;
  let units = 0;
  for (const char of text) {
    if (/[\u4e00-\u9fff]/.test(char)) units += 2;
    else if (/[A-Z0-9]/.test(char)) units += 1.05;
    else units += 0.85;
  }
  return Math.max(1, units);
};

const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

export const getAdaptiveColumnWidths = <T,>(
  rows: T[],
  columns: AdaptiveColumnConfig<T>[],
): Record<string, AdaptiveColumnWidth> => {
  const idealWidths = columns.map(col => {
    if (col.fixed) return { key: col.key, width: col.fixed };
    const min = col.min ?? 64;
    const max = col.max ?? 180;
    const padding = col.padding ?? 24;
    const headerUnits = textUnits(col.header);
    const bodyUnits = rows.reduce((longest, row) => Math.max(longest, textUnits(col.getValue(row))), 0);
    return {
      key: col.key,
      width: Math.round(clamp(Math.max(headerUnits, bodyUnits) * 8 + padding, min, max)),
    };
  });
  const totalWidth = idealWidths.reduce((sum, col) => sum + col.width, 0) || 1;

  return idealWidths.reduce<Record<string, AdaptiveColumnWidth>>((acc, col) => {
    acc[col.key] = {
      desktop: `${((col.width / totalWidth) * 100).toFixed(3)}%`,
      mobile: col.width,
    };
    return acc;
  }, {});
};

export const adaptiveTableSx = {
  width: { xs: 'max-content', sm: '100%' },
  minWidth: { xs: 'max-content', sm: '100%' },
  maxWidth: { sm: '100%' },
  tableLayout: 'fixed',
  '& .MuiTableCell-root': {
    whiteSpace: 'normal',
    overflowWrap: 'break-word',
    wordBreak: 'normal',
    verticalAlign: 'top',
    px: { xs: 0.75, sm: 0.5 },
  },
};

export const adaptiveCellSx = (width: AdaptiveColumnWidth) => ({
  width: { xs: width.mobile, sm: width.desktop },
  minWidth: { xs: width.mobile, sm: 0 },
  maxWidth: { xs: width.mobile, sm: width.desktop },
  whiteSpace: 'normal',
  overflowWrap: 'break-word',
  wordBreak: 'normal',
  verticalAlign: 'top',
});

export const formatDateTimeDisplay = (value: unknown): string => {
  const text = String(value ?? '').trim();
  if (!text) return '-';
  const normalized = text.replace('T', ' ').substring(0, 19);
  const separator = normalized.indexOf(' ');
  return separator < 0
    ? normalized
    : `${normalized.substring(0, separator)}\n${normalized.substring(separator + 1)}`;
};

export const adaptiveDateCellSx = {
  whiteSpace: 'pre-line',
  overflowWrap: 'normal',
  wordBreak: 'keep-all',
};
