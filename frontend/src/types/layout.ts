/**
 * v0.4.36: 可视化页面布局编辑器类型定义
 */

/** 字段定义（对应 system_settings 中 layout_* 的 JSON 数组元素） */
export interface FieldDef {
  key: string;
  type: 'text' | 'select' | 'textarea' | 'number' | 'date' | 'datetime' | 'divider' | 'heading';
  label: string;
  width: number;
  required: boolean;
  visible: boolean;
  sort_order: number;
  placeholder?: string;
  options?: string; // select 类型时可选
}

/** 布局配置（存储在 system_settings 中 value 为 JSON 数组） */
export interface PageLayout {
  fields: FieldDef[];
}

/** 拖拽类型 */
export type DragSourceType = 'library' | 'field';

/** 拖拽项 */
export interface DragItem {
  type: DragSourceType;
  fieldKey?: string;
  fieldType?: FieldDef['type'];
  fromIndex?: number;
}

// v0.4.50: 表格全局配置
export interface TableConfig {
  row_height: number;
  seq_column_width: number;
  checkbox_column_width: number;
}

export const DEFAULT_TABLE_CONFIG: TableConfig = {
  row_height: 48,
  seq_column_width: 50,
  checkbox_column_width: 36,
};

/** v0.4.50: 统一表单布局格式（兼容旧版数组格式） */
export interface FormLayout {
  table_config: TableConfig;
  fields: FieldDef[];
}
