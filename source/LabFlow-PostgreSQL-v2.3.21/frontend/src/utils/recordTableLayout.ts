/**
 * v2.3.20 记录表列宽引擎。
 *
 * 目标：一页显示全部列，且列内容尽量完整。
 *
 * 背景：此前列宽完全等于「后台配置 px 归一到百分比」，与内容无关 ——
 * 内容很短的列（部门 / 实验室 / 高项）拿到大量宽度，批号、主要成分、注意事项这类
 * 长内容列却装不下自身内容（样品信息登记记录的批号列只有 78px 要显示 18 个字符）。
 *
 * 现在按以下顺序决定列宽：
 * 1. `custom` 模式的列直接使用管理员配置宽度（等同升级前行为，尊重既有配置）；
 * 2. `auto` 模式的列按内容测量：`max(表头宽, 本页内容 P90 宽) + 单元格内边距`；
 * 3. 容器有富余时，优先补给「可扩展列」（批号 / 主要成分 / 注意事项），让长内容横向展开；
 * 4. 容器不足时，先从「可折行列」按超出量扣减到下限，仍不足才整体等比压缩。
 *
 * 上下限默认按输入方式给出，管理员可用 `min_width` / `max_width` 按列覆盖（0 表示沿用默认）。
 */

export type RecordWidthMode = 'auto' | 'custom';

export interface RecordColumnLayoutInput<T> {
  key: string;
  /** 表头文字，参与自然宽计算 */
  header: string;
  /** 输入方式，决定默认上下限与是否可折行 */
  dataType?: string;
  getValue: (row: T) => unknown;
  /** 管理员配置宽度，仅 `custom` 模式生效 */
  width?: number;
  widthMode?: RecordWidthMode;
  /** 0 或未传表示沿用系统默认下限 */
  minWidth?: number;
  /** 0 或未传表示沿用系统默认上限 */
  maxWidth?: number;
  /** 强制固定宽度（选择列等），不参与测量与压缩 */
  fixed?: number;
  /** 可扩展列：容器有富余时优先吸收 */
  extendable?: boolean;
}

export interface RecordColumnLayout {
  key: string;
  /** 最终像素宽度（供移动端卡片 / 拖拽计算使用） */
  px: number;
  /** 桌面端百分比宽度（配合 tableLayout: fixed 使用） */
  percent: string;
  /** 该列是否来自 custom 模式 */
  isCustom: boolean;
}

/** 12.5px 字号下的近似字符宽度（px），用于内容测量。 */
const CJK_WIDTH = 14;
const ASCII_WIDTH = 8;
const OTHER_WIDTH = 6;
/** 单元格左右内边距合计 */
const CELL_PADDING = 20;
/** 可扩展列最多放大到自然宽的倍数 */
const EXPAND_LIMIT = 2.2;

const TYPE_BOUNDS: Record<string, { min: number; max: number }> = {
  number: { min: 48, max: 96 },
  date: { min: 100, max: 160 },
  datetime: { min: 100, max: 160 },
  attachment: { min: 76, max: 150 },
  action: { min: 96, max: 170 },
  select: { min: 64, max: 170 },
  textarea: { min: 120, max: 360 },
  text: { min: 56, max: 240 },
};

/** 命中这些关键字的文本列按「长文本」处理，允许更宽的上限。 */
const LONG_TEXT_HINT = /备注|注意|成分|说明|描述|方法|批号|样品|物质|项目/;

const DEFAULT_BOUNDS = { min: 56, max: 200 };

const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

/** 单个字符串的显示宽度（px）。 */
export const measureTextWidth = (value: unknown): number => {
  const text = String(value ?? '').trim();
  if (!text || text === '-') return 0;
  let width = 0;
  for (const char of text) {
    const code = char.codePointAt(0) ?? 0;
    if (code >= 0x2e80 && code <= 0x9fff) width += CJK_WIDTH;
    else if (code <= 0x7f) width += ASCII_WIDTH;
    else width += OTHER_WIDTH;
  }
  return width;
};

/** 取列的默认上下限：优先按输入方式，其次按关键字的「长文本」判定。 */
export const resolveColumnBounds = (
  header: string,
  dataType: string | undefined,
  minWidth?: number,
  maxWidth?: number,
) => {
  const byType = TYPE_BOUNDS[(dataType || 'text').toLowerCase()] || DEFAULT_BOUNDS;
  const longText = LONG_TEXT_HINT.test(header);
  const fallbackMin = longText ? Math.max(byType.min, 110) : byType.min;
  const fallbackMax = longText ? 240 : byType.max;
  const min = minWidth && minWidth > 0 ? minWidth : fallbackMin;
  const max = maxWidth && maxWidth > 0 ? maxWidth : fallbackMax;
  return { min: Math.min(min, max), max: Math.max(min, max) };
};

/** 长文本列允许两行显示：超出一行时纵向展开，而不是被截断。 */
export const shouldWrapColumn = (header: string, dataType: string | undefined, naturalWidth: number) => {
  const type = (dataType || 'text').toLowerCase();
  if (type === 'number' || type === 'date' || type === 'datetime' || type === 'action' || type === 'attachment') {
    return false;
  }
  return LONG_TEXT_HINT.test(header) || naturalWidth > 140;
};

/** 内容 P90 宽度：避免单条超长数据把整列撑开。 */
const percentileWidth = (values: number[], ratio = 0.9): number => {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * ratio) - 1));
  return sorted[index];
};

export interface ComputeOptions {
  /** 内容区可用宽度（px）。未测量到或为 0 时退化为按自然宽比例分配。 */
  containerWidth: number;
}

/**
 * 计算列宽。返回每列的像素宽与百分比宽。
 *
 * 未传 containerWidth 时按自然宽输出百分比（保持旧调用方可用）。
 */
export function computeRecordColumnWidths<T>(
  rows: T[],
  columns: RecordColumnLayoutInput<T>[],
  options: ComputeOptions,
): Record<string, RecordColumnLayout> {
  // 容器尚未测量到时（首次渲染 / 页面处于加载态）用视口宽度估算，
  // 避免退化成「按自然宽比例」分配，把短列压到 20~30px、表头逐字竖排。
  // 真实宽度到位后 ResizeObserver 会立即纠正。
  const estimated = typeof window === 'undefined' ? 0 : Math.max(720, window.innerWidth - 320);
  const containerWidth = options.containerWidth > 0 ? options.containerWidth : estimated;

  const entries = columns.map(column => {
    const type = (column.dataType || 'text').toLowerCase();
    const isCustom = column.widthMode === 'custom' && Number(column.width) > 0;
    const bounds = resolveColumnBounds(column.header, type, column.minWidth, column.maxWidth);
    const headerWidth = measureTextWidth(column.header);

    if (column.fixed && column.fixed > 0) {
      return {
        key: column.key,
        natural: column.fixed,
        target: column.fixed,
        min: column.fixed,
        max: column.fixed,
        isCustom: false,
        extendable: false,
        wrap: false,
        fixed: true,
      };
    }

    if (isCustom) {
      // custom 模式：完全按管理员配置，不做内容测量，压缩时也尽量最后才动它。
      const width = clamp(Number(column.width), 48, 500);
      return {
        key: column.key,
        natural: width,
        target: width,
        min: width,
        max: width,
        isCustom: true,
        extendable: false,
        wrap: false,
        fixed: false,
      };
    }

    const bodyWidths = rows.map(row => measureTextWidth(column.getValue(row)));
    // 表头可能折行，按「至少能放 2 个字」估算，避免纯长表头把列撑到最宽。
    const bodyWidth = percentileWidth(bodyWidths);
    const natural = Math.round(Math.max(headerWidth, bodyWidth) + CELL_PADDING);
    const target = clamp(natural, bounds.min, bounds.max);
    return {
      key: column.key,
      natural,
      target,
      min: bounds.min,
      max: bounds.max,
      isCustom: false,
      extendable: Boolean(column.extendable),
      wrap: shouldWrapColumn(column.header, type, natural),
      fixed: false,
    };
  });

  const total = entries.reduce((sum, entry) => sum + entry.target, 0);

  if (!containerWidth || total <= 0) {
    const safeTotal = total || 1;
    return entries.reduce<Record<string, RecordColumnLayout>>((acc, entry) => {
      acc[entry.key] = {
        key: entry.key,
        px: entry.target,
        percent: `${((entry.target / safeTotal) * 100).toFixed(4)}%`,
        isCustom: entry.isCustom,
      };
      return acc;
    }, {});
  }

  const finalWidths = new Map<string, number>(entries.map(entry => [entry.key, entry.target]));

  if (total < containerWidth) {
    // 有富余：先给可扩展列，剩余再按比例均分给未触及上限的列。
    let surplus = containerWidth - total;
    const expandables = entries.filter(entry => !entry.fixed && entry.extendable);
    for (const entry of expandables) {
      if (surplus <= 0) break;
      const limit = Math.min(entry.max, Math.round(entry.natural * EXPAND_LIMIT));
      const room = Math.max(0, limit - entry.target);
      const give = Math.min(room, surplus);
      finalWidths.set(entry.key, entry.target + give);
      surplus -= give;
    }
    if (surplus > 0) {
      const flexible = entries.filter(entry => !entry.fixed && !entry.isCustom && !entry.extendable);
      const rooms = flexible.map(entry => Math.max(0, entry.max - (finalWidths.get(entry.key) || entry.target)));
      const roomTotal = rooms.reduce((sum, room) => sum + room, 0);
      if (roomTotal > 0) {
        flexible.forEach((entry, index) => {
          const give = Math.min(rooms[index], Math.round((rooms[index] / roomTotal) * surplus));
          finalWidths.set(entry.key, (finalWidths.get(entry.key) || entry.target) + give);
        });
      }
    }
  } else if (total > containerWidth) {
    // 空间不足：先从可折行列按「可扣减量」比例扣到下限，仍不足才整体等比压缩。
    let deficit = total - containerWidth;
    const shrinkables = entries.filter(entry => !entry.fixed && entry.wrap && !entry.isCustom);
    const capacities = shrinkables.map(entry => Math.max(0, entry.target - entry.min));
    const capacityTotal = capacities.reduce((sum, value) => sum + value, 0);
    if (capacityTotal > 0) {
      const applied = Math.min(capacityTotal, deficit);
      shrinkables.forEach((entry, index) => {
        const take = Math.round((capacities[index] / capacityTotal) * applied);
        finalWidths.set(entry.key, (finalWidths.get(entry.key) || entry.target) - take);
      });
      deficit -= applied;
    }
    if (deficit > 0) {
      const finalTotal = entries.reduce((sum, entry) => sum + (finalWidths.get(entry.key) || entry.target), 0) || 1;
      entries.forEach(entry => {
        const current = finalWidths.get(entry.key) || entry.target;
        finalWidths.set(entry.key, Math.max(40, Math.round((current / finalTotal) * containerWidth)));
      });
    }
  }

  const finalTotal = entries.reduce((sum, entry) => sum + (finalWidths.get(entry.key) || entry.target), 0) || 1;
  return entries.reduce<Record<string, RecordColumnLayout>>((acc, entry) => {
    const px = finalWidths.get(entry.key) || entry.target;
    acc[entry.key] = {
      key: entry.key,
      px,
      percent: `${((px / finalTotal) * 100).toFixed(4)}%`,
      isCustom: entry.isCustom,
    };
    return acc;
  }, {});
}

/**
 * 操作列宽度三档（v2.3.20）：
 * - `full`：始终显示全部按钮，按按钮总宽计算（等同升级前）；
 * - `auto`：容器宽 ≥1400 文字档、≥1024 图标档、其余收纳档。
 */
export type ActionColumnMode = 'auto' | 'full';

export interface ActionColumnLayout {
  /** 容器宽度对应的档位 */
  tier: 'text' | 'icon' | 'compact';
  /** 是否只显示「主操作 + 更多 ▾」 */
  collapsed: boolean;
  px: number;
}

export function computeActionColumnLayout(
  mode: ActionColumnMode,
  containerWidth: number,
  buttonWidthSum: number,
  textThreshold = 1400,
  iconThreshold = 1024,
): ActionColumnLayout {
  if (mode === 'full') {
    return { tier: 'text', collapsed: false, px: clamp(buttonWidthSum + 24, 120, 1600) };
  }
  if (containerWidth >= textThreshold) {
    return { tier: 'text', collapsed: false, px: 160 };
  }
  if (containerWidth >= iconThreshold) {
    return { tier: 'icon', collapsed: true, px: 152 };
  }
  return { tier: 'compact', collapsed: true, px: 96 };
}
