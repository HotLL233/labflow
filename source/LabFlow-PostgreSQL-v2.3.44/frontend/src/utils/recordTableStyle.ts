/**
 * v2.3.25 记录表 Excel 化样式：规格定义、规范化与解析引擎。
 *
 * 三套记录页（分析检测登记 / 研发送样记录 / 样品信息登记）共用同一份规格：
 *   表级（行高、表头行高、边框、斑马纹）
 *   表头样式 / 表体样式
 *   列级样式（表体列、表头列）
 *   行级样式（按记录标识固定）
 *   条件格式规则（按字段值命中整行或某一列）
 *
 * 设计约定：
 * 1. 样式只在读取时解析，任何位置都不做「逐格重算」——解析结果按
 *    「列 + 行 + 是否表头」缓存，规格或数据变化时整块重建。
 * 2. 颜色、字号等一律做白名单校验，避免后台写入的脏值把界面弄坏。
 * 3. 行级与条件格式样式都基于「数据值」定位，不使用行号，
 *    因此翻页、排序、筛选后不会错位。
 */

export type StyleAlign = 'left' | 'center' | 'right';
export type StyleVAlign = 'top' | 'middle';
export type StyleWrap = 'single' | 'wrap' | 'clamp2';

export interface CellStyle {
  fontFamily?: string;
  /** 单位 px */
  fontSize?: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  color?: string;
  bgColor?: string;
  align?: StyleAlign;
  vAlign?: StyleVAlign;
  wrap?: StyleWrap;
  /** 列宽（px）。只对列级样式有意义，设置后该列按固定宽度渲染。 */
  width?: number;
  /** 日期 / 日期时间显示格式，如 yyyy/MM/dd。留空表示沿用页面原有格式。 */
  dateFormat?: string;
}

export interface TableLevelStyle {
  /** 表体行高（px） */
  rowHeight: number;
  /** 表头行高（px） */
  headerHeight: number;
  borderColor: string;
  borderWidth: number;
  /** 斑马纹 */
  zebra: boolean;
  zebraColor: string;
  /** 冻结（吸顶）表头 */
  stickyHeader: boolean;
  /** 显示选择复选框列 */
  showCheckbox: boolean;
  /** 显示序号列 */
  showSeq: boolean;
  /** 操作列展示：自动、全部显示并换行、折叠到更多 */
  actionDisplay: 'auto' | 'full' | 'collapsed';
}

/**
 * v2.3.32：窄屏卡片布局的边框与底色。
 * 卡片外框与卡片内字段块各有一套颜色，方便做成「外深内浅」这类层次。
 */
export interface CardLevelStyle {
  /** 卡片外框 */
  borderColor: string;
  borderWidth: number;
  /** 卡片底色，空字符串表示跟随主题默认 */
  bgColor: string;
  /** 卡片内字段块边框 */
  fieldBorderColor: string;
  fieldBorderWidth: number;
  /** 字段块底色，空字符串表示跟随页面默认 */
  fieldBgColor: string;
  /** 三行时合并末两组字段，保留全部摘要信息。 */
  summaryRows: 3 | 4;
  /** null 表示使用页面默认分组，组内顺序即展示顺序。 */
  summaryFields: string[][] | null;
}

/** v2.3.32：分页设置。size 为 0 表示沿用各页面原有每页条数。 */
export interface PagingStyle {
  size: number;
}

export const PAGE_SIZE_OPTIONS = [20, 50, 100, 200];

/**
 * v2.3.34：记录页布局方式。
 * - `auto`：按设备判定。移动设备用卡片；桌面设备（含手机浏览器开启「桌面网站」后上报的桌面 UA）用表格。
 * - `card`：强制卡片（窄窗口想省地方时用）。
 * - `table`：强制表格。
 */
export type LayoutMode = 'auto' | 'card' | 'table';

export interface LayoutStyle {
  mode: LayoutMode;
}

/** 极窄视口兜底：这个宽度下表格必然要横向滚动，直接回落卡片。 */
export const VERY_NARROW_QUERY = '(max-width:767.95px)';

const MOBILE_UA = /Android|webOS|iPhone|iPod|BlackBerry|IEMobile|Opera Mini|Mobile|HarmonyOS/i;

/**
 * 是否为移动设备。
 *
 * 手机浏览器开启「桌面网站」后 UA 会变成桌面 UA —— 按需求那种情况应当显示 PC 表格，
 * 所以这里只认真正的移动 UA；iPadOS 13+ 上报桌面 UA，用触点数兜底。
 */
export function detectMobileDevice(): boolean {
  if (typeof navigator === 'undefined') return false;
  const ua = navigator.userAgent || '';
  if (MOBILE_UA.test(ua)) return true;
  return /Macintosh/.test(ua) && (navigator.maxTouchPoints ?? 0) > 1;
}

/** 布局判定：auto 以设备类型为主，仅在极窄视口才回落到卡片。 */
export function resolveCardLayout(mode: LayoutMode, mobileDevice: boolean, veryNarrowViewport: boolean): boolean {
  if (mode === 'card') return true;
  if (mode === 'table') return false;
  return mobileDevice || veryNarrowViewport;
}

export type RuleOp = 'eq' | 'neq' | 'contains' | 'notContains' | 'gt' | 'gte' | 'lt' | 'lte' | 'empty' | 'notEmpty';

export const RULE_OP_LABELS: Record<RuleOp, string> = {
  eq: '等于',
  neq: '不等于',
  contains: '包含',
  notContains: '不包含',
  gt: '大于',
  gte: '大于等于',
  lt: '小于',
  lte: '小于等于',
  empty: '为空',
  notEmpty: '不为空',
};

export interface RuleCondition {
  field: string;
  op: RuleOp;
  value: string;
}

export interface ConditionalRule {
  id: string;
  name: string;
  enabled: boolean;
  /** row = 整行；column = 仅 rule.column 指定的列 */
  range: 'row' | 'column';
  column?: string;
  match: 'all' | 'any';
  conditions: RuleCondition[];
  style: CellStyle & { leftBar?: string };
}

export interface RecordTableStyleSpec {
  version: 1;
  table: TableLevelStyle;
  /** 窄屏卡片的边框与底色 */
  card: CardLevelStyle;
  /** 每页条数 */
  paging: PagingStyle;
  /** 卡片 / 表格布局方式 */
  layout: LayoutStyle;
  header: CellStyle;
  body: CellStyle;
  columns: Record<string, CellStyle>;
  headerColumns: Record<string, CellStyle>;
  rows: Record<string, CellStyle>;
  rules: ConditionalRule[];
  /** 隐藏的列（字段名）。个人视图与全局样式各自独立保存。 */
  hiddenColumns: string[];
}

export const RECORD_TABLE_STYLE_VERSION = 1;

export const DEFAULT_RECORD_TABLE_STYLE: RecordTableStyleSpec = {
  version: RECORD_TABLE_STYLE_VERSION,
  table: {
    rowHeight: 38,
    headerHeight: 36,
    borderColor: '#d9dfe7',
    borderWidth: 1,
    zebra: false,
    zebraColor: '#fafbfd',
    stickyHeader: true,
    showCheckbox: true,
    showSeq: true,
    actionDisplay: 'auto',
  },
  card: {
    borderColor: '#e0e0e0',
    borderWidth: 1,
    bgColor: '',
    fieldBorderColor: '#d9dfe7',
    fieldBorderWidth: 1,
    fieldBgColor: '',
    summaryRows: 4,
    summaryFields: null,
  },
  paging: {
    // 0 = 沿用各页面原有的每页条数，避免升级后自动改变请求量。
    size: 0,
  },
  layout: {
    // auto = 移动设备用卡片、桌面设备用表格（手机开「桌面网站」即按桌面处理）。
    mode: 'auto',
  },
  header: {
    // 与表体同理：颜色、底色、换行都留给各页面原有表头样式（表头显示模式），
    // 避免升级后统一覆盖掉后台配置的「单行省略 / 自动换行」。
    bold: true,
    vAlign: 'middle',
  },
  body: {
    // align / wrap / bgColor 默认不设置：对齐、换行与行底色沿用各页面原有策略
    // （序号居中、长文本两行截断、已检测行浅灰），只有显式配置时才覆盖。
    fontSize: 12,
    color: '#1f2d3d',
    vAlign: 'top',
  },
  columns: {},
  headerColumns: {},
  rows: {},
  rules: [],
  hiddenColumns: [],
};

/** 字体白名单：只允许系统里一定存在的字体，避免出现不可控的字体回退。 */
export const FONT_OPTIONS = [
  { value: '', label: '跟随全局' },
  { value: 'Microsoft YaHei', label: '微软雅黑' },
  { value: 'SimSun', label: '宋体' },
  { value: 'SimHei', label: '黑体' },
  { value: 'KaiTi', label: '楷体' },
];

export const FONT_SIZE_OPTIONS = [11, 12, 13, 14, 16, 18];

export const COLOR_OPTIONS = [
  '#1f2d3d', '#5b6b7c', '#8d9aa8', '#b53b00', '#c62828', '#ad1457',
  '#6a1b9a', '#1976d2', '#0277bd', '#00695c', '#2e7d32', '#6d4c41',
];

export const FILL_OPTIONS = [
  '#ffffff', '#f5f7f5', '#fafbfd', '#eef3fb', '#e8f1fb', '#fff2e0',
  '#fff5f5', '#fdeaea', '#edf6ee', '#fdf3e3', '#f3ecf7', '#eceff1',
];

const HEX_COLOR = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i;
const ALIGNS: StyleAlign[] = ['left', 'center', 'right'];
const VALIGNS: StyleVAlign[] = ['top', 'middle'];
const WRAPS: StyleWrap[] = ['single', 'wrap', 'clamp2'];
const RULE_OPS: RuleOp[] = ['eq', 'neq', 'contains', 'notContains', 'gt', 'gte', 'lt', 'lte', 'empty', 'notEmpty'];

const safeColor = (value: unknown): string | undefined => {
  if (typeof value !== 'string') return undefined;
  const text = value.trim();
  return HEX_COLOR.test(text) ? text.toLowerCase() : undefined;
};

const safeNumber = (value: unknown, min: number, max: number): number | undefined => {
  const num = Number(value);
  if (!Number.isFinite(num)) return undefined;
  return Math.min(max, Math.max(min, Math.round(num)));
};

const safeBool = (value: unknown): boolean | undefined =>
  typeof value === 'boolean' ? value : undefined;

const safeEnum = <T extends string>(value: unknown, allowed: T[]): T | undefined =>
  typeof value === 'string' && (allowed as string[]).includes(value) ? value as T : undefined;

/** 日期 / 日期时间显示格式。空值表示沿用页面原有格式。 */
export const DATE_FORMAT_OPTIONS: { value: string; label: string }[] = [
  { value: '', label: '默认格式' },
  { value: 'yyyy-MM-dd', label: '2026-09-30' },
  { value: 'yyyy/MM/dd', label: '2026/09/30' },
  { value: 'yyyy年MM月dd日', label: '2026年09月30日' },
  { value: 'MM-dd', label: '09-30' },
  { value: 'yyyy-MM-dd HH:mm', label: '2026-09-30 14:30' },
  { value: 'yyyy-MM-dd HH:mm:ss', label: '2026-09-30 14:30:00' },
  { value: 'yyyy/MM/dd HH:mm', label: '2026/09/30 14:30' },
  { value: 'yyyy/MM/dd HH:mm:ss', label: '2026/09/30 14:30:00' },
  { value: 'yyyy.MM.dd HH:mm:ss', label: '2026.09.30 14:30:00' },
  { value: 'yyyyMMdd', label: '20260930' },
  { value: 'yyyyMMdd HH:mm:ss', label: '20260930 14:30:00' },
  { value: 'MM/dd HH:mm:ss', label: '09/30 14:30:00' },
  { value: 'MM/dd HH:mm', label: '09/30 14:30' },
  { value: 'MM/dd/yyyy HH:mm:ss', label: '09/30/2026 14:30:00' },
  { value: 'dd/MM/yyyy HH:mm:ss', label: '30/09/2026 14:30:00' },
  { value: 'yyyy年MM月dd日 HH:mm', label: '2026年09月30日 14:30' },
  { value: 'yyyy年MM月dd日 HH:mm:ss', label: '2026年09月30日 14:30:00' },
  { value: 'yyyy-MM', label: '2026-09' },
  { value: 'MM月dd日', label: '09月30日' },
  { value: 'HH:mm', label: '14:30' },
  { value: 'HH:mm:ss', label: '14:30:00' },
];

const DATE_FORMAT_VALUES = DATE_FORMAT_OPTIONS.map(item => item.value).filter(Boolean);

const safeDateFormat = (value: unknown): string | undefined =>
  typeof value === 'string' && DATE_FORMAT_VALUES.includes(value) ? value : undefined;

/** 按格式串渲染日期时间；无法识别的时间串返回 null，由调用方决定回退显示。 */
export function formatDateByPattern(value: unknown, pattern?: string): string | null {
  if (!pattern) return null;
  const text = String(value ?? '').trim().replace('T', ' ');
  if (!text) return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})(?:[ ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?)?/.exec(text);
  if (!match) return null;
  const year = match[1];
  const month = match[2];
  const day = match[3];
  const hour = match[4] ?? '';
  const minute = match[5] ?? '';
  const second = match[6] ?? '';
  return pattern
    .replace(/yyyy/g, year)
    .replace(/MM/g, month)
    .replace(/dd/g, day)
    .replace(/HH/g, hour)
    .replace(/mm/g, minute)
    .replace(/ss/g, second);
}

/** 把任意来源的样式对象收敛成可信的 CellStyle。 */
export function normalizeCellStyle(raw: unknown): CellStyle {
  if (!raw || typeof raw !== 'object') return {};
  const source = raw as Record<string, unknown>;
  const style: CellStyle = {};
  const fontFamily = typeof source.fontFamily === 'string' ? source.fontFamily.trim() : '';
  if (fontFamily) style.fontFamily = fontFamily.slice(0, 60);
  const fontSize = safeNumber(source.fontSize, 9, 24);
  if (fontSize !== undefined) style.fontSize = fontSize;
  const bold = safeBool(source.bold);
  if (bold !== undefined) style.bold = bold;
  const italic = safeBool(source.italic);
  if (italic !== undefined) style.italic = italic;
  const underline = safeBool(source.underline);
  if (underline !== undefined) style.underline = underline;
  const color = safeColor(source.color);
  if (color) style.color = color;
  const bgColor = safeColor(source.bgColor);
  if (bgColor) style.bgColor = bgColor;
  const align = safeEnum(source.align, ALIGNS);
  if (align) style.align = align;
  const vAlign = safeEnum(source.vAlign, VALIGNS);
  if (vAlign) style.vAlign = vAlign;
  const wrap = safeEnum(source.wrap, WRAPS);
  if (wrap) style.wrap = wrap;
  const width = safeNumber(source.width, 40, 800);
  if (width !== undefined) style.width = width;
  const dateFormat = safeDateFormat(source.dateFormat);
  if (dateFormat) style.dateFormat = dateFormat;
  return style;
}

const normalizeTableLevel = (raw: unknown): TableLevelStyle => {
  const source = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return {
    rowHeight: safeNumber(source.rowHeight, 24, 90) ?? DEFAULT_RECORD_TABLE_STYLE.table.rowHeight,
    headerHeight: safeNumber(source.headerHeight, 24, 90) ?? DEFAULT_RECORD_TABLE_STYLE.table.headerHeight,
    borderColor: safeColor(source.borderColor) ?? DEFAULT_RECORD_TABLE_STYLE.table.borderColor,
    borderWidth: safeNumber(source.borderWidth, 0, 3) ?? DEFAULT_RECORD_TABLE_STYLE.table.borderWidth,
    zebra: safeBool(source.zebra) ?? DEFAULT_RECORD_TABLE_STYLE.table.zebra,
    zebraColor: safeColor(source.zebraColor) ?? DEFAULT_RECORD_TABLE_STYLE.table.zebraColor,
    stickyHeader: safeBool(source.stickyHeader) ?? DEFAULT_RECORD_TABLE_STYLE.table.stickyHeader,
    showCheckbox: safeBool(source.showCheckbox) ?? DEFAULT_RECORD_TABLE_STYLE.table.showCheckbox,
    showSeq: safeBool(source.showSeq) ?? DEFAULT_RECORD_TABLE_STYLE.table.showSeq,
    actionDisplay: safeEnum(source.actionDisplay, ['auto', 'full', 'collapsed'] as const) ?? DEFAULT_RECORD_TABLE_STYLE.table.actionDisplay,
  };
};

const normalizeStyleMap = (raw: unknown): Record<string, CellStyle> => {
  if (!raw || typeof raw !== 'object') return {};
  const entries = Object.entries(raw as Record<string, unknown>);
  const result: Record<string, CellStyle> = {};
  entries.slice(0, 200).forEach(([key, value]) => {
    if (!key || key.length > 80) return;
    const style = normalizeCellStyle(value);
    if (Object.keys(style).length > 0) result[key] = style;
  });
  return result;
};

const normalizeConditions = (raw: unknown): RuleCondition[] => {
  if (!Array.isArray(raw)) return [];
  return raw.slice(0, 6).flatMap(item => {
    if (!item || typeof item !== 'object') return [];
    const source = item as Record<string, unknown>;
    const field = typeof source.field === 'string' ? source.field.trim() : '';
    const op = safeEnum(source.op, RULE_OPS);
    if (!field || !op) return [];
    return [{
      field: field.slice(0, 80),
      op,
      value: typeof source.value === 'string' ? source.value.slice(0, 200) : String(source.value ?? ''),
    }];
  });
};

const normalizeRules = (raw: unknown): ConditionalRule[] => {
  if (!Array.isArray(raw)) return [];
  return raw.slice(0, 20).flatMap((item, index) => {
    if (!item || typeof item !== 'object') return [];
    const source = item as Record<string, unknown>;
    const conditions = normalizeConditions(source.conditions);
    if (conditions.length === 0) return [];
    const style = normalizeCellStyle(source);
    const leftBar = safeColor(source.leftBar);
    const range = source.range === 'column' ? 'column' as const : 'row' as const;
    return [{
      id: typeof source.id === 'string' && source.id ? source.id.slice(0, 60) : `rule-${index + 1}`,
      name: typeof source.name === 'string' && source.name.trim() ? source.name.trim().slice(0, 60) : `规则 ${index + 1}`,
      enabled: source.enabled === false ? false : true,
      range,
      column: typeof source.column === 'string' ? source.column.slice(0, 80) : undefined,
      match: source.match === 'any' ? 'any' as const : 'all' as const,
      conditions,
      style: leftBar ? { ...style, leftBar } : style,
    }];
  });
};

const normalizeCardLevel = (raw: unknown): CardLevelStyle => {
  const source = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return {
    borderColor: safeColor(source.borderColor) ?? DEFAULT_RECORD_TABLE_STYLE.card.borderColor,
    borderWidth: safeNumber(source.borderWidth, 0, 4) ?? DEFAULT_RECORD_TABLE_STYLE.card.borderWidth,
    bgColor: safeColor(source.bgColor) ?? '',
    fieldBorderColor: safeColor(source.fieldBorderColor) ?? DEFAULT_RECORD_TABLE_STYLE.card.fieldBorderColor,
    fieldBorderWidth: safeNumber(source.fieldBorderWidth, 0, 4) ?? DEFAULT_RECORD_TABLE_STYLE.card.fieldBorderWidth,
    fieldBgColor: safeColor(source.fieldBgColor) ?? '',
    summaryRows: source.summaryRows === 3 ? 3 : 4,
    summaryFields: Array.isArray(source.summaryFields)
      ? normalizeSummaryFields(source.summaryFields)
      : null,
  };
};

export function normalizeSummaryFields(groups: unknown[]): string[][] {
  const used = new Set<string>();
  return Array.from({ length: 4 }, (_, index) => (Array.isArray(groups[index]) ? groups[index] : []).filter((key: unknown): key is string => {
    if (typeof key !== 'string' || !key.trim() || key.length > 80 || used.has(key)) return false;
    used.add(key);
    return true;
  }));
}

export function cardSummaryGroups(card: CardLevelStyle, defaults: string[][]): string[][] {
  const groups = normalizeSummaryFields(card.summaryFields ?? defaults);
  return card.summaryRows === 3 ? [groups[0], groups[1], [...groups[2], ...groups[3]]] : groups;
}

/** 摘要为管理员共享配置，其他个人视觉偏好仍保留。 */
export function resolveRecordTableStyle(global: RecordTableStyleSpec, personal: RecordTableStyleSpec | null, isAdmin: boolean): RecordTableStyleSpec {
  if (!personal) return global;
  if (isAdmin) return personal;
  return { ...personal, card: { ...personal.card, summaryRows: global.card.summaryRows, summaryFields: global.card.summaryFields } };
}

/** 每页条数只接受预设档位，避免异常值把接口一次拉爆。 */
const normalizePaging = (raw: unknown): PagingStyle => {
  const source = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const size = safeNumber(source.size, 0, 500) ?? 0;
  return { size: PAGE_SIZE_OPTIONS.includes(size) ? size : 0 };
};

const normalizeLayout = (raw: unknown): LayoutStyle => {
  const source = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const mode = safeEnum(source.mode, ['auto', 'card', 'table'] as LayoutMode[]);
  return { mode: mode ?? DEFAULT_RECORD_TABLE_STYLE.layout.mode };
};

/** 把存储中的 JSON 收敛为可用规格；任何缺项都回落到系统默认，保证升级后旧配置仍可用。 */
export function normalizeRecordTableStyle(raw: unknown): RecordTableStyleSpec {
  let parsed: unknown = raw;
  if (typeof raw === 'string') {
    try {
      parsed = JSON.parse(raw);
    } catch {
      parsed = null;
    }
  }
  if (!parsed || typeof parsed !== 'object') {
    return {
      ...DEFAULT_RECORD_TABLE_STYLE,
      table: { ...DEFAULT_RECORD_TABLE_STYLE.table },
      card: { ...DEFAULT_RECORD_TABLE_STYLE.card },
      paging: { ...DEFAULT_RECORD_TABLE_STYLE.paging },
      layout: { ...DEFAULT_RECORD_TABLE_STYLE.layout },
      header: { ...DEFAULT_RECORD_TABLE_STYLE.header },
      body: { ...DEFAULT_RECORD_TABLE_STYLE.body },
      columns: {}, headerColumns: {}, rows: {}, rules: [], hiddenColumns: [],
    };
  }
  const source = parsed as Record<string, unknown>;
  return {
    version: RECORD_TABLE_STYLE_VERSION,
    table: normalizeTableLevel(source.table),
    card: normalizeCardLevel(source.card),
    paging: normalizePaging(source.paging),
    layout: normalizeLayout(source.layout),
    header: { ...DEFAULT_RECORD_TABLE_STYLE.header, ...normalizeCellStyle(source.header) },
    body: { ...DEFAULT_RECORD_TABLE_STYLE.body, ...normalizeCellStyle(source.body) },
    columns: normalizeStyleMap(source.columns),
    headerColumns: normalizeStyleMap(source.headerColumns),
    rows: normalizeStyleMap(source.rows),
    rules: normalizeRules(source.rules),
    hiddenColumns: Array.isArray(source.hiddenColumns)
      ? source.hiddenColumns.filter((item): item is string => typeof item === 'string' && item.length > 0).slice(0, 80)
      : [],
  };
}

/** 规格里是否包含任何自定义内容（用于界面提示「已自定义」）。 */
export function hasCustomStyle(spec: RecordTableStyleSpec): boolean {
  return Object.keys(spec.columns).length > 0
    || Object.keys(spec.headerColumns).length > 0
    || Object.keys(spec.rows).length > 0
    || spec.rules.length > 0
    || spec.table.zebra
    || spec.table.rowHeight !== DEFAULT_RECORD_TABLE_STYLE.table.rowHeight
    || spec.table.headerHeight !== DEFAULT_RECORD_TABLE_STYLE.table.headerHeight
    || spec.table.borderColor !== DEFAULT_RECORD_TABLE_STYLE.table.borderColor
    || Object.keys(spec.header).length > Object.keys(DEFAULT_RECORD_TABLE_STYLE.header).length
    || Object.keys(spec.body).length > Object.keys(DEFAULT_RECORD_TABLE_STYLE.body).length;
}

const compareNumber = (left: number, right: number): number | null => {
  if (!Number.isFinite(left) || !Number.isFinite(right)) return null;
  return left === right ? 0 : (left > right ? 1 : -1);
};

/** 单条条件求值。数值型条件优先按数值比较，否则退化为字符串比较。 */
export function evaluateCondition(condition: RuleCondition, rowValues: Record<string, string>): boolean {
  const raw = rowValues[condition.field];
  const text = raw === undefined || raw === null ? '' : String(raw);
  const value = condition.value ?? '';
  switch (condition.op) {
    case 'empty':
      return text.trim() === '';
    case 'notEmpty':
      return text.trim() !== '';
    case 'eq':
      return text.trim() === value.trim();
    case 'neq':
      return text.trim() !== value.trim();
    case 'contains':
      return value ? text.includes(value) : true;
    case 'notContains':
      return value ? !text.includes(value) : true;
    default: {
      const order = compareNumber(Number(text), Number(value));
      if (order === null) return false;
      switch (condition.op) {
        case 'gt': return order > 0;
        case 'gte': return order >= 0;
        case 'lt': return order < 0;
        case 'lte': return order <= 0;
        default: return false;
      }
    }
  }
}

export function evaluateRule(rule: ConditionalRule, rowValues: Record<string, string>): boolean {
  if (!rule.enabled || rule.conditions.length === 0) return false;
  const results = rule.conditions.map(condition => evaluateCondition(condition, rowValues));
  return rule.match === 'any' ? results.some(Boolean) : results.every(Boolean);
}

/** 规则是否命中指定列：整行规则命中所有列，列规则只命中目标列。 */
export function ruleHitColumn(rule: ConditionalRule, columnKey: string): boolean {
  return rule.range === 'row' || !rule.column || rule.column === columnKey;
}

export interface ResolveContext {
  columnKey: string;
  isHeader?: boolean;
  rowKey?: string;
  rowValues?: Record<string, string>;
  /** 从 0 开始的行序号，用于斑马纹 */
  rowIndex?: number;
}

const pickCellStyle = (source: CellStyle): CellStyle => {
  const result: CellStyle = {};
  if (source.fontFamily !== undefined) result.fontFamily = source.fontFamily;
  if (source.fontSize !== undefined) result.fontSize = source.fontSize;
  if (source.bold !== undefined) result.bold = source.bold;
  if (source.italic !== undefined) result.italic = source.italic;
  if (source.underline !== undefined) result.underline = source.underline;
  if (source.color !== undefined) result.color = source.color;
  if (source.bgColor !== undefined) result.bgColor = source.bgColor;
  if (source.align !== undefined) result.align = source.align;
  if (source.vAlign !== undefined) result.vAlign = source.vAlign;
  if (source.wrap !== undefined) result.wrap = source.wrap;
  if (source.width !== undefined) result.width = source.width;
  if (source.dateFormat !== undefined) result.dateFormat = source.dateFormat;
  return result;
};

export interface ResolvedCell {
  style: CellStyle;
  leftBar?: string;
}

/** 计算单元格最终样式（不含 MUI 转换），供界面与调试共用。 */
export function resolveCellStyle(spec: RecordTableStyleSpec, context: ResolveContext): ResolvedCell {
  const isHeader = Boolean(context.isHeader);
  const columnKey = context.columnKey;
  let style: CellStyle = pickCellStyle(isHeader ? spec.header : spec.body);
  let leftBar: string | undefined;

  const columnStyle = isHeader ? spec.headerColumns[columnKey] : spec.columns[columnKey];
  if (columnStyle) style = { ...style, ...pickCellStyle(columnStyle) };

  if (!isHeader) {
    if (context.rowKey) {
      const rowStyle = spec.rows[context.rowKey];
      if (rowStyle) style = { ...style, ...pickCellStyle(rowStyle) };
    }
    if (spec.rules.length > 0 && context.rowValues) {
      spec.rules.forEach(rule => {
        if (!ruleHitColumn(rule, columnKey)) return;
        if (!evaluateRule(rule, context.rowValues!)) return;
        style = { ...style, ...pickCellStyle(rule.style) };
        if (rule.style.leftBar) leftBar = rule.style.leftBar;
      });
    }
  }

  return { style, leftBar };
}

export type CellSx = Record<string, unknown>;

/** 单元格样式 → MUI sx。表格栅格、换行与溢出策略都在这里统一，避免各页面各写一遍。 */
export function cellStyleToSx(spec: RecordTableStyleSpec, resolved: ResolvedCell, isHeader: boolean, zebra: boolean): CellSx {
  const { style } = resolved;
  const wrap = style.wrap;
  const bgColor = zebra && style.bgColor === spec.body.bgColor ? spec.table.zebraColor : style.bgColor;
  const sx: CellSx = {
    boxSizing: 'border-box',
    borderRadius: '2px',
    minWidth: 0,
    maxWidth: '100%',
    border: `${spec.table.borderWidth}px solid ${spec.table.borderColor}`,
    fontSize: `${style.fontSize ?? 12}px`,
    lineHeight: 1.45,
    color: style.color,
    bgcolor: bgColor,
    textAlign: style.align,
    verticalAlign: style.vAlign === 'middle' ? 'middle' : 'top',
    px: 0.75,
    py: isHeader ? 0.5 : 0.5,
  };
  if (style.fontFamily) sx.fontFamily = style.fontFamily;
  if (style.bold) sx.fontWeight = 700;
  if (style.italic) sx.fontStyle = 'italic';
  if (style.underline) sx.textDecoration = 'underline';
  if (isHeader) sx.height = `${spec.table.headerHeight}px`;

  // 未配置换行方式时不写任何换行相关属性，沿用页面原有策略。
  if (wrap === 'single') {
    sx.whiteSpace = 'nowrap';
    sx.display = 'block';
    sx.overflow = 'hidden';
    sx.textOverflow = 'ellipsis';
  } else if (wrap === 'clamp2') {
    sx.whiteSpace = 'normal';
    sx.display = '-webkit-box';
    sx.WebkitLineClamp = 2;
    sx.WebkitBoxOrient = 'vertical';
    sx.overflow = 'hidden';
    sx.overflowWrap = 'anywhere';
  } else if (wrap === 'wrap') {
    sx.whiteSpace = 'normal';
    sx.display = 'block';
    sx.overflowWrap = 'anywhere';
    sx.wordBreak = 'break-word';
    sx.overflow = 'auto';
    sx.maxHeight = 220;
  }
  if (resolved.leftBar) sx.boxShadow = `inset 3px 0 0 ${resolved.leftBar}`;
  return sx;
}

/** 文本流属性只能落在单元格内部容器，不能改变 td/th 的 table-cell 布局。 */
function cellGridSx(sx: CellSx): CellSx {
  const grid: CellSx = { ...sx };
  TEXT_FLOW_KEYS.forEach(key => { delete grid[key]; });
  return grid;
}

export interface StyleResolver {
  cellSx: (columnKey: string, rowKey: string | undefined, rowIndex: number) => CellSx;
  headerSx: (columnKey: string) => CellSx;
  headerFlowSx: (columnKey: string) => CellSx;
  /** 单元格内层容器的文本流样式（已缓存，避免每帧新建对象拖慢滚动） */
  flowSx: (columnKey: string, rowKey: string | undefined, rowIndex: number) => CellSx;
  /** 只含字体与颜色，供窄屏卡片布局的字段容器复用（不搬边框与内边距） */
  visualSx: (columnKey: string, rowKey: string | undefined, rowIndex: number) => CellSx;
  /** 整行容器样式：行级手工样式 + 命中的整行条件格式（底色与左侧色条） */
  rowSx: (rowKey: string) => CellSx;
}

const VISUAL_KEYS = [
  'fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'textDecoration',
  'color', 'bgcolor', 'textAlign',
] as const;

/** 从单元格样式中取出「看得见」的字体与颜色属性。 */
export function visualFromCellSx(sx: CellSx): CellSx {
  const visual: CellSx = {};
  VISUAL_KEYS.forEach(key => {
    if (sx[key] !== undefined) visual[key] = sx[key];
  });
  return visual;
}

const TEXT_FLOW_KEYS = [
  'whiteSpace', 'display', 'overflow', 'textOverflow', 'WebkitLineClamp',
  'WebkitBoxOrient', 'overflowWrap', 'wordBreak', 'maxHeight',
] as const;

/**
 * 取出样式里的「文本流」属性（换行 / 截断 / 滚动）。
 * 记录页的单元格内部通常还有一层 Box 负责排版，只把外层 td 的样式改掉不生效，
 * 因此页面需要把这一小组属性同步到内层容器上。
 */
export function textFlowFromCellSx(sx: CellSx): CellSx {
  const flow: CellSx = {};
  TEXT_FLOW_KEYS.forEach(key => {
    if (sx[key] !== undefined) flow[key] = sx[key];
  });
  return flow;
}

/**
 * 构建解析器。同一份规格 + 同一批数据只会解析一次每个单元格，
 * 页面重新渲染时直接命中缓存；规格或数据变化时调用方重建解析器即可。
 */
export function createStyleResolver(
  spec: RecordTableStyleSpec,
  rowValuesByKey: Record<string, Record<string, string>>,
): StyleResolver {
  const cache = new Map<string, CellSx>();
  const flowCache = new Map<string, CellSx>();
  const visualCache = new Map<string, CellSx>();
  const rowCache = new Map<string, CellSx>();

  const resolve = (columnKey: string, rowKey: string | undefined, rowIndex: number, isHeader: boolean): CellSx => {
    const cacheKey = `${isHeader ? 'h' : 'b'}|${columnKey}|${rowKey ?? ''}|${rowIndex}`;
    const cached = cache.get(cacheKey);
    if (cached) return cached;
    const resolved = resolveCellStyle(spec, {
      columnKey,
      isHeader,
      rowKey,
      rowValues: rowKey ? rowValuesByKey[rowKey] : undefined,
      rowIndex,
    });
    const zebra = !isHeader && spec.table.zebra && rowIndex % 2 === 1;
    const sx = cellStyleToSx(spec, resolved, isHeader, zebra);
    cache.set(cacheKey, sx);
    return sx;
  };

  return {
    cellSx: (columnKey, rowKey, rowIndex) => cellGridSx(resolve(columnKey, rowKey, rowIndex, false)),
    headerSx: columnKey => cellGridSx(resolve(columnKey, undefined, -1, true)),
    headerFlowSx: columnKey => textFlowFromCellSx(resolve(columnKey, undefined, -1, true)),
    flowSx: (columnKey, rowKey, rowIndex) => {
      const cacheKey = `${columnKey}|${rowKey ?? ''}|${rowIndex}`;
      const cached = flowCache.get(cacheKey);
      if (cached) return cached;
      const next = textFlowFromCellSx(resolve(columnKey, rowKey, rowIndex, false));
      flowCache.set(cacheKey, next);
      return next;
    },
    visualSx: (columnKey, rowKey, rowIndex) => {
      const cacheKey = `${columnKey}|${rowKey ?? ''}|${rowIndex}`;
      const cached = visualCache.get(cacheKey);
      if (cached) return cached;
      const next = visualFromCellSx(resolve(columnKey, rowKey, rowIndex, false));
      visualCache.set(cacheKey, next);
      return next;
    },
    rowSx: rowKey => {
      const cached = rowCache.get(rowKey);
      if (cached) return cached;
      let style: CellStyle = spec.rows[rowKey] ? pickCellStyle(spec.rows[rowKey]) : {};
      let leftBar: string | undefined;
      const values = rowValuesByKey[rowKey];
      if (values) {
        spec.rules.forEach(rule => {
          // 窄屏卡片是整块容器，只有「整行」规则能落到底色上；单列规则由字段容器承载。
          if (rule.range !== 'row') return;
          if (!evaluateRule(rule, values)) return;
          style = { ...style, ...pickCellStyle(rule.style) };
          if (rule.style.leftBar) leftBar = rule.style.leftBar;
        });
      }
      const sx: CellSx = {};
      if (style.bgColor) sx.bgcolor = style.bgColor;
      if (style.color) sx.color = style.color;
      if (style.bold) sx.fontWeight = 700;
      if (style.fontSize) sx.fontSize = `${style.fontSize}px`;
      if (leftBar) sx.borderLeft = `4px solid ${leftBar}`;
      rowCache.set(rowKey, sx);
      return sx;
    },
  };
}

/**
 * 表格本体样式：行高、吸顶表头。
 *
 * v2.3.30：页面为了让表头能放右键菜单/拖拽手柄，会给表头单元格单独设 `position: relative`，
 * 该选择器与 MUI `stickyHeader` 的优先级相同，谁后注入谁生效 —— 结果就是吸顶时有时无。
 * 这里用「表头单元格 + stickyHeader 类」两级选择器强制吸顶，优先级更高，不再被覆盖。
 */
export function tableSxFromSpec(spec: RecordTableStyleSpec, defaultHeaderBg = '#f5f7f5'): CellSx {
  const sx: CellSx = {
    '& td': { height: `${spec.table.rowHeight}px` },
  };
  if (spec.table.stickyHeader) {
    sx['& .MuiTableCell-stickyHeader'] = {
      position: 'sticky',
      top: 0,
      zIndex: 4,
      bgcolor: spec.header.bgColor || defaultHeaderBg,
      backgroundImage: 'none',
    };
  }
  return sx;
}

export const RECORD_TABLE_STYLE_STORAGE_PREFIX = 'labflow.record-table-style:';

/** 窄屏卡片外框样式（颜色与宽度可在「表格样式 → 手机卡片」里单独配置）。 */
export function cardContainerSx(spec: RecordTableStyleSpec): CellSx {
  const sx: CellSx = { border: `${spec.card.borderWidth}px solid ${spec.card.borderColor}` };
  if (spec.card.bgColor) sx.bgcolor = spec.card.bgColor;
  return sx;
}

/** 窄屏卡片内字段块样式，与卡片外框各自独立配色。 */
export function cardFieldSx(spec: RecordTableStyleSpec): CellSx {
  const sx: CellSx = { border: `${spec.card.fieldBorderWidth}px solid ${spec.card.fieldBorderColor}` };
  if (spec.card.fieldBgColor) sx.bgcolor = spec.card.fieldBgColor;
  return sx;
}

export function personalStyleKey(moduleKey: string, userId?: number | null): string {
  return `${RECORD_TABLE_STYLE_STORAGE_PREFIX}${moduleKey}${userId ? `:${userId}` : ''}:v1`;
}

export function loadPersonalStyle(storageKey: string): RecordTableStyleSpec | null {
  if (!storageKey) return null;
  try {
    const raw = localStorage.getItem(storageKey);
    if (!raw) return null;
    return normalizeRecordTableStyle(raw);
  } catch {
    return null;
  }
}

export function savePersonalStyle(storageKey: string, spec: RecordTableStyleSpec): void {
  if (!storageKey) return;
  try {
    localStorage.setItem(storageKey, JSON.stringify(spec));
  } catch {
    // 本机存储不可用时只影响记忆，不影响本次会话。
  }
}

export function clearPersonalStyle(storageKey: string): void {
  if (!storageKey) return;
  try {
    localStorage.removeItem(storageKey);
  } catch {
    // 忽略：清不掉也不影响会话内的显示。
  }
}

export function globalStyleSettingKey(moduleKey: string): string {
  return `record_table_style_${moduleKey}`;
}

let ruleSeq = 0;

export function createRuleId(): string {
  ruleSeq += 1;
  return `rule-${Date.now().toString(36)}-${ruleSeq}`;
}
