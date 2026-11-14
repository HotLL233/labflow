import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle, Divider,
  FormControlLabel, Paper, Switch,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Tabs, Tab,
  TextField, Typography, MenuItem, Snackbar,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import EditIcon from '@mui/icons-material/Edit';
import RefreshIcon from '@mui/icons-material/Refresh';
import SaveIcon from '@mui/icons-material/Save';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy, arrayMove } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { FieldDef, FormLayout, TableConfig } from '../types/layout';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import { getSetting, updateSetting } from '../api/client';
import { useManagement, useUnsavedManagement } from './ManagementContext';

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
  onEdit: () => void;
  onRemove: () => void;
}

/** 字段显示范围摘要，供列表行只读展示；具体开关在编辑弹窗内设置。 */
const fieldScopeSummary = (item: FieldDef) => {
  const parts = [
    item.required === true ? '必填' : '选填',
    item.show_in_form !== false ? '表单' : '表单隐藏',
    item.show_in_list !== false ? '列表' : '列表隐藏',
    item.show_in_export !== false ? '导出' : '导出隐藏',
  ];
  return `${FIELD_TYPE_LABELS[item.type]} · ${parts.join(' / ')}`;
};

const SortableRow: React.FC<SortableRowProps> = ({ item, index, onEdit, onRemove }) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: item.key });
  const system = SYSTEM_FIELDS.has(item.key);
  return (
    <TableRow ref={setNodeRef} sx={{ transform: CSS.Transform.toString(transform), transition, bgcolor: isDragging ? '#fff8e1' : 'inherit' }}>
      <TableCell padding="checkbox" sx={{ width: 42 }}>
        <Box {...attributes} {...listeners} sx={{ cursor: 'grab', display: 'flex', color: '#78909c' }} title="拖动调整顺序">
          <DragIndicatorIcon fontSize="small" />
        </Box>
      </TableCell>
      <TableCell align="center" sx={{ width: 54, fontWeight: 700 }}>{index + 1}</TableCell>
      <TableCell sx={{ minWidth: 0 }}>
        <Typography variant="body2" fontFamily="monospace" sx={{ overflowWrap: 'anywhere' }}>{item.key}</Typography>
        {system && <Chip size="small" label="系统字段" sx={{ mt: 0.25, height: 18 }} />}
      </TableCell>
      <TableCell sx={{ minWidth: 0 }}>
        <Typography variant="body2" fontWeight={600} sx={{ overflowWrap: 'anywhere', lineHeight: 1.35 }}>{item.label}</Typography>
        <Typography variant="caption" color="text.secondary" sx={{ display: 'block', overflowWrap: 'anywhere' }}>{fieldScopeSummary(item)}</Typography>
      </TableCell>
      <TableCell sx={{ width: 96 }}>
        <Typography variant="body2">{item.width || 100} px</Typography>
      </TableCell>
      <TableCell padding="checkbox" sx={{ width: 108, whiteSpace: 'nowrap' }}>
        <Button size="small" color="primary" onClick={onEdit} title="编辑字段（启用、必填与显示范围）"><EditIcon fontSize="small" /></Button>
        <Button size="small" color="error" disabled={system} onClick={onRemove} title={system ? '系统字段不可删除' : '停用字段'}><DeleteOutlineIcon fontSize="small" /></Button>
      </TableCell>
    </TableRow>
  );
};

const ManageFormConfig: React.FC = () => {
  const management = useManagement();
  const [tab, setTab] = useState(() => management?.readDraft<number>('form-selected') || 0);
  const [loadError, setLoadError] = useState('');
  const request = useRef(0);
  const [fields, setFields] = useState<FieldDef[]>([]);
  const [tableConfig, setTableConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [snack, setSnack] = useState('');
  const [error, setError] = useState(false);
  // v2.3.18: 必填与显示范围（表单 / 列表 / 导出）开关统一放到字段编辑弹窗内，
  // 列表行只展示只读摘要，表格因此不再横向滚动。
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [draft, setDraft] = useState<FieldDef | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  const template = FORM_TEMPLATES[tab];
  const currentForm = useRef({ fields, tableConfig, draft, editingIndex }); currentForm.current = { fields, tableConfig, draft, editingIndex };

  const load = useCallback(async (key: string) => {
    const id = ++request.current;
    const restored = management?.readDraft<{ fields: FieldDef[]; tableConfig: TableConfig; editingIndex: number | null; draft: FieldDef | null }>(key);
    if (restored) { setFields(restored.fields); setTableConfig(restored.tableConfig); setEditingIndex(restored.editingIndex); setDraft(restored.draft); setDirty(true); setLoadError(''); return; }
    setLoading(true); setLoadError('');
    try {
      const response = await getSetting(key);
      if (id !== request.current) return;
      if (response.code !== 0) throw new Error(response.message || '配置读取失败');
      const parsed = response.data?.value ? JSON.parse(response.data.value) : null;
      const raw = Array.isArray(parsed) ? parsed : parsed?.fields;
      if (parsed && !Array.isArray(raw)) throw new Error('字段配置格式异常');
      setFields(normalizeFields(raw || template.defaultFields));
      setTableConfig({ ...DEFAULT_TABLE_CONFIG, ...(parsed?.table_config || {}) });
      setDirty(false);
    } catch (error: any) { if (id === request.current) setLoadError(error?.message || '配置读取失败'); }
    finally { if (id === request.current) setLoading(false); }
  }, [template, management?.readDraft]);
  useEffect(() => { void load(template.key); return () => { ++request.current; }; }, [load, template.key]);

  const updateAt = (index: number, patch: Partial<FieldDef>) => {
    setFields(current => current.map((item, i) => i === index ? { ...item, ...patch } : item));
    setDirty(true);
  };
  const closeFieldEdit = () => { setEditingIndex(null); setDraft(null); };
  const openFieldEdit = (index: number) => {
    if (!fields[index]) return;
    setEditingIndex(index);
    setDraft({ ...fields[index] });
  };
  const patchDraft = (patch: Partial<FieldDef>) => setDraft(current => (current ? { ...current, ...patch } : current));
  const setDraftVisibility = (key: 'show_in_form' | 'show_in_list' | 'show_in_export', value: boolean) => {
    patchDraft({ [key]: value, ...(key === 'show_in_form' ? { visible: value } : {}) } as Partial<FieldDef>);
  };
  const confirmFieldEdit = () => {
    if (editingIndex === null || !draft) return;
    updateAt(editingIndex, draft);
    closeFieldEdit();
  };
  const removeAt = (index: number) => {
    if (SYSTEM_FIELDS.has(fields[index]?.key)) return;
    setFields(current => normalizeFields(current.filter((_, i) => i !== index)));
    setDirty(true);
  };
  const addField = () => {
    const key = `custom_${Date.now()}`;
    const created: FieldDef = { key, type: 'text', label: '新字段', width: 120, required: false, visible: true, show_in_form: true, show_in_list: true, show_in_export: true, entry_row: 1, sort_order: fields.length + 1, placeholder: '' };
    setFields(current => normalizeFields([...current, created]));
    setDirty(true);
    // 新增后直接打开编辑弹窗，必填与显示范围在同一处设置。
    setEditingIndex(fields.length);
    setDraft({ ...created });
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
    if (saving || loadError || loading) return false;
    const submitted = JSON.stringify(currentForm.current);
    setSaving(true);
    try {
      const normalized = normalizeFields(fields.map((field, index) => index === editingIndex && draft ? draft : field));
      const response = await updateSetting(template.key, { table_config: tableConfig, fields: normalized } satisfies FormLayout);
      if (response.code !== 0) throw new Error(response.message || '保存失败');
      const unchanged = JSON.stringify(currentForm.current) === submitted;
      if (unchanged) { setFields(normalized); setDirty(false); closeFieldEdit(); management?.clearDraft(template.key); }
      setSnack(unchanged ? '字段配置已保存' : '本次提交已保存，后续编辑仍待保存'); setError(false); return unchanged;
    } catch (e: any) { setSnack(e?.message || '字段配置保存失败'); setError(true); return false; }
    finally { setSaving(false); }
  };
  const fieldDirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(fields[editingIndex ?? -1]);
  const pendingChanges = dirty || fieldDirty;
  useEffect(() => { management?.writeDraft('form-selected', tab); if (pendingChanges) management?.writeDraft(template.key, { fields, tableConfig, editingIndex, draft }); else management?.clearDraft(template.key); }, [fields, tableConfig, editingIndex, draft, pendingChanges, tab, template.key, management?.writeDraft, management?.clearDraft]);
  useUnsavedManagement(template.key, { label: `${template.name}字段配置`, dirty: pendingChanges, save, discard: () => { setDirty(false); closeFieldEdit(); } });
  const reset = () => { setFields(normalizeFields(template.defaultFields)); setTableConfig({ ...DEFAULT_TABLE_CONFIG }); setDirty(true); };
  const formFields = useMemo(() => fields.filter(item => item.show_in_form !== false), [fields]);
  const listFields = useMemo(() => fields.filter(item => item.show_in_list !== false), [fields]);

  return (
    <Box>
      <Tabs value={tab} onChange={(_, value) => { const change = () => { setTab(value); setEditingIndex(null); setDraft(null); setDirty(false); }; if (management) management.guard(change); else if (!pendingChanges || window.confirm('放弃未保存的字段配置？')) change(); }} variant="scrollable" scrollButtons="auto" allowScrollButtonsMobile sx={{ mb: 2 }}>
        {FORM_TEMPLATES.map(item => <Tab key={item.key} label={item.name} />)}
      </Tabs>
      <Typography variant="body2" sx={{ mb: 1.5 }}>作用对象：{template.name} · {pendingChanges ? '有未保存修改' : '已读取系统配置'}。字段窗口只应用草稿，底部保存后生效。</Typography>
      {loadError && <Alert severity="error" sx={{ mb: 2 }} action={<Button onClick={() => void load(template.key)}>重试读取</Button>}>{loadError}。为避免覆盖现有配置，读取成功后才允许保存。</Alert>}
      <Box component="fieldset" disabled={saving || Boolean(loadError)} sx={{ border: 0, p: 0, m: 0, minWidth: 0 }}>
      {loading ? <Typography color="text.secondary" sx={{ p: 2 }}>加载中...</Typography> : <>
        <Paper variant="outlined" sx={{ p: 2, mb: 2, borderRadius: R }}>
          <Typography variant="subtitle2" fontWeight={700} sx={{ mb: 1 }}>表格配置</Typography>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
            <TextField size="small" type="number" label="行高" sx={{ width: 144 }} value={tableConfig.row_height} onChange={e => { setTableConfig(c => ({ ...c, row_height: Number(e.target.value) || 48 })); setDirty(true); }} />
            <TextField size="small" type="number" label="序号列宽" sx={{ width: 144 }} value={tableConfig.seq_column_width} onChange={e => { setTableConfig(c => ({ ...c, seq_column_width: Number(e.target.value) || 50 })); setDirty(true); }} />
            <TextField size="small" type="number" label="选择列宽" sx={{ width: 144 }} value={tableConfig.checkbox_column_width} onChange={e => { setTableConfig(c => ({ ...c, checkbox_column_width: Number(e.target.value) || 36 })); setDirty(true); }} />
          </Box>
        </Paper>
        <Alert severity="info" sx={{ mb: 2, borderRadius: R }}>拖动左侧手柄调整顺序。列序号自动生成；必填与显示范围（表单 / 列表 / 导出）在“编辑”弹窗内设置；系统字段不能删除，停用后历史数据仍保留。样品信息登记字段请在“样品信息登记管理 → 自定义列”中配置，以保留检测类型显示范围。</Alert>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', lg: 'minmax(0,1fr) 280px' }, gap: 2, alignItems: 'start' }}>
        <Box sx={{ minWidth: 0 }}>
        <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, overflowX: 'auto' }}>
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            {/* v2.3.18: 固定布局 + 精简列，一屏显示完整行，不再左右滑动。 */}
            <Table size="small" sx={{ width: '100%', tableLayout: 'fixed', minWidth: 620 }}>
              <TableHead><TableRow>
                <TableCell padding="checkbox" sx={{ width: 42 }} />
                <TableCell align="center" sx={{ width: 56 }}>序号</TableCell>
                <TableCell sx={{ width: '30%' }}>字段标识</TableCell>
                <TableCell>显示名称 / 显示范围</TableCell>
                <TableCell align="center" sx={{ width: 96 }}>宽度</TableCell>
                <TableCell align="center" sx={{ width: 108 }}>操作</TableCell>
              </TableRow></TableHead>
              <SortableContext items={fields.map(item => item.key)} strategy={verticalListSortingStrategy}>
                <TableBody>{fields.map((item, index) => <SortableRow key={item.key} item={item} index={index} onEdit={() => openFieldEdit(index)} onRemove={() => removeAt(index)} />)}</TableBody>
              </SortableContext>
            </Table>
          </DndContext>
        </TableContainer>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', mt: 1.5, flexWrap: 'wrap' }}>
          <Button size="small" variant="outlined" startIcon={<AddIcon />} onClick={addField}>新增自定义字段</Button>
          <Button size="small" variant="contained" startIcon={<SaveIcon />} disabled={!pendingChanges || saving || Boolean(loadError)} onClick={save}>{saving ? '保存中...' : '保存字段配置'}</Button>
          <Button size="small" color="warning" variant="outlined" startIcon={<RefreshIcon />} onClick={reset}>恢复默认</Button>
          <Typography variant="caption" color="text.secondary">表单 {formFields.length} 项，列表 {listFields.length} 项，全部字段 {fields.length} 项</Typography>
        </Box>
        </Box>
        <Box sx={{ p: 2, borderLeft: { lg: '1px solid' }, borderTop: { xs: '1px solid', lg: 'none' }, borderColor: 'divider', minWidth: 0 }}>
          <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1 }}>表单字段预览</Typography>
          {formFields.map(item => <Box key={item.key} sx={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: 1, py: 1, borderBottom: '1px solid', borderColor: 'divider' }}>
            <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{item.label}{item.required ? ' *' : ''}</Typography><Typography variant="caption" color="text.secondary" sx={{ whiteSpace: 'nowrap' }}>{FIELD_TYPE_LABELS[item.type]}</Typography>
          </Box>)}
          <Typography variant="subtitle2" fontWeight={600} sx={{ mt: 2, mb: 1 }}>列表字段预览</Typography>
        <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
          {listFields.map((item, index) => <Chip key={item.key} label={`${index + 1}. ${item.label}`} variant="outlined" size="small" />)}
        </Box>
        </Box>
        </Box>
      </>}

      </Box>
      {/* v2.3.18: 字段编辑弹窗：必填与显示范围统一在此设置，保存后仍按“保存字段配置”提交。 */}
      <Dialog open={editingIndex !== null} onClose={closeFieldEdit} fullWidth maxWidth="sm">
        <DialogTitle>{draft ? `编辑字段：${draft.label || draft.key}` : '编辑字段'}</DialogTitle>
        <DialogContent dividers>
          <Typography variant="caption" color="text.secondary">作用于 {template.name}；应用后仍需保存字段配置</Typography>
          {draft && <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '1fr 1fr' }, gap: 1.5, pt: 0.5 }}>
            <TextField label="显示名称" value={draft.label} onChange={e => patchDraft({ label: e.target.value })} />
            <TextField select label="类型" value={draft.type} onChange={e => patchDraft({ type: e.target.value as FieldDef['type'] })}>
              {FIELD_TYPES.map(type => <MenuItem key={type} value={type}>{FIELD_TYPE_LABELS[type]}</MenuItem>)}
            </TextField>
            <TextField label="选项 / 来源" value={draft.options || ''} placeholder="下拉来源或选项" onChange={e => patchDraft({ options: e.target.value })} />
            <TextField label="列宽(px)" type="number" value={draft.width || 100} inputProps={{ min: 40, max: 480 }}
              onChange={e => patchDraft({ width: Math.max(40, Number(e.target.value) || 100) })} />
            <TextField select label="录入行" value={draft.entry_row || defaultEntryRow(draft.key)}
              onChange={e => patchDraft({ entry_row: Number(e.target.value) === 2 ? 2 : 1 })}>
              <MenuItem value={1}>第 1 行</MenuItem>
              <MenuItem value={2}>第 2 行</MenuItem>
            </TextField>
            <Box sx={{ gridColumn: { sm: '1 / -1' }, display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
              <FormControlLabel control={<Switch checked={draft.required === true} onChange={e => patchDraft({ required: e.target.checked })} />} label="必填" />
              <FormControlLabel control={<Switch checked={draft.show_in_form !== false} onChange={e => setDraftVisibility('show_in_form', e.target.checked)} />} label="表单显示" />
              <FormControlLabel control={<Switch checked={draft.show_in_list !== false} onChange={e => setDraftVisibility('show_in_list', e.target.checked)} />} label="列表显示" />
              <FormControlLabel control={<Switch checked={draft.show_in_export !== false} onChange={e => setDraftVisibility('show_in_export', e.target.checked)} />} label="导出显示" />
            </Box>
            {SYSTEM_FIELDS.has(draft.key) && <Alert severity="info" sx={{ gridColumn: { sm: '1 / -1' } }}>系统字段的标识不可修改，停用后历史数据仍保留。</Alert>}
          </Box>}
        </DialogContent>
        <DialogActions>
          <Button onClick={closeFieldEdit}>取消</Button>
          <Button variant="contained" onClick={confirmFieldEdit}>应用到待保存配置</Button>
        </DialogActions>
      </Dialog>

      <Snackbar open={Boolean(snack)} autoHideDuration={3500} onClose={() => setSnack('')}><Alert severity={error ? 'error' : 'success'}>{snack}</Alert></Snackbar>
    </Box>
  );
};

export default ManageFormConfig;
