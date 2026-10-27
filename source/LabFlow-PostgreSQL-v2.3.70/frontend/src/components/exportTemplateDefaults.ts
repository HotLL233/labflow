export interface SheetColumnConfig {
  label: string;
  width: number;
  // Missing in legacy templates means visible, preserving every historical export.
  visible?: boolean;
}

export interface SheetConfig {
  id: string;
  title: string;
  color: string;
  enabled: boolean;
  columns: Record<string, SheetColumnConfig>;
}

export interface ExportTemplate {
  file_name: string;
  sheets: Record<string, SheetConfig>;
}

export interface TemplateDef {
  key: string;
  name: string;
  defaultSheetIds: string[];
}

export const TEMPLATES: TemplateDef[] = [
  { key: 'export_template_workload', name: '分析检测统计', defaultSheetIds: ['sheet1', 'sheet2', 'sheet3', 'sheet4', 'sheet5', 'sheet6', 'sheet7', 'sheet8', 'sheet9', 'sheet10', 'sheet11', 'sheet12', 'sheet13'] },
  { key: 'export_template_rd', name: '研发送样统计', defaultSheetIds: ['sheet1', 'sheet2', 'sheet3', 'sheet4', 'sheet5', 'sheet6', 'sheet7', 'sheet8', 'sheet9', 'sheet10', 'sheet11'] },
  { key: 'export_template_sample_info', name: '样品信息登记', defaultSheetIds: ['detail', 'by_status', 'by_type', 'by_lab', 'by_project', 'by_user', 'by_month'] },
];

export const DEFAULT_SHEETS_WORKLOAD: Record<string, SheetConfig> = {
  sheet1: { id: 'sheet1', title: '各实验室项目方法对应表', color: '#1976D2', enabled: true, columns: { lab_name: { label: '使用实验室', width: 14 }, project_name: { label: '项目代号', width: 18 }, instrument: { label: '仪器', width: 18 }, method_name: { label: '检测方法', width: 30 }, method_type: { label: '检测类型', width: 14 }, quantity: { label: '检测数量', width: 12 }, project_total: { label: '项目检测总量', width: 15 }, high_item: { label: '高项', width: 12 } } },
  sheet2: { id: 'sheet2', title: '仪器-汇总', color: '#43A047', enabled: true, columns: { date: { label: '日期', width: 12 }, instrument: { label: '仪器', width: 14 }, lab_name: { label: '实验室', width: 14 }, project_name: { label: '项目', width: 20 }, high_item: { label: '高项', width: 12 }, method_name: { label: '方法', width: 30 }, quantity: { label: '数量', width: 12 }, daily_total: { label: '按天数量总计', width: 15 } } },
  sheet3: { id: 'sheet3', title: '项目-汇总', color: '#FF9800', enabled: true, columns: { project_name: { label: '项目', width: 20 }, high_item: { label: '高项', width: 12 }, lab_name: { label: '实验室', width: 14 }, instrument: { label: '仪器', width: 14 }, method_name: { label: '方法', width: 30 }, multiplier: { label: '单价倍率', width: 10 }, quantity: { label: '数量', width: 12 }, unit_price: { label: '单价', width: 12 }, detail_amount: { label: '明细金额', width: 15 }, project_total: { label: '项目金额汇总', width: 15 } } },
  sheet4: { id: 'sheet4', title: '实验室-汇总', color: '#9C27B0', enabled: true, columns: { lab_name: { label: '使用实验室', width: 14 }, project_name: { label: '项目', width: 18 }, high_item: { label: '高项', width: 12 }, instrument: { label: '仪器', width: 14 }, method_name: { label: '方法', width: 30 }, multiplier: { label: '单价倍率', width: 10 }, quantity: { label: '数量', width: 12 }, unit_price: { label: '单价', width: 12 }, total_qty: { label: '数量总计', width: 12 }, detail_amount: { label: '明细金额', width: 15 }, lab_total: { label: '实验室金额汇总', width: 15 } } },
  sheet5: { id: 'sheet5', title: '人员记录明细', color: '#E91E63', enabled: true, columns: { recorded_at: { label: '录入时间', width: 16 }, detection_division: { label: '检测部门', width: 14 }, sending_division: { label: '送样部门', width: 14 }, lab_name: { label: '送样实验室', width: 14 }, project_name: { label: '研发项目', width: 20 }, high_item: { label: '高项', width: 12 }, method_name: { label: '方法', width: 30 }, method_type: { label: '检测类型', width: 14 }, quantity: { label: '数量', width: 10 }, user_name: { label: '检测人', width: 14 } } },
  sheet6: { id: 'sheet6', title: '人员工作量明细', color: '#00BCD4', enabled: true, columns: { user_name: { label: '检测人', width: 14 }, instrument: { label: '仪器', width: 24 }, method_type: { label: '类型', width: 16 }, method: { label: '方法', width: 30 }, coefficient: { label: '系数', width: 10 }, quantity: { label: '数量', width: 10 }, workload: { label: '工作量明细', width: 16 }, total_workload: { label: '人员工作量汇总', width: 18 } } },
  sheet7: { id: 'sheet7', title: '实验室总表', color: '#4CAF50', enabled: true, columns: { lab_name: { label: '实验室', width: 14 }, project_name: { label: '项目', width: 20 }, method_type: { label: '检测类型', width: 16 }, quantity: { label: '数量', width: 12 }, unit_price: { label: '单价', width: 12 }, multiplier: { label: '单价倍率', width: 10 }, detail_amount: { label: '明细金额', width: 15 }, project_total: { label: '项目金额汇总', width: 16 }, lab_total: { label: '实验室金额汇总', width: 16 } } },
  sheet8: { id: 'sheet8', title: '项目总表', color: '#FFC107', enabled: true, columns: { project_name: { label: '项目', width: 20 }, method_type: { label: '检测类型', width: 16 }, quantity: { label: '数量', width: 12 }, unit_price: { label: '单价', width: 12 }, multiplier: { label: '单价倍率', width: 10 }, detail_amount: { label: '明细金额', width: 15 }, project_total: { label: '项目金额汇总', width: 16 } } },
  sheet9: { id: 'sheet9', title: '仪器汇总表', color: '#9E9E9E', enabled: true, columns: { instrument: { label: '仪器编号', width: 16 }, quantity: { label: '检测量', width: 12 }, instrument_type: { label: '类型', width: 12 }, type_total: { label: '按类型汇总', width: 15 } } },
  sheet10: { id: 'sheet10', title: '理化汇总表', color: '#795548', enabled: true, columns: { method_name: { label: '方法名', width: 40 }, quantity: { label: '数量', width: 12 } } },
  sheet11: { id: 'sheet11', title: '事业部汇总', color: '#673AB7', enabled: true, columns: { detection_division: { label: '检测部门', width: 16 }, sending_division: { label: '送样部门', width: 16 }, lab_count: { label: '实验室数', width: 10 }, total_quantity: { label: '检测数量', width: 10 }, record_count: { label: '记录数', width: 10 }, coefficient_score: { label: '系数分', width: 10 } } },
  sheet12: { id: 'sheet12', title: '辅助工作明细汇总', color: '#FF9800', enabled: true, columns: { user_name: { label: '检测人', width: 14 }, auxiliary_method: { label: '辅助工作名称', width: 28 }, coefficient: { label: '系数', width: 10 }, quantity: { label: '数量', width: 10 }, workload: { label: '明细工作量', width: 16 }, total_workload: { label: '辅助工作量汇总', width: 18 } } },
  sheet13: { id: 'sheet13', title: '人员工作量汇总', color: '#3F51B5', enabled: true, columns: { user_name: { label: '检测人', width: 16 }, total_workload: { label: '总工作量', width: 14 }, method_type: { label: '检测类型', width: 18 }, coefficient: { label: '系数', width: 10 }, quantity: { label: '数量', width: 10 }, workload: { label: '工作量', width: 14 } } },
};

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

export const DEFAULT_SHEETS_RD: Record<string, SheetConfig> = clone(DEFAULT_SHEETS_WORKLOAD);
delete DEFAULT_SHEETS_RD.sheet13;
delete DEFAULT_SHEETS_RD.sheet12;
DEFAULT_SHEETS_RD.sheet5 = { id: 'sheet5', title: '送样人记录明细', color: '#E91E63', enabled: true, columns: { recorded_at: { label: '录入时间', width: 16 }, lab_name: { label: '实验室', width: 14 }, project_name: { label: '研发项目', width: 20 }, high_item: { label: '高项', width: 12 }, method_name: { label: '方法', width: 30 }, method_type: { label: '检测类型', width: 14 }, quantity: { label: '数量', width: 10 }, user_name: { label: '送样人', width: 14 } } };
// Sheet6 has dynamic type columns. It follows current master data and only
// exposes workbook-level options here, avoiding a misleading fixed-column UI.
DEFAULT_SHEETS_RD.sheet6 = { id: 'sheet6', title: '送样人汇总表', color: '#00BCD4', enabled: true, columns: {} };
DEFAULT_SHEETS_RD.sheet11 = { id: 'sheet11', title: '检测类型汇总', color: '#00BCD4', enabled: true, columns: { method_type: { label: '检测类型', width: 12 }, quantity: { label: '数量', width: 12 }, unit_price: { label: '单价', width: 12 }, detail_amount: { label: '明细金额', width: 15 }, type_total: { label: '类型金额汇总', width: 15 } } };

export const DEFAULT_SHEETS_SAMPLE: Record<string, SheetConfig> = {
  detail: { id: 'detail', title: '记录明细', color: '#1976D2', enabled: true, columns: { seq_no: { label: '序号', width: 8 }, user_name: { label: '送样人', width: 14 }, lab_name: { label: '实验室', width: 14 }, project_name: { label: '项目', width: 16 }, submitted_at: { label: '送样日期', width: 16 }, detection_type: { label: '检测类型', width: 16 }, status: { label: '状态', width: 12 } } },
  by_status: { id: 'by_status', title: '按状态', color: '#43A047', enabled: true, columns: { status: { label: '状态', width: 14 }, count: { label: '数量', width: 10 } } },
  by_type: { id: 'by_type', title: '按检测类型', color: '#FF9800', enabled: true, columns: { type_name: { label: '检测类型', width: 14 }, type_key: { label: '类型标识', width: 18 }, count: { label: '数量', width: 10 } } },
  by_lab: { id: 'by_lab', title: '按实验室', color: '#9C27B0', enabled: true, columns: { lab_name: { label: '实验室', width: 14 }, count: { label: '数量', width: 10 } } },
  by_project: { id: 'by_project', title: '按项目', color: '#4CAF50', enabled: true, columns: { project_name: { label: '项目', width: 16 }, count: { label: '数量', width: 10 } } },
  by_user: { id: 'by_user', title: '按送样人', color: '#E91E63', enabled: true, columns: { user_name: { label: '送样人', width: 14 }, count: { label: '数量', width: 10 } } },
  by_month: { id: 'by_month', title: '按月份', color: '#00BCD4', enabled: true, columns: { month: { label: '月份', width: 14 }, count: { label: '数量', width: 10 } } },
};

export const DEFAULT_TEMPLATES: Record<string, ExportTemplate> = {
  export_template_workload: { file_name: '样品管理_{s}_{e}', sheets: DEFAULT_SHEETS_WORKLOAD },
  export_template_rd: { file_name: '研发送样统计_{s}_{e}', sheets: DEFAULT_SHEETS_RD },
  export_template_sample_info: { file_name: '样品信息登记_{s}_{e}', sheets: DEFAULT_SHEETS_SAMPLE },
};

export const defaultTemplateFor = (key: string): ExportTemplate =>
  clone(DEFAULT_TEMPLATES[key] || { file_name: '', sheets: {} });

// Templates saved before 2.2.18 used several retired export column keys.
// Keep only presentation settings that still map to a physical export column.
export const normalizeTemplate = (key: string, saved: unknown): ExportTemplate => {
  const base = defaultTemplateFor(key);
  if (!saved || typeof saved !== 'object') return base;

  const source = saved as Partial<ExportTemplate>;
  if (typeof source.file_name === 'string' && source.file_name.trim()) {
    base.file_name = source.file_name;
  }
  if (!source.sheets || typeof source.sheets !== 'object') return base;

  Object.entries(base.sheets).forEach(([sheetId, defaultSheet]) => {
    const oldSheet = source.sheets?.[sheetId];
    if (!oldSheet || typeof oldSheet !== 'object') return;
    const sheet = oldSheet as Partial<SheetConfig>;
    if (typeof sheet.title === 'string' && sheet.title.trim()) defaultSheet.title = sheet.title;
    if (typeof sheet.color === 'string' && /^#[0-9a-f]{6}$/i.test(sheet.color)) defaultSheet.color = sheet.color;
    if (typeof sheet.enabled === 'boolean') defaultSheet.enabled = sheet.enabled;

    Object.entries(defaultSheet.columns).forEach(([columnKey, defaultColumn]) => {
      const oldColumn = sheet.columns?.[columnKey];
      if (!oldColumn || typeof oldColumn !== 'object') return;
      if (typeof oldColumn.label === 'string' && oldColumn.label.trim()) defaultColumn.label = oldColumn.label;
      if (typeof oldColumn.width === 'number' && Number.isFinite(oldColumn.width)) {
        defaultColumn.width = Math.min(60, Math.max(3, oldColumn.width));
      }
      if (typeof oldColumn.visible === 'boolean') defaultColumn.visible = oldColumn.visible;
    });
  });
  return base;
};
