import React, { useState, useEffect, useCallback } from 'react';
import {
  Box, Paper, Typography, Tabs, Tab, Switch, TextField, Button,
  Snackbar, Alert, IconButton, Tooltip, Divider, Table, TableBody,
  TableCell, TableContainer, TableHead, TableRow,
} from '@mui/material';
import SaveIcon from '@mui/icons-material/Save';
import RefreshIcon from '@mui/icons-material/Refresh';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import EditIcon from '@mui/icons-material/Edit';
import { getSetting, updateSetting } from '../api/client';

const R = '2px';

// ───── 导出模板定义 ─────
interface SheetColumnConfig {
  label: string;
  width: number;
}
interface SheetConfig {
  id: string;
  title: string;
  color: string;
  enabled: boolean;
  columns: Record<string, SheetColumnConfig>;
}
interface ExportTemplate {
  file_name: string;
  sheets: Record<string, SheetConfig>;
}

interface TemplateDef {
  key: string;
  name: string;
  defaultSheetIds: string[];
}
const TEMPLATES: TemplateDef[] = [
  { key: 'export_template_workload', name: '分析检测统计', defaultSheetIds: ['sheet1','sheet2','sheet3','sheet4','sheet5','sheet6','sheet7','sheet8','sheet9','sheet10','sheet11'] },
  { key: 'export_template_rd', name: '研发送样统计', defaultSheetIds: ['sheet1','sheet2','sheet3','sheet4','sheet5','sheet6','sheet7','sheet8','sheet9','sheet10','sheet11'] },
  { key: 'export_template_sample_info', name: '样品信息登记', defaultSheetIds: ['detail','by_status','by_type','by_lab','by_project','by_user','by_month'] },
];

const DEFAULT_SHEETS_WORKLOAD: Record<string, SheetConfig> = {
  sheet1: { id:'sheet1', title:'各实验室项目方法对应表', color:'#1976D2', enabled:true, columns:{ lab_name:{label:'使用实验室',width:14}, project_name:{label:'项目代号',width:18}, lc_instrument:{label:'液相仪器',width:18}, method_name:{label:'检测方法',width:30}, coefficient:{label:'单价倍率',width:10}, quantity:{label:'检测数量',width:12}, lc_qty:{label:'液相检测量',width:12}, gc_qty:{label:'气相检测量',width:12}, total_qty:{label:'项目检测总量',width:15} } },
  sheet2: { id:'sheet2', title:'仪器-汇总', color:'#43A047', enabled:true, columns:{ date:{label:'日期',width:12}, instrument:{label:'仪器',width:14}, lab_name:{label:'实验室',width:14}, project_name:{label:'项目',width:20}, method_name:{label:'方法',width:30}, coefficient:{label:'单价倍率',width:10}, quantity:{label:'数量',width:12}, daily_total:{label:'按天数量总计',width:15} } },
  sheet3: { id:'sheet3', title:'项目-汇总', color:'#FF9800', enabled:true, columns:{ project_name:{label:'项目',width:20}, lab_name:{label:'实验室',width:14}, instrument:{label:'仪器',width:14}, method_name:{label:'方法',width:30}, quantity:{label:'数量',width:12}, total_qty:{label:'项目总量',width:15} } },
  sheet4: { id:'sheet4', title:'实验室-汇总', color:'#9C27B0', enabled:true, columns:{ lab_name:{label:'使用实验室',width:14}, project_name:{label:'项目',width:18}, method_name:{label:'方法',width:30}, quantity:{label:'数量',width:12}, person_qty:{label:'人检测量',width:14}, total_qty:{label:'实验室总检测量',width:16} } },
  sheet5: { id:'sheet5', title:'人员-汇总', color:'#E91E63', enabled:true, columns:{ date:{label:'日期',width:12}, project_name:{label:'项目',width:18}, method_name:{label:'方法',width:30}, quantity:{label:'数量',width:12}, user_name:{label:'检测人',width:14} } },
  sheet6: { id:'sheet6', title:'人员汇总表', color:'#00BCD4', enabled:true, columns:{ user_name:{label:'检测人',width:14}, quantity:{label:'数量',width:12}, count:{label:'检测次数',width:12}, instruments:{label:'使用仪器',width:18} } },
  sheet7: { id:'sheet7', title:'实验室总表', color:'#4CAF50', enabled:true, columns:{ lab_name:{label:'实验室',width:14}, project_name:{label:'项目',width:18}, method_name:{label:'方法',width:30}, multiplier:{label:'单价倍率',width:10}, quantity:{label:'数量',width:12}, coefficient:{label:'系数',width:10} } },
  sheet8: { id:'sheet8', title:'项目总表', color:'#FFC107', enabled:true, columns:{ project_name:{label:'项目',width:18}, lab_name:{label:'实验室',width:14}, method_name:{label:'方法',width:30}, multiplier:{label:'单价倍率',width:10}, quantity:{label:'数量',width:12} } },
  sheet9: { id:'sheet9', title:'仪器汇总表', color:'#9E9E9E', enabled:true, columns:{ instrument:{label:'仪器',width:14}, quantity:{label:'检测量',width:12}, total:{label:'总数量',width:12} } },
  sheet10:{ id:'sheet10', title:'理化汇总表', color:'#795548', enabled:true, columns:{ project_name:{label:'项目',width:18}, method_name:{label:'理化方法',width:30}, quantity:{label:'数量',width:12} } },
  sheet11:{ id:'sheet11', title:'事业部汇总', color:'#673AB7', enabled:true, columns:{ division_name:{label:'事业部',width:16}, lab_count:{label:'实验室数',width:10}, total_quantity:{label:'检测数量',width:10}, record_count:{label:'记录数',width:10}, coefficient_score:{label:'系数分',width:10} } },
};

// 研发送样复用分析检测的 sheet 定义
const DEFAULT_SHEETS_RD: Record<string, SheetConfig> = DEFAULT_SHEETS_WORKLOAD;

const DEFAULT_SHEETS_SAMPLE: Record<string, SheetConfig> = {
  detail: { id:'detail', title:'记录明细', color:'#1976D2', enabled:true, columns:{ seq_no:{label:'序号',width:8}, submitted_by:{label:'送样人',width:14}, sample_type:{label:'检测类型',width:14}, project_name:{label:'项目',width:16}, method_name:{label:'方法',width:20}, quantity:{label:'数量',width:10}, created_at:{label:'创建时间',width:16} } },
  by_status: { id:'by_status', title:'按状态', color:'#43A047', enabled:true, columns:{ status:{label:'状态',width:14}, count:{label:'数量',width:10} } },
  by_type: { id:'by_type', title:'按检测类型', color:'#FF9800', enabled:true, columns:{ type_name:{label:'检测类型',width:14}, count:{label:'数量',width:10} } },
  by_lab: { id:'by_lab', title:'按实验室', color:'#9C27B0', enabled:true, columns:{ lab_name:{label:'实验室',width:14}, count:{label:'数量',width:10} } },
  by_project: { id:'by_project', title:'按项目', color:'#4CAF50', enabled:true, columns:{ project_name:{label:'项目',width:16}, count:{label:'数量',width:10} } },
  by_user: { id:'by_user', title:'按送样人', color:'#E91E63', enabled:true, columns:{ user_name:{label:'送样人',width:14}, count:{label:'数量',width:10} } },
  by_month: { id:'by_month', title:'按月份', color:'#00BCD4', enabled:true, columns:{ month:{label:'月份',width:14}, count:{label:'数量',width:10} } },
};

const DEFAULT_TEMPLATES: Record<string, ExportTemplate> = {
  'export_template_workload': { file_name: '样品管理_{s}_{e}', sheets: DEFAULT_SHEETS_WORKLOAD },
  'export_template_rd': { file_name: '研发送样统计_{s}_{e}', sheets: DEFAULT_SHEETS_RD },
  'export_template_sample_info': { file_name: '样品信息登记_{s}_{e}', sheets: DEFAULT_SHEETS_SAMPLE },
};

const hexToRgb = (hex: string): string => {
  const c = hex.replace('#', '');
  const r = parseInt(c.substring(0,2), 16);
  const g = parseInt(c.substring(2,4), 16);
  const b = parseInt(c.substring(4,6), 16);
  return `${r},${g},${b}`;
};

// ───── 组件 ─────
const ManageExportConfig: React.FC = () => {
  const [tab, setTab] = useState(0);
  const [template, setTemplate] = useState<ExportTemplate | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(false);
  const [snack, setSnack] = useState('');
  const [snackErr, setSnackErr] = useState(false);
  const [editSheet, setEditSheet] = useState<string | null>(null); // sheet id being edited

  const templDef = TEMPLATES[tab];
  const key = templDef?.key;

  const load = useCallback(async () => {
    if (!key) return;
    setLoading(true);
    try {
      const r = await getSetting(key);
      if (r.code === 0 && r.data) {
        const parsed = JSON.parse(r.data.value) as ExportTemplate;
        if (parsed && parsed.file_name) {
          setTemplate(parsed);
          setDirty(false);
          setLoading(false);
          return;
        }
      }
    } catch {}
    setTemplate({ file_name: DEFAULT_TEMPLATES[key]?.file_name || '', sheets: { ...DEFAULT_TEMPLATES[key]?.sheets } });
    setDirty(false);
    setLoading(false);
  }, [key]);

  useEffect(() => { load(); }, [load]);
  useEffect(() => { setEditSheet(null); }, [tab]);

  const save = async () => {
    if (!key || !template) return;
    setSaving(true);
    try {
      const r = await updateSetting(key, template);
      if (r.code === 0) {
        setDirty(false);
        setSnack('✅ 导出模板已保存');
        setSnackErr(false);
      } else throw new Error(r.message);
    } catch (e: any) {
      setSnack(`❌ 保存失败: ${e.message || '未知错误'}`);
      setSnackErr(true);
    } finally { setSaving(false); }
  };

  const reset = () => {
    if (!key) return;
    setTemplate({ file_name: DEFAULT_TEMPLATES[key]?.file_name || '', sheets: { ...DEFAULT_TEMPLATES[key]?.sheets } });
    setEditSheet(null);
    setDirty(true);
  };

  const updateFileName = (v: string) => {
    if (!template) return;
    setTemplate({ ...template, file_name: v });
    setDirty(true);
  };

  const toggleSheet = (sid: string) => {
    if (!template) return;
    setTemplate({ ...template, sheets: { ...template.sheets, [sid]: { ...template.sheets[sid], enabled: !template.sheets[sid]?.enabled } } });
    setDirty(true);
  };

  const updateSheetTitle = (sid: string, title: string) => {
    if (!template) return;
    setTemplate({ ...template, sheets: { ...template.sheets, [sid]: { ...template.sheets[sid], title } } });
    setDirty(true);
  };

  const updateSheetColor = (sid: string, color: string) => {
    if (!template) return;
    setTemplate({ ...template, sheets: { ...template.sheets, [sid]: { ...template.sheets[sid], color } } });
    setDirty(true);
  };

  const updateColumnWidth = (sid: string, colKey: string, width: number) => {
    if (!template) return;
    const sheet = template.sheets[sid];
    if (!sheet) return;
    setTemplate({
      ...template,
      sheets: {
        ...template.sheets,
        [sid]: { ...sheet, columns: { ...sheet.columns, [colKey]: { ...sheet.columns[colKey], width } } },
      },
    });
    setDirty(true);
  };

  const updateColumnLabel = (sid: string, colKey: string, label: string) => {
    if (!template) return;
    const sheet = template.sheets[sid];
    if (!sheet) return;
    setTemplate({
      ...template,
      sheets: {
        ...template.sheets,
        [sid]: { ...sheet, columns: { ...sheet.columns, [colKey]: { ...sheet.columns[colKey], label } } },
      },
    });
    setDirty(true);
  };

  const sheets = template?.sheets ? Object.entries(template.sheets) : [];
  const enabledSheets = sheets.filter(([, s]) => s.enabled);

  return (
    <Box>
      <Tabs value={tab} onChange={(_, v) => setTab(v)} sx={{ mb: 2, '& .MuiTab-root': { minWidth: 120, fontSize: '0.85rem' } }}>
        {TEMPLATES.map(t => <Tab key={t.key} label={t.name} />)}
      </Tabs>

      {loading ? (
        <Typography variant="body2" sx={{ color: '#999', p: 2 }}>加载中…</Typography>
      ) : !template ? (
        <Typography variant="body2" sx={{ color: '#999', p: 2 }}>加载失败</Typography>
      ) : (
        <>
          {/* 文件名 */}
          <Paper variant="outlined" sx={{ p: 2, mb: 2, bgcolor: '#fafafa' }}>
            <Typography variant="subtitle2" sx={{ mb: 1, color: '#555' }}>导出文件命名</Typography>
            <TextField size="small" value={template.file_name} onChange={e => updateFileName(e.target.value)}
              sx={{ width: 350, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.85rem' } }}
              placeholder="文件名模板" helperText="可用占位符: {s}=开始日期 {e}=结束日期" />
          </Paper>

          {/* Sheet 列表 */}
          <Typography variant="subtitle2" sx={{ mb: 1, color: '#555' }}>
            Sheet 列表（共 {sheets.length} 张，启用 {enabledSheets.length} 张）
          </Typography>

          <Paper variant="outlined" sx={{ p: 1, mb: 2 }}>
            {sheets.map(([sid, sheet]) => (
              <Box key={sid} sx={{
                display: 'flex', alignItems: 'center', gap: 1, py: 0.8, px: 1,
                borderBottom: '1px solid #eee', flexWrap: 'wrap',
              }}>
                <Switch size="small" checked={sheet.enabled} onChange={() => toggleSheet(sid)} />
                <Box sx={{ width: 14, height: 14, borderRadius: '2px', bgcolor: sheet.color || '#ccc', flexShrink: 0 }} />
                <TextField size="small" value={sheet.title} onChange={e => updateSheetTitle(sid, e.target.value)}
                  sx={{ width: 220, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                  inputProps={{ style: { padding: '2px 6px' } }} disabled={!sheet.enabled} />
                <input type="color" value={sheet.color || '#1976D2'}
                  onChange={e => updateSheetColor(sid, e.target.value)}
                  style={{ width: 32, height: 28, border: 'none', cursor: 'pointer', padding: 0 }} />
                <Button size="small" onClick={() => setEditSheet(editSheet === sid ? null : sid)} disabled={!sheet.enabled}
                  startIcon={<EditIcon />} sx={{ borderRadius: R, fontSize: '0.75rem', ml: 'auto' }}>
                  {editSheet === sid ? '收起列' : '编辑列'}
                </Button>
              </Box>
            ))}
          </Paper>

          {/* 展开的列编辑 */}
          {editSheet && template.sheets[editSheet] && (() => {
            const sheet = template.sheets[editSheet];
            return (
              <Paper variant="outlined" sx={{ p: 2, mb: 2, bgcolor: '#f5f5f5' }}>
                <Box sx={{ display: 'flex', alignItems: 'center', mb: 1.5 }}>
                  <IconButton size="small" onClick={() => setEditSheet(null)} sx={{ mr: 1 }}><ChevronLeftIcon /></IconButton>
                  <Typography variant="subtitle2">{sheet.title} - 列配置</Typography>
                </Box>
                {Object.entries(sheet.columns).map(([colKey, col]) => (
                  <Box key={colKey} sx={{ display: 'flex', alignItems: 'center', gap: 1, py: 0.5, flexWrap: 'wrap' }}>
                    <Typography variant="caption" sx={{ minWidth: 80, fontWeight: 600, fontSize: '0.75rem', color: '#666' }}>
                      {colKey}
                    </Typography>
                    <TextField size="small" value={col.label} onChange={e => updateColumnLabel(editSheet, colKey, e.target.value)}
                      sx={{ width: 140, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                      inputProps={{ style: { padding: '2px 6px' } }} placeholder="标签" />
                    <TextField size="small" type="number" value={col.width}
                      onChange={e => updateColumnWidth(editSheet, colKey, Number(e.target.value) || 5)}
                      sx={{ width: 70, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                      inputProps={{ min: 3, max: 60, style: { padding: '2px 6px', textAlign: 'center' } }} placeholder="宽" />
                  </Box>
                ))}
              </Paper>
            );
          })()}

          {/* 操作按钮 */}
          <Box sx={{ display: 'flex', gap: 1, mb: 3 }}>
            <Button variant="contained" size="small" startIcon={<SaveIcon />} onClick={save}
              disabled={!dirty || saving} sx={{ borderRadius: R, fontSize: '0.8rem' }}>
              {saving ? '保存中…' : '保存导出模板'}
            </Button>
            <Button variant="outlined" size="small" startIcon={<RefreshIcon />} onClick={reset}
              sx={{ borderRadius: R, fontSize: '0.8rem' }} color="warning">重置默认</Button>
          </Box>

          {/* 预览 */}
          <Divider sx={{ mb: 2 }} />
          <Typography variant="subtitle2" sx={{ mb: 1, color: '#555' }}>导出 Sheet 清单预览</Typography>
          <Paper variant="outlined" sx={{ overflow: 'auto' }}>
            <TableContainer>
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell sx={{ fontWeight: 600, fontSize: '0.75rem' }}>序号</TableCell>
                    <TableCell sx={{ fontWeight: 600, fontSize: '0.75rem' }}>启用</TableCell>
                    <TableCell sx={{ fontWeight: 600, fontSize: '0.75rem' }}>Sheet 名称</TableCell>
                    <TableCell sx={{ fontWeight: 600, fontSize: '0.75rem' }}>颜色</TableCell>
                    <TableCell sx={{ fontWeight: 600, fontSize: '0.75rem' }}>列数</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {enabledSheets.map(([sid, sheet], idx) => (
                    <TableRow key={sid}>
                      <TableCell sx={{ fontSize: '0.75rem' }}>{idx + 1}</TableCell>
                      <TableCell><Switch size="small" checked={sheet.enabled} disabled /></TableCell>
                      <TableCell sx={{ fontSize: '0.8rem' }}>{sheet.title}</TableCell>
                      <TableCell>
                        <Box sx={{ width: 18, height: 18, borderRadius: '2px', bgcolor: sheet.color || '#ccc' }} />
                      </TableCell>
                      <TableCell sx={{ fontSize: '0.75rem' }}>{Object.keys(sheet.columns).length} 列</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </Paper>
        </>
      )}

      <Snackbar open={!!snack} autoHideDuration={4000} onClose={() => setSnack('')}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snackErr ? 'error' : 'success'} onClose={() => setSnack('')}
          variant="filled" sx={{ borderRadius: R }}>
          {snack}
        </Alert>
      </Snackbar>
    </Box>
  );
};

export default ManageExportConfig;
