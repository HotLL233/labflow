import { useEffect, useState } from 'react';
import { getSetting } from '../api/client';
import type { TableConfig } from '../types/layout';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';

const normalize = (value: unknown): TableConfig => {
  const source = value && typeof value === 'object' ? value as Record<string, unknown> : {};
  return {
    row_height: Math.max(28, Number(source.row_height) || DEFAULT_TABLE_CONFIG.row_height),
    seq_column_width: Math.max(32, Number(source.seq_column_width) || DEFAULT_TABLE_CONFIG.seq_column_width),
    checkbox_column_width: Math.max(28, Number(source.checkbox_column_width) || DEFAULT_TABLE_CONFIG.checkbox_column_width),
  };
};

/** 记录表与后台配置共用的表格配置读取入口。 */
export function useTableConfig(settingKey = 'form_sample_entry') {
  const [config, setConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });

  useEffect(() => {
    let disposed = false;
    setConfig({ ...DEFAULT_TABLE_CONFIG });
    getSetting(settingKey).then(response => {
      if (disposed || response.code !== 0 || !response.data?.value) return;
      try {
        const parsed = JSON.parse(response.data.value) as { table_config?: unknown };
        if (!disposed) setConfig(normalize(parsed.table_config));
      } catch {
        // 配置损坏时保留默认值，不能阻塞记录表。
      }
    }).catch(() => {});
    return () => { disposed = true; };
  }, [settingKey]);

  return config;
}
