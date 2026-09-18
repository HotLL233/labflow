import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert, Box, Button, Chip, Divider, FormControlLabel, Paper, Select, Switch,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Tabs, Tab,
  TextField, Typography, MenuItem, Snackbar,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import RefreshIcon from '@mui/icons-material/Refresh';
import SaveIcon from '@mui/icons-material/Save';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy, arrayMove } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { FieldDef, FormLayout, TableConfig } from '../types/layout';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import { getSetting, updateSetting } from '../api/client';

const R = '2px';
const FIELD_TYPES: FieldDef['type'][] = ['text', 'textarea', 'number', 'select', 'select_other', 'date', 'datetime', 'heading', 'divider'];
const FIELD_TYPE_LABELS: Record<FieldDef['type'], string> = {
  text: '文本',
  textarea: '多行文本',
  number: '数字',
  select: '下拉选择',
  select_other: '下拉选择（含其他）',
  date: '日期',
  datetime: '日期时间',
  heading: '分组标题',
  divider: '分隔线',
};

const defaultEntryRow = (key: string): 1 | 2 =>
  ['method_name', 'quantity', 'batch_no', 'notes'].includes(key) ? 2 : 1;

interface FormTemplate {
  key: string;
  name: string;
  defaultFields: FieldDef[];
}

const field = (key: string, type: FieldDef['type'], label: string, width: number, required = false, options?: string): FieldDef => ({
  key, type, label, width, required, visible: true, sort_order: 0,
  show_in_form: true, show_in_list: true, show_in_export: true, options, entry_row: defaultEntryRow(key),
});

const FORM_TEMPLATES: FormTemplate[] = [
  {
    key: 'form_sample_entry', name: '研发送样录入',
    defaultFields: [
      field('user_name', 'text', '送样人', 120, true),
      field('division_id', 'select', '部门', 140, false, '从用户关联读取'),
      field('lab_name', 'select', '实验室', 150, false, '实验室列表'),
      field('project_name', 'select', '项目', 160, true, '当前实验室进行中项目'),
      field('detection_type', 'select', '检测类型', 120, false, '项目关联检测类型'),
      field('method_name', 'select', '方法', 210, false, '项目关联方法'),
      field('quantity', 'number', '数量', 80, true),
      field('batch_no', 'text', '批号', 110),
      field('notes', 'textarea', '注意事项', 180),
    ],
  },
  {
    key: 'form_entry', name: '分析检测录入',
    defaultFields: [
      field('user_name', 'text', '检测人', 120, true), field('group_id', 'select', '实验室', 150, true),
      field('project_id', 'select', '项目', 160), field('method_id', 'select', '方法', 200),
      field('quantity', 'number', '数量', 80, true), field('recorded_at', 'datetime', '检测时间', 160),
      field('notes', 'textarea', '备注', 180),
    ],
  },
];

const SYSTEM_FIELDS = new Set(FORM_TEMPLATES.flatMap(template => template.defaultFields.map(item => item.key)));

const normalizeFields = (items: FieldDef[]): FieldDef[] => items.map((item, index) => ({
  ...item,
  sort_order: index + 1,
  visible: item.visible !== false,
  show_in_form: item.show_in_form ?? item.visible !== false,
  show_in_list: item.show_in_list ?? item.visible !== false,
  show_in_export: item.show_in_export ?? item.visible !== false,
  entry_row: item.entry_row === 2 ? 2 : defaultEntryRow(item.key),
}));

interface SortableRowProps {
  item: FieldDef;
  index: number;
  onChange: (patch: Partial<FieldDef>) => void;
  onRemove: () => void;
}

const SortableRow: React.FC<SortableRowProps> = ({ item, index, onChange, onRemove }) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: item.key });
  const system = SYSTEM_FIELDS.has(item.key);
  const setVisibility = (key: 'show_in_form' | 'show_in_list' | 'show_in_export', value: boolean) => {
    onChange({ [key]: value, ...(key === 'show_in_form' ? { visible: value } : {}) });
  };
  return (
    <TableRow ref={setNodeRef} sx={{ transform: CSS.Transform.toString(transform), transition, bgcolor: isDragging ? '#fff8e1' : 'inherit' }}>
      <TableCell padding="checkbox" sx={{ width: 42 }}>
        <Box {...attributes} {...listeners} sx={{ cursor: 'grab', display: 'flex', color: '#78909c' }} title="拖动调整顺序">
          <DragIndicatorIcon fontSize="small" />
        </Box>
      </TableCell>
      <TableCell align="center" sx={{ width: 54, fontWeight: 700 }}>{index + 1}</TableCell>
      <TableCell sx={{ minWidth: 150 }}>
        <Typography variant="body2" fontFamily="monospace">{item.key}</Typography>
        {system && <Chip size="small" label="系统字段" sx={{ mt: 0.25, height: 18 }} />}
      </TableCell>
      <TableCell sx={{ minWidth: 140 }}>
        <TextField size="small" fullWidth value={item.label} onChange={e => onChange({ label: e.target.value })} />
      </TableCell>
      <TableCell sx={{ minWidth: 110 }}>
        <Select size="small" fullWidth value={item.type} onChange={e => onChange({ type: e.target.value as FieldDef['type'] })}>
          {FIELD_TYPES.map(type => <MenuItem key={type} value={type}>{FIELD_TYPE_LABELS[type]}</MenuItem>)}
        </Select>
      </TableCell>
      <TableCell sx={{ minWidth: 150 }}>
        <TextField size="small" fullWidth value={item.options || ''} placeholder="下拉来源/选项" onChange={e => onChange({ options: e.target.value })} />
      </TableCell>
      <TableCell sx={{ width: 72 }}>
        <TextField size="small" type="number" value={item.width || 100} inputProps={{ min: 40, max: 480 }} onChange={e => onChange({ width: Math.max(40, Number(e.target.value) || 100) })} sx={{ width: 70 }} />
      </TableCell>
      <TableCell sx={{ width: 92 }}>
        <Select size="small" fullWidth value={item.entry_row || defaultEntryRow(item.key)} onChange={e => onChange({ entry_row: Number(e.target.value) === 2 ? 2 : 1 })}>
          <MenuItem value={1}>第 1 行</MenuItem>
          <MenuItem value={2}>第 2 行</MenuItem>
        </Select>
      </TableCell>
      <TableCell align="center" sx={{ width: 72 }}><Switch size="small" checked={item.required === true} onChange={e => onChange({ required: e.target.checked })} /></TableCell>
      <TableCell align="center" sx={{ width: 72 }}><Switch size="small" checked={item.show_in_form !== false} onChange={e => setVisibility('show_in_form', e.target.checked)} /></TableCell>
      <TableCell align="center" sx={{ width: 72 }}><Switch size="small" checked={item.show_in_list !== false} onChange={e => setVisibility('show_in_list', e.target.checked)} /></TableCell>
      <TableCell align="center" sx={{ width: 72 }}><Switch size="small" checked={item.show_in_export !== false} onChange={e => setVisibility('show_in_export', e.target.checked)} /></TableCell>
      <TableCell padding="checkbox" sx={{ width: 54 }}>
        <Button size="small" color="error" disabled={system} onClick={onRemove} title={system ? '系统字段不可删除' : '停用字段'}><DeleteOutlineIcon fontSize="small" /></Button>
      </TableCell>
    </TableRow>
  );
};

const ManageFormConfig: React.FC = () => {
  const [tab, setTab] = useState(0);
  const [fields, setFields] = useState<FieldDef[]>([]);
  const [tableConfig, setTableConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [snack, setSnack] = useState('');
  const [error, setError] = useState(false);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  const template = FORM_TEMPLATES[tab];

  const load = useCallback(async (key: string) => {
    setLoading(true);
    try {
      const response = await getSetting(key);
      if (response.code === 0 && response.data) {
        const parsed = JSON.parse(response.data.value);
        const raw = Array.isArray(parsed) ? parsed : parsed.fields;
        if (Array.isArray(raw) && raw.length) {
          setFields(normalizeFields(raw));
          setTableConfig({ ...DEFAULT_TABLE_CONFIG, ...(parsed.table_config || {}) });
          setDirty(false);
          return;
        }
      }
    } catch {
      // 使用默认模板，避免配置损坏时无法进入管理页。
    } finally {
      setLoading(false);
    }
    setFields(normalizeFields(template.defaultFields));
    setTableConfig({ ...DEFAULT_TABLE_CONFIG });
    setDirty(false);
  }, [template]);

  useEffect(() => { load(template.key); }, [load, template.key]);

  const updateAt = (index: number, patch: Partial<FieldDef>) => {
    setFields(current => current.map((item, i) => i === index ? { ...item, ...patch } : item));
    setDirty(true);
  };
  const removeAt = (index: number) => {
    if (SYSTEM_FIELDS.has(fields[index]?.key)) return;
    setFields(current => normalizeFields(current.filter((_, i) => i !== index)));
    setDirty(true);
  };
  const addField = () => {
    const key = `custom_${Date.now()}`;
    setFields(current => normalizeFields([...current, { key, type: 'text', label: '新字段', width: 120, required: false, visible: true, show_in_form: true, show_in_list: true, show_in_export: true, entry_row: 1, sort_order: current.length + 1, placeholder: '' }]));
    setDirty(true);
  };
  const onDragEnd = (event: DragEndEvent) => {
    if (!event.over || event.active.id === event.over.id) return;
    setFields(current => {
      const oldIndex = current.findIndex(item => item.key === event.active.id);
      const newIndex = current.findIndex(item => item.key === event.over?.id);
      return normalizeFields(arrayMove(current, oldIndex, newIndex));
    });
    setDirty(true);
  };
  const save = async () => {
    setSaving(true);
    try {
      const normalized = normalizeFields(fields);
      const response = await updateSetting(template.key, { table_config: tableConfig, fields: normalized } satisfies FormLayout);
      if (response.code !== 0) throw new Error(response.message || '保存失败');
      setFields(normalized); setDirty(false); setSnack('字段配置已保存'); setError(false);
    } catch (e: any) { setSnack(e?.message || '字段配置保存失败'); setError(true); }
    finally { setSaving(false); }
  };
  const reset = () => { setFields(normalizeFields(template.defaultFields)); setTableConfig({ ...DEFAULT_TABLE_CONFIG }); setDirty(true); };
  const formFields = useMemo(() => fields.filter(item => item.show_in_form !== false), [fields]);
  const listFields = useMemo(() => fields.filter(item => item.show_in_list !== false), [fields]);

  return (
    <Box>
      <Tabs value={tab} onChange={(_, value) => setTab(value)} sx={{ mb: 2 }}>
        {FORM_TEMPLATES.map(item => <Tab key={item.key} label={item.name} />)}
      </Tabs>
      {loading ? <Typography color="text.secondary" sx={{ p: 2 }}>加载中...</Typography> : <>
        <Paper variant="outlined" sx={{ p: 2, mb: 2, borderRadius: R }}>
          <Typography variant="subtitle2" fontWeight={700} sx={{ mb: 1 }}>表格配置</Typography>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
            <TextField size="small" type="number" label="行高" value={tableConfig.row_height} onChange={e => { setTableConfig(c => ({ ...c, row_height: Number(e.target.value) || 48 })); setDirty(true); }} />
            <TextField size="small" type="number" label="序号列宽" value={tableConfig.seq_column_width} onChange={e => { setTableConfig(c => ({ ...c, seq_column_width: Number(e.target.value) || 50 })); setDirty(true); }} />
            <TextField size="small" type="number" label="选择列宽" value={tableConfig.checkbox_column_width} onChange={e => { setTableConfig(c => ({ ...c, checkbox_column_width: Number(e.target.value) || 36 })); setDirty(true); }} />
          </Box>
        </Paper>
        <Alert severity="info" sx={{ mb: 2, borderRadius: R }}>拖动左侧手柄调整顺序。列序号自动生成；系统字段不能删除，停用后历史数据仍保留。样品信息登记字段请在“样品信息登记管理 → 自定义列”中配置，以保留检测类型显示范围。</Alert>
        <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, overflowX: 'auto' }}>
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            <Table size="small" sx={{ minWidth: 1320 }}>
              <TableHead><TableRow>
                <TableCell padding="checkbox" /> <TableCell align="center">列序号</TableCell><TableCell>字段标识</TableCell><TableCell>显示名称</TableCell><TableCell>类型</TableCell><TableCell>选项/来源</TableCell><TableCell>宽度</TableCell><TableCell>必填</TableCell><TableCell>表单</TableCell><TableCell>列表</TableCell><TableCell>导出</TableCell><TableCell>操作</TableCell>
              </TableRow></TableHead>
              <SortableContext items={fields.map(item => item.key)} strategy={verticalListSortingStrategy}>
                <TableBody>{fields.map((item, index) => <SortableRow key={item.key} item={item} index={index} onChange={patch => updateAt(index, patch)} onRemove={() => removeAt(index)} />)}</TableBody>
              </SortableContext>
            </Table>
          </DndContext>
        </TableContainer>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', mt: 1.5, flexWrap: 'wrap' }}>
          <Button size="small" variant="outlined" startIcon={<AddIcon />} onClick={addField}>新增自定义字段</Button>
          <Button size="small" variant="contained" startIcon={<SaveIcon />} disabled={!dirty || saving} onClick={save}>{saving ? '保存中...' : '保存字段配置'}</Button>
          <Button size="small" color="warning" variant="outlined" startIcon={<RefreshIcon />} onClick={reset}>恢复默认</Button>
          <Typography variant="caption" color="text.secondary">表单 {formFields.length} 项，列表 {listFields.length} 项，全部字段 {fields.length} 项</Typography>
        </Box>
        <Divider sx={{ my: 2 }} />
        <Typography variant="subtitle2" fontWeight={700} sx={{ mb: 1 }}>预览</Typography>
        <Paper variant="outlined" sx={{ p: 1.5, display: 'flex', gap: 1, flexWrap: 'wrap', borderRadius: R }}>
          {listFields.map((item, index) => <Chip key={item.key} label={`${index + 1}. ${item.label}`} variant="outlined" size="small" />)}
        </Paper>
      </>}
      <Snackbar open={Boolean(snack)} autoHideDuration={3500} onClose={() => setSnack('')}><Alert severity={error ? 'error' : 'success'}>{snack}</Alert></Snackbar>
    </Box>
  );
};

export default ManageFormConfig;
