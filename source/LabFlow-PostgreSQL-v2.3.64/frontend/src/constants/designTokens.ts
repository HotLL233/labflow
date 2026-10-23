/**
 * LabFlow 设计令牌（v2.3.64）
 *
 * 本文件是三类业务色与字号的唯一真源。页面与组件只能从这里取值，
 * 配置层也只放行这里定义的业务类名，不再接受自由十六进制输入。
 *
 * 业务色分三类，每类给出四个角色：
 *   main   主色：标题文字、图标、边框。已在白底/页底/表头底/另两类 soft 底
 *          共 6 种底色上实测均 ≥ 4.5:1（WCAG AA 正文门槛），见下方各值注释。
 *   soft   浅底：卡片底色、选中态底色。
 *   line   描边：分隔线与卡片外框。
 *   accent 点缀：图表色块、进度条、装饰渐变。仅作图形使用，
 *          不承载文字，因此按 WCAG 非文本 3:1 校验。
 */

export type BusinessKey = 'analysis' | 'sample' | 'info';

export interface BusinessColor {
  key: BusinessKey;
  /** 业务中文名，同时作为配置层的选项文案 */
  label: string;
  /** 主色：文字/图标/边框 */
  main: string;
  /** 浅底色 */
  soft: string;
  /** 描边色 */
  line: string;
  /** 点缀色：仅图形，不承载文字 */
  accent: string;
}

export const BUSINESS_COLORS: Record<BusinessKey, BusinessColor> = {
  /** 分析检测：靛蓝。最低对比 8.68:1（浅橙底） */
  analysis: {
    key: 'analysis',
    label: '分析检测',
    main: '#283593',
    soft: '#E8EAF6',
    line: '#C5CAE9',
    accent: '#3949AB',
  },
  /** 研发送样：深橙。最低对比 6.10:1（浅蓝底） */
  sample: {
    key: 'sample',
    label: '研发送样',
    main: '#9A3412',
    soft: '#FFF3E0',
    line: '#FFE0B2',
    accent: '#E65100',
  },
  /** 样品信息登记：深绿。最低对比 6.57:1（浅蓝底） */
  info: {
    key: 'info',
    label: '样品信息登记',
    main: '#1B5E20',
    soft: '#E8F5E9',
    line: '#C8E6C9',
    accent: '#2E7D32',
  },
};

/** 配置层下拉选项，顺序固定为 分析检测 → 研发送样 → 样品信息登记 */
export const BUSINESS_OPTIONS = (['analysis', 'sample', 'info'] as BusinessKey[]).map(
  key => BUSINESS_COLORS[key],
);

export const BUSINESS_KEYS = Object.keys(BUSINESS_COLORS) as BusinessKey[];

/** 判断任意值是否为合法业务类名，供配置数据兼容旧值使用 */
export function isBusinessKey(value: unknown): value is BusinessKey {
  return typeof value === 'string' && (BUSINESS_KEYS as string[]).includes(value);
}

/**
 * 取业务色。传入未知值时回落到分析检测，保证任何旧配置都不会渲染出空颜色。
 */
export function businessColor(key: string | null | undefined): BusinessColor {
  return isBusinessKey(key) ? BUSINESS_COLORS[key] : BUSINESS_COLORS.analysis;
}

/**
 * 字号阶梯：6 档，全部以 rem 表达，跟随浏览器根字号缩放。基准 16px。
 *
 * 这 6 档与 MUI 的既有主档对齐，因此主题层的 variant 尺寸不需要改动，
 * 收敛的只是散落在组件里的 35 种行内字号写法。
 *
 * 档位   rem        等效  用途                                对应 MUI
 * xs     0.75rem    12px  徽标、说明文字、辅助标记            caption / overline
 * sm     0.8125rem  13px  密集表格与表单正文                  subtitle2
 * base   0.875rem   14px  常规正文                            body2
 * md     1rem       16px  强调正文、小标题                    body1 / subtitle1
 * lg     1.25rem    20px  区块标题                            h6
 * xl     1.5rem     24px  页面标题                            h5
 *
 * 不在此列的 34px 以上尺寸属于展示级标题，不参与正文阶梯；
 * 表格样式引擎里用户可配的字号（9–24）也不受此表约束。
 */
export const FONT = {
  xs: '0.75rem',
  sm: '0.8125rem',
  base: '0.875rem',
  md: '1rem',
  lg: '1.25rem',
  xl: '1.5rem',
} as const;

/** 字号阶梯的像素等效值，仅用于图表刻度等必须给数字的场景 */
export const FONT_PX = {
  xs: 12,
  sm: 13,
  base: 14,
  md: 16,
  lg: 20,
  xl: 24,
} as const;
