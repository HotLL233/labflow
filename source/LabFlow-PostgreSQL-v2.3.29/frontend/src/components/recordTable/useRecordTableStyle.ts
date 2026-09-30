import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getSetting, updateSetting } from '../../api/admin';
import { hasPermission } from '../../constants/permissions';
import { useUser } from '../../UserContext';
import {
  DEFAULT_RECORD_TABLE_STYLE, clearPersonalStyle, createStyleResolver, globalStyleSettingKey,
  loadPersonalStyle, normalizeRecordTableStyle, personalStyleKey, savePersonalStyle, tableSxFromSpec,
  type CellStyle, type CellSx, type ConditionalRule, type RecordTableStyleSpec, type StyleResolver,
  type TableLevelStyle,
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
  canEditGlobal: boolean;
  saving: boolean;
  message: string;
  messageError: boolean;
  clearMessage: () => void;
  /** 修改样式。首次修改会自动生成个人视图副本，不影响其他人。 */
  updateCellStyle: (target: StyleTarget, patch: CellStyle) => void;
  updateTableStyle: (patch: Partial<TableLevelStyle>) => void;
  setRules: (rules: ConditionalRule[]) => void;
  /** 放弃个人视图，重新跟随全局 */
  resetPersonal: () => void;
  /** 个人视图整体恢复系统默认 */
  resetAll: () => void;
  /** 把当前样式发布为全局（需 manage:settings） */
  saveGlobal: (name?: string) => void;
  resolver: StyleResolver;
  tableSx: CellSx;
}

const emptyRowValues: Record<string, Record<string, string>> = {};

export function useRecordTableStyle(
  moduleKey: string,
  rowValuesByKey: Record<string, Record<string, string>> = emptyRowValues,
): RecordTableStyleApi {
  const { user } = useUser();
  const canEditGlobal = Boolean(user?.is_admin || hasPermission(user?.permissions || [], 'manage:settings'));
  const settingKey = globalStyleSettingKey(moduleKey);
  const storageKey = useMemo(() => personalStyleKey(moduleKey, user?.id), [moduleKey, user?.id]);

  const [globalSpec, setGlobalSpec] = useState<RecordTableStyleSpec>(DEFAULT_RECORD_TABLE_STYLE);
  const [personalSpec, setPersonalSpec] = useState<RecordTableStyleSpec | null>(null);
  const [globalLoaded, setGlobalLoaded] = useState(false);
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
    setGlobalLoaded(false);
    getSetting(settingKey)
      .then(response => {
        if (cancelled) return;
        if (response.code === 0 && response.data) {
          setGlobalSpec(normalizeRecordTableStyle(response.data.value));
        } else {
          setGlobalSpec(DEFAULT_RECORD_TABLE_STYLE);
        }
      })
      .catch(() => {
        // 尚未配置过全局样式时接口返回 404，按系统默认处理即可。
        if (!cancelled) setGlobalSpec(DEFAULT_RECORD_TABLE_STYLE);
      })
      .finally(() => {
        if (!cancelled) setGlobalLoaded(true);
      });
    return () => {
      cancelled = true;
    };
  }, [settingKey]);

  const spec = personalSpec ?? globalSpec;
  const source: RecordTableStyleApi['source'] = personalSpec ? 'personal' : (globalLoaded ? 'global' : 'default');

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
    commit(current => ({ ...current, table: { ...current.table, ...patch } }));
  }, [commit]);

  const setRules = useCallback((rules: ConditionalRule[]) => {
    commit(current => ({ ...current, rules }));
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
  const tableSx = useMemo(() => tableSxFromSpec(spec), [spec]);

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
    setRules,
    resetPersonal,
    resetAll,
    saveGlobal,
    resolver,
    tableSx,
  };
}
