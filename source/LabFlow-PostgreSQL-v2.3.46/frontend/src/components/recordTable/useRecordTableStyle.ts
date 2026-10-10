import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getSetting, updateSetting } from '../../api/admin';
import { useUser } from '../../UserContext';
import {
  DEFAULT_RECORD_TABLE_STYLE, clearPersonalStyle, createStyleResolver, globalStyleSettingKey,
  loadPersonalStyle, normalizeRecordTableStyle, normalizeTableLevel, personalStyleKey, savePersonalStyle, tableSxFromSpec, resolveRecordTableStyle,
  type CardLevelStyle, type CellStyle, type CellSx, type ConditionalRule, type LayoutMode,
  type RecordTableStyleSpec, type StyleResolver, type TableLevelStyle,
} from '../../utils/recordTableStyle';

/**
 * v2.3.25 记录表样式控制器。
 *
 * 生效优先级：个人视图（本机） → 全局样式（system_settings） → 系统默认。
 * 说明：全局样式写在 `record_table_style_<module>`，保存接口本身要求
 *      `manage:settings` 权限并写审计日志（见 settings_handler.rs）。
 */

export type StyleTarget =
  | { kind: 'body' }
  | { kind: 'header' }
  | { kind: 'column'; column: string }
  | { kind: 'columnHeader'; column: string }
  | { kind: 'row'; rowKey: string };

export interface RecordTableStyleApi {
  spec: RecordTableStyleSpec;
  /** 当前生效来源 */
  source: 'default' | 'global' | 'personal';
  /** v2.3.30：发布全局样式只允许系统管理员 */
  canEditGlobal: boolean;
  saving: boolean;
  message: string;
  messageError: boolean;
  clearMessage: () => void;
  /** 修改样式。首次修改会自动生成个人视图副本，不影响其他人。 */
  updateCellStyle: (target: StyleTarget, patch: CellStyle) => void;
  updateTableStyle: (patch: Partial<TableLevelStyle>) => void;
  /** 操作列展示模式，供记录列表同步按钮布局与列宽。 */
  actionDisplay: TableLevelStyle['actionDisplay'];
  /** v2.3.32：窄屏卡片外框 / 内框颜色与底色 */
  updateCardStyle: (patch: Partial<CardLevelStyle>) => void;
  /** v2.3.32：每页条数；0 表示沿用页面原有默认 */
  setPageSize: (size: number) => void;
  /** v2.3.34：卡片 / 表格布局方式（auto 按设备判定） */
  setLayoutMode: (mode: LayoutMode) => void;
  setRules: (rules: ConditionalRule[]) => void;
  /** 设置某列宽度（px）；传 undefined 表示恢复自动列宽 */
  setColumnWidth: (column: string, width?: number) => void;
  /** 切换列的显示 / 隐藏 */
  toggleColumnHidden: (column: string) => void;
  /** 全部列恢复显示 */
  showAllColumns: () => void;
  /** 放弃个人视图，重新跟随全局 */
  resetPersonal: () => void;
  /** 个人视图整体恢复系统默认 */
  resetAll: () => void;
  /** 把当前样式发布为全局（仅管理员） */
  saveGlobal: (name?: string) => void;
  resolver: StyleResolver;
  tableSx: CellSx;
}

export interface RecordTableStyleOptions {
  /** 页面默认表头底色：吸顶表头需要不透明背景，未显式配置表头填充时使用它 */
  defaultHeaderBg?: string;
}

const emptyRowValues: Record<string, Record<string, string>> = {};

export function useRecordTableStyle(
  moduleKey: string,
  rowValuesByKey: Record<string, Record<string, string>> = emptyRowValues,
  options: RecordTableStyleOptions = {},
): RecordTableStyleApi {
  const { user } = useUser();
  const canEditGlobal = Boolean(user?.is_admin);
  const defaultHeaderBg = options.defaultHeaderBg;
  const settingKey = globalStyleSettingKey(moduleKey);
  const storageKey = useMemo(() => personalStyleKey(moduleKey, user?.id), [moduleKey, user?.id]);

  const [globalSpec, setGlobalSpec] = useState<RecordTableStyleSpec>(DEFAULT_RECORD_TABLE_STYLE);
  const [personalSpec, setPersonalSpec] = useState<RecordTableStyleSpec | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  const [messageError, setMessageError] = useState(false);
  const storageKeyRef = useRef(storageKey);

  // 本机视图随时可能因为切换账号而变化，这里同步读取一次即可。
  useEffect(() => {
    storageKeyRef.current = storageKey;
    setPersonalSpec(loadPersonalStyle(storageKey));
  }, [storageKey]);

  useEffect(() => {
    let cancelled = false;
    getSetting(settingKey)
      .then(response => {
        if (cancelled) return;
        // 没有全局配置时回落到默认对象（同一个引用），React 会跳过这次重渲染。
        setGlobalSpec(response.code === 0 && response.data
          ? normalizeRecordTableStyle(response.data.value)
          : DEFAULT_RECORD_TABLE_STYLE);
      })
      .catch(() => {
        // 尚未配置过全局样式时接口返回 404，按系统默认处理即可。
        if (!cancelled) setGlobalSpec(DEFAULT_RECORD_TABLE_STYLE);
      });
    return () => {
      cancelled = true;
    };
  }, [settingKey]);

  const spec = useMemo(() => resolveRecordTableStyle(globalSpec, personalSpec, canEditGlobal), [globalSpec, personalSpec, canEditGlobal]);
  /**
   * v2.3.33：来源直接由规格推断，不再用「是否加载完成」的额外状态。
   * 否则每次进入记录页都会因为一次无意义的状态更新而把整页（含所有记录行）重渲染一遍。
   */
  const source: RecordTableStyleApi['source'] = personalSpec
    ? 'personal'
    : (globalSpec === DEFAULT_RECORD_TABLE_STYLE ? 'default' : 'global');

  const persistPersonal = useCallback((next: RecordTableStyleSpec) => {
    setPersonalSpec(next);
    savePersonalStyle(storageKeyRef.current, next);
  }, []);

  /** 所有修改都落在个人视图上：管理员要发布到全局需要显式点「发布为全局」。 */
  const commit = useCallback((mutate: (current: RecordTableStyleSpec) => RecordTableStyleSpec) => {
    setPersonalSpec(previous => {
      const base = previous ?? globalSpec;
      const next = mutate(base);
      savePersonalStyle(storageKeyRef.current, next);
      setMessage('已保存到本机个人视图；如需所有用户生效，请点击“发布为全局”');
      setMessageError(false);
      return next;
    });
  }, [globalSpec]);

  const updateCellStyle = useCallback((target: StyleTarget, patch: CellStyle) => {
    commit(current => {
      // 值为 undefined 表示「取消这一项设置」，解析时会自动忽略，等价于回到上层样式。
      const mergeInto = (style: CellStyle): CellStyle => ({ ...style, ...patch });
      switch (target.kind) {
        case 'body':
          return { ...current, body: mergeInto(current.body) };
        case 'header':
          return { ...current, header: mergeInto(current.header) };
        case 'column':
          return { ...current, columns: { ...current.columns, [target.column]: mergeInto(current.columns[target.column] || {}) } };
        case 'columnHeader':
          return { ...current, headerColumns: { ...current.headerColumns, [target.column]: mergeInto(current.headerColumns[target.column] || {}) } };
        case 'row':
          return { ...current, rows: { ...current.rows, [target.rowKey]: mergeInto(current.rows[target.rowKey] || {}) } };
        default:
          return current;
      }
    });
  }, [commit]);

  const updateTableStyle = useCallback((patch: Partial<TableLevelStyle>) => {
    commit(current => ({ ...current, table: normalizeTableLevel({ ...current.table, ...patch }) }));
  }, [commit]);

  const updateCardStyle = useCallback((patch: Partial<CardLevelStyle>) => {
    commit(current => ({ ...current, card: { ...current.card, ...patch } }));
  }, [commit]);

  const setPageSize = useCallback((size: number) => {
    commit(current => ({ ...current, paging: { size: size > 0 ? Math.round(size) : 0 } }));
  }, [commit]);

  const setLayoutMode = useCallback((mode: LayoutMode) => {
    commit(current => ({ ...current, layout: { mode } }));
  }, [commit]);

  const setRules = useCallback((rules: ConditionalRule[]) => {
    commit(current => ({ ...current, rules }));
  }, [commit]);

  /** v2.3.30：列宽直接写进样式规格，拖拽表头分隔线与工具栏输入框共用同一份数据。 */
  const setColumnWidth = useCallback((column: string, width?: number) => {
    commit(current => {
      const nextStyle: CellStyle = { ...(current.columns[column] || {}) };
      if (width && width > 0) nextStyle.width = Math.round(width);
      else delete nextStyle.width;
      return { ...current, columns: { ...current.columns, [column]: nextStyle } };
    });
  }, [commit]);

  const toggleColumnHidden = useCallback((column: string) => {
    commit(current => ({
      ...current,
      hiddenColumns: current.hiddenColumns.includes(column)
        ? current.hiddenColumns.filter(item => item !== column)
        : [...current.hiddenColumns, column],
    }));
  }, [commit]);

  const showAllColumns = useCallback(() => {
    commit(current => ({ ...current, hiddenColumns: [] }));
  }, [commit]);

  const resetPersonal = useCallback(() => {
    clearPersonalStyle(storageKeyRef.current);
    setPersonalSpec(null);
    setMessage('已恢复为跟随全局样式');
    setMessageError(false);
  }, []);

  const resetAll = useCallback(() => {
    clearPersonalStyle(storageKeyRef.current);
    setPersonalSpec(DEFAULT_RECORD_TABLE_STYLE);
    savePersonalStyle(storageKeyRef.current, DEFAULT_RECORD_TABLE_STYLE);
    setMessage('已恢复出厂默认样式');
    setMessageError(false);
  }, []);

  const saveGlobal = useCallback((name?: string) => {
    setSaving(true);
    const payload: RecordTableStyleSpec & { name?: string } = { ...spec };
    if (name) payload.name = name;
    updateSetting(settingKey, payload)
      .then(response => {
        if (response.code !== 0) throw new Error(response.message || '保存失败');
        setGlobalSpec(spec);
        // 发布后本机视图与全局一致，直接放弃本机覆盖，避免"发布完却还显示旧的本机样式"。
        clearPersonalStyle(storageKeyRef.current);
        setPersonalSpec(null);
        setMessage('全局样式已发布，已切换为跟随全局');
        setMessageError(false);
      })
      .catch((error: unknown) => {
        const detail = error instanceof Error ? error.message : String(error);
        setMessage(`全局样式保存失败：${detail}`);
        setMessageError(true);
      })
      .finally(() => setSaving(false));
  }, [settingKey, spec]);

  const resolver = useMemo(() => createStyleResolver(spec, rowValuesByKey), [spec, rowValuesByKey]);
  const tableSx = useMemo(() => tableSxFromSpec(spec, defaultHeaderBg), [spec, defaultHeaderBg]);

  return {
    spec,
    source,
    canEditGlobal,
    saving,
    message,
    messageError,
    clearMessage: () => setMessage(''),
    updateCellStyle,
    updateTableStyle,
    actionDisplay: spec.table.actionDisplay,
    updateCardStyle,
    setPageSize,
    setLayoutMode,
    setRules,
    setColumnWidth,
    toggleColumnHidden,
    showAllColumns,
    resetPersonal,
    resetAll,
    saveGlobal,
    resolver,
    tableSx,
  };
}
