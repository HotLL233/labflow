import React, { useState, useEffect, useCallback } from 'react';
import {
  Box, Paper, Typography, Tabs, Tab, Switch, TextField, Button,
  Snackbar, Alert, IconButton, Tooltip, Divider, Table, TableBody,
  TableCell, TableContainer, TableHead, TableRow, Select, MenuItem,
} from '@mui/material';
import SaveIcon from '@mui/icons-material/Save';
import RefreshIcon from '@mui/icons-material/Refresh';
import DeleteIcon from '@mui/icons-material/Delete';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import AddIcon from '@mui/icons-material/Add';
import type { FieldDef, TableConfig, FormLayout } from '../types/layout';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import { getSetting, updateSetting } from '../api/client';

const R = '2px';

// ───── 表单模板定义 ─────
interface FormTemplate {
  key: string;          // system_settings key
  name: string;         // 显示名
  defaultFields: FieldDef[];
}
const FORM_TEMPLATES: FormTemplate[] = [
  {
    key: 'form_sample_entry',
    name: '研发送样录入',
    defaultFields: [
      { key: 'user_name', type: 'text', label: '送样人', width: 120, required: true, visible: true, sort_order: 1, placeholder: '' },
      { key: 'division_id', type: 'select', label: '部门', width: 140, required: false, visible: true, sort_order: 2, options: '从用户分组读取' },
      { key: 'lab_name', type: 'text', label: '实验室', width: 150, required: false, visible: true, sort_order: 3, placeholder: '' },
      { key: 'project_name', type: 'text', label: '项目', width: 160, required: false, visible: true, sort_order: 4, placeholder: '' },
      { key: 'detection_type', type: 'select', label: '检测类型', width: 120, required: false, visible: true, sort_order: 5, options: '从检测类型表读取' },
      { key: 'method_name', type: 'text', label: '方法', width: 200, required: false, visible: true, sort_order: 6, placeholder: '' },
      { key: 'quantity', type: 'number', label: '数量', width: 80, required: false, visible: true, sort_order: 7 },
      { key: 'batch_no', type: 'text', label: '批号', width: 100, required: false, visible: true, sort_order: 8, placeholder: '' },
      { key: 'notes', type: 'text', label: '注意事项', width: 150, required: false, visible: true, sort_order: 9, placeholder: '' },
    ],
  },
  {
    key: 'form_sample_info_entry',
    name: '样品信息登记',
    defaultFields: [
      { key: 'sample_time', type: 'datetime', label: '送样时间', width: 160, required: false, visible: true, sort_order: 1 },
      { key: 'submitted_by', type: 'text', label: '送样人', width: 120, required: true, visible: true, sort_order: 2, placeholder: '' },
      { key: 'department', type: 'text', label: '部门', width: 140, required: false, visible: true, sort_order: 3, placeholder: '' },
      { key: 'sample_type', type: 'select', label: '检测类型', width: 120, required: true, visible: true, sort_order: 4, options: '从检测类型表读取' },
      { key: 'project_name', type: 'text', label: '项目名称', width: 160, required: false, visible: true, sort_order: 5, placeholder: '' },
      { key: 'method_name', type: 'text', label: '检测方法', width: 200, required: false, visible: true, sort_order: 6, placeholder: '' },
      { key: 'sample_name', type: 'text', label: '样品名称', width: 150, required: false, visible: true, sort_order: 7, placeholder: '' },
      { key: 'sample_count', type: 'number', label: '样品数', width: 80, required: false, visible: true, sort_order: 8 },
      { key: 'notes', type: 'text', label: '备注', width: 150, required: false, visible: true, sort_order: 9, placeholder: '' },
    ],
  },
  {
    key: 'form_entry',
    name: '分析检测录入',
    defaultFields: [
      { key: 'user_name', type: 'text', label: '检测人', width: 120, required: true, visible: true, sort_order: 1, placeholder: '' },
      { key: 'group_id', type: 'select', label: '实验室', width: 150, required: true, visible: true, sort_order: 2, options: '实验室列表' },
      { key: 'project_id', type: 'select', label: '项目', width: 160, required: false, visible: true, sort_order: 3, options: '项目列表' },
      { key: 'method_id', type: 'select', label: '方法', width: 200, required: false, visible: true, sort_order: 4, options: '方法列表' },
      { key: 'quantity', type: 'number', label: '数量', width: 80, required: false, visible: true, sort_order: 5 },
      { key: 'recorded_at', type: 'datetime', label: '检测时间', width: 160, required: false, visible: true, sort_order: 6 },
      { key: 'notes', type: 'text', label: '备注', width: 150, required: false, visible: true, sort_order: 7, placeholder: '' },
    ],
  },
];

const FIELD_TYPES: string[] = ['text', 'number', 'select', 'datetime', 'textarea'];

// ───── 组件 ─────
const ManageFormConfig: React.FC = () => {
  const [tab, setTab] = useState(0);
  const [fields, setFields] = useState<FieldDef[]>([]);
  const [tableConfig, setTableConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(false);
  const [snack, setSnack] = useState('');
  const [snackErr, setSnackErr] = useState(false);

  const template = FORM_TEMPLATES[tab];
  const key = template?.key;

  // 加载（兼容新旧格式）
  const load = useCallback(async (k: string) => {
    setLoading(true);
    try {
      const r = await getSetting(k);
      if (r.code === 0 && r.data) {
        const parsed = JSON.parse(r.data.value);
        // 新格式（v0.4.50+）：{ table_config, fields }
        if (!Array.isArray(parsed) && parsed.fields) {
          setFields((parsed.fields as FieldDef[]).sort((a, b) => (a.sort_order ?? 99) - (b.sort_order ?? 99)));
          setTableConfig({ ...DEFAULT_TABLE_CONFIG, ...parsed.table_config });
          setDirty(false);
          setLoading(false);
          return;
        }
        // 旧格式（v0.4.49-）：FieldDef[]
        if (Array.isArray(parsed) && parsed.length > 0) {
          setFields(parsed.sort((a, b) => (a.sort_order ?? 99) - (b.sort_order ?? 99)));
          setTableConfig({ ...DEFAULT_TABLE_CONFIG });
          setDirty(false);
          setLoading(false);
          return;
        }
      }
    } catch {}
    // 默认值
    setFields(template.defaultFields);
    setTableConfig({ ...DEFAULT_TABLE_CONFIG });
    setDirty(false);
    setLoading(false);
  }, []);

  useEffect(() => {
    if (key) load(key);
  }, [key, load]);

  // 切换 tab
  const handleTab = (_: any, v: number) => { setTab(v); };

  // 字段编辑
  const updateField = (idx: number, patch: Partial<FieldDef>) => {
    setFields(prev => {
      const next = [...prev];
      next[idx] = { ...next[idx], ...patch };
      return next;
    });
    setDirty(true);
  };

  const removeField = (idx: number) => {
    setFields(prev => prev.filter((_, i) => i !== idx));
    setDirty(true);
  };

  const moveField = (from: number, to: number) => {
    if (to < 0 || to >= fields.length) return;
    setFields(prev => {
      const next = [...prev];
      const [item] = next.splice(from, 1);
      next.splice(to, 0, item);
      return next;
    });
    setDirty(true);
  };

  const addField = () => {
    const maxOrder = fields.reduce((m, f) => Math.max(m, f.sort_order ?? 0), 0);
    setFields(prev => [...prev, {
      key: `field_${Date.now()}`,
      type: 'text',
      label: '新字段',
      width: 120,
      required: false,
      visible: true,
      sort_order: maxOrder + 1,
      placeholder: '',
    }]);
    setDirty(true);
  };

  const save = async () => {
    if (!key) return;
    setSaving(true);
    try {
      // 重新编号 sort_order
      const sorted = fields.map((f, i) => ({ ...f, sort_order: i + 1 }));
      const formLayout: FormLayout = { table_config: tableConfig, fields: sorted };
      const r = await updateSetting(key, formLayout);
      if (r.code === 0) {
        setFields(sorted);
        setDirty(false);
        setSnack('✅ 字段布局已保存');
        setSnackErr(false);
      } else {
        throw new Error(r.message);
      }
    } catch (e: any) {
      setSnack(`❌ 保存失败: ${e.message || '未知错误'}`);
      setSnackErr(true);
    } finally {
      setSaving(false);
    }
  };

  const reset = () => {
    if (!key) return;
    setFields(template.defaultFields);
    setTableConfig({ ...DEFAULT_TABLE_CONFIG });
    setDirty(true);
  };

  const updateTableConfig = (patch: Partial<TableConfig>) => {
    setTableConfig(prev => ({ ...prev, ...patch }));
    setDirty(true);
  };

  // 预览表头
  const visibleFields = fields.filter(f => f.visible !== false).sort((a, b) => (a.sort_order ?? 99) - (b.sort_order ?? 99));
  const totalWidth = visibleFields.reduce((s, f) => s + (f.width || 100), 0) + tableConfig.seq_column_width + tableConfig.checkbox_column_width;

  return (
    <Box>
      <Tabs value={tab} onChange={handleTab} sx={{ mb: 2, '& .MuiTab-root': { minWidth: 120, fontSize: '0.85rem' } }}>
        {FORM_TEMPLATES.map((t, i) => (
          <Tab key={t.key} label={t.name} />
        ))}
      </Tabs>

      {loading ? (
        <Typography variant="body2" sx={{ color: '#999', p: 2 }}>加载中…</Typography>
      ) : (
        <>
          {/* v0.4.50: 表格全局设置 */}
          <Paper variant="outlined" sx={{ p: 2, mb: 2, bgcolor: '#fafafa' }}>
            <Typography variant="subtitle2" sx={{ mb: 1, color: '#555' }}>表格全局设置</Typography>
            <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}>
              <TextField size="small" type="number" label="行高" value={tableConfig.row_height}
                onChange={e => updateTableConfig({ row_height: Number(e.target.value) || 48 })}
                sx={{ width: 100, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                inputProps={{ min: 24, max: 120, style: { textAlign: 'center' } }} />
              <TextField size="small" type="number" label="序号列宽" value={tableConfig.seq_column_width}
                onChange={e => updateTableConfig({ seq_column_width: Number(e.target.value) || 50 })}
                sx={{ width: 110, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                inputProps={{ min: 20, max: 200, style: { textAlign: 'center' } }} />
              <TextField size="small" type="number" label="复选框列宽" value={tableConfig.checkbox_column_width}
                onChange={e => updateTableConfig({ checkbox_column_width: Number(e.target.value) || 36 })}
                sx={{ width: 110, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                inputProps={{ min: 20, max: 100, style: { textAlign: 'center' } }} />
            </Box>
          </Paper>

          {/* 字段列表 */}
          <Typography variant="subtitle2" sx={{ mb: 1, color: '#555' }}>
            字段定义（共 {fields.length} 个，可见 {visibleFields.length} 个）
          </Typography>

          <Paper variant="outlined" sx={{ p: 1, mb: 2 }}>
            {fields.map((f, idx) => (
              <Box key={`${f.key}-${idx}`} sx={{
                display: 'flex', alignItems: 'center', gap: 1, py: 0.8, px: 1,
                borderBottom: idx === fields.length - 1 ? 'none' : '1px solid #eee',
                flexWrap: 'wrap',
              }}>
                {/* 排序按钮 */}
                <Tooltip title="上移">
                  <span>
                    <IconButton size="small" onClick={() => moveField(idx, idx - 1)} disabled={idx === 0}
                      sx={{ p: 0.3 }}><Typography fontSize="1rem">⬆</Typography></IconButton>
                  </span>
                </Tooltip>
                <Tooltip title="下移">
                  <span>
                    <IconButton size="small" onClick={() => moveField(idx, idx + 1)} disabled={idx === fields.length - 1}
                      sx={{ p: 0.3 }}><Typography fontSize="1rem">⬇</Typography></IconButton>
                  </span>
                </Tooltip>

                <DragIndicatorIcon sx={{ fontSize: 18, color: '#bbb', mr: 0.5 }} />

                {/* 可见开关 */}
                <Switch size="small" checked={f.visible !== false}
                  onChange={e => updateField(idx, { visible: e.target.checked })} />

                {/* 标签 */}
                <TextField size="small" value={f.label} onChange={e => updateField(idx, { label: e.target.value })}
                  sx={{ width: 110, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                  inputProps={{ style: { padding: '2px 6px' } }} placeholder="标签" />

                {/* 宽度 */}
                <TextField size="small" type="number" value={f.width || ''}
                  onChange={e => updateField(idx, { width: Number(e.target.value) || 100 })}
                  sx={{ width: 65, '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.8rem' } }}
                  inputProps={{ min: 40, max: 400, style: { padding: '2px 6px', textAlign: 'center' } }} placeholder="px" />

                {/* 类型 */}
                <Select size="small" value={f.type} onChange={e => updateField(idx, { type: e.target.value as FieldDef['type'] })}
                  sx={{ width: 100, borderRadius: R, fontSize: '0.8rem', '& .MuiSelect-select': { py: 0.6 } }}>
                  {FIELD_TYPES.map(ft => <MenuItem key={ft} value={ft} sx={{ fontSize: '0.8rem' }}>{ft}</MenuItem>)}
                </Select>

                {/* 删除 */}
                <IconButton size="small" onClick={() => removeField(idx)}
                  sx={{ ml: 'auto', color: '#e57373', p: 0.5 }}>
                  <DeleteIcon fontSize="small" />
                </IconButton>
              </Box>
            ))}

            <Button startIcon={<AddIcon />} size="small" onClick={addField}
              sx={{ mt: 1, borderRadius: R, fontSize: '0.8rem' }}>
              添加字段
            </Button>
          </Paper>

          {/* 操作按钮 */}
          <Box sx={{ display: 'flex', gap: 1, mb: 3 }}>
            <Button variant="contained" size="small" startIcon={<SaveIcon />} onClick={save}
              disabled={!dirty || saving}
              sx={{ borderRadius: R, fontSize: '0.8rem' }}>
              {saving ? '保存中…' : '保存字段布局'}
            </Button>
            <Button variant="outlined" size="small" startIcon={<RefreshIcon />} onClick={reset}
              sx={{ borderRadius: R, fontSize: '0.8rem' }} color="warning">
              重置为默认
            </Button>
          </Box>

          {/* 预览 */}
          <Divider sx={{ mb: 2 }} />
          <Typography variant="subtitle2" sx={{ mb: 1, color: '#555' }}>实时预览</Typography>
          <Paper variant="outlined" sx={{ overflow: 'auto' }}>
            <TableContainer>
              <Table size="small" sx={{ minWidth: Math.max(totalWidth + 80, 600) }}>
                <TableHead>
                  <TableRow sx={{ height: tableConfig.row_height }}>
                    <TableCell padding="checkbox" sx={{ borderRight: '1px solid #ddd', width: tableConfig.checkbox_column_width }}>
                      <Typography fontSize="0.7rem">☐</Typography>
                    </TableCell>
                    <TableCell sx={{ fontWeight: 600, fontSize: '0.75rem', whiteSpace: 'nowrap', borderRight: '1px solid #ddd', width: tableConfig.seq_column_width, textAlign: 'center' }}>
                      序号
                    </TableCell>
                    {visibleFields.map(f => (
                      <TableCell key={f.key} sx={{
                        fontWeight: 600, fontSize: '0.75rem', whiteSpace: 'nowrap',
                        width: f.width || 100, borderRight: '1px solid #eee',
                      }}>
                        {f.label}
                      </TableCell>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  <TableRow sx={{ height: tableConfig.row_height }}>
                    <TableCell padding="checkbox" sx={{ borderRight: '1px solid #ddd', width: tableConfig.checkbox_column_width }}>
                      <Typography fontSize="0.7rem">☐</Typography>
                    </TableCell>
                    <TableCell sx={{ fontSize: '0.7rem', textAlign: 'center', borderRight: '1px solid #ddd', width: tableConfig.seq_column_width, color: '#999' }}>
                      1
                    </TableCell>
                    {visibleFields.map(f => (
                      <TableCell key={f.key} sx={{
                        p: 0.5, borderRight: '1px solid #eee',
                        width: f.width || 100,
                      }}>
                        <Box sx={{
                          height: Math.min(tableConfig.row_height - 16, 28), bgcolor: '#f5f5f5', borderRadius: R,
                          display: 'flex', alignItems: 'center', px: 1,
                        }}>
                          <Typography fontSize="0.7rem" color="#999">
                            {f.type === 'select' ? '▾' : f.type === 'number' ? '123' : '·'}
                          </Typography>
                        </Box>
                      </TableCell>
                    ))}
                  </TableRow>
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

export default ManageFormConfig;
