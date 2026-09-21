import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle,
  FormControl, FormControlLabel, InputLabel, MenuItem, Paper, Select, Snackbar,
  Switch, Table, TableBody, TableCell, TableHead, TableRow, TextField, Typography,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import EditIcon from '@mui/icons-material/Edit';
import IconButton from '@mui/material/IconButton';
import SaveIcon from '@mui/icons-material/Save';
import { DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, arrayMove, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import {
  createRdRecordColumn, deleteRdRecordColumn, getRdRecordColumns,
  reorderRdRecordColumns, updateRdRecordColumn,
} from '../api/client';
import type { RdRecordColumn, RdRecordColumnInput } from '../types';
import { parseRdFieldOptions, parseRdOptionDetailRules, type RdOptionDetailRule } from '../utils/rdOptionDetails';

const R = '2px';
const TYPES: Array<{ value: RdRecordColumn['data_type']; label: string }> = [
  { value: 'text', label: '单行文本' },
  { value: 'textarea', label: '多行文本' },
  { value: 'number', label: '数字' },
  { value: 'date', label: '日期' },
  { value: 'datetime', label: '日期时间' },
  { value: 'select', label: '下拉选择' },
];

const emptyDraft = (): RdRecordColumnInput => ({
  name: '', label: '', data_type: 'text', width: 140, is_required: false,
  is_active: true, show_in_list: true, show_in_form: true, show_in_export: true,
  options: '', option_detail_rules: '', default_value: '', placeholder: '', applicable_types: '', entry_row: 2,
});

const SortableColumnRow: React.FC<{
  column: RdRecordColumn;
  index: number;
  total: number;
  onEdit: (column: RdRecordColumn) => void;
  onDelete: (column: RdRecordColumn) => void;
}> = ({ column, index, total, onEdit, onDelete }) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: column.id });
  const style = { transform: CSS.Transform.toString(transform), transition, opacity: isDragging ? 0.5 : column.is_active ? 1 : 0.55 };
  return <TableRow ref={setNodeRef} hover sx={style}>
    <TableCell sx={{ whiteSpace: 'nowrap', p: 0.25 }}>
      <IconButton size="small" aria-label={`拖拽排序 ${column.label}`} title="按住拖拽调整顺序" {...attributes} {...listeners} sx={{ cursor: 'grab', touchAction: 'none' }}><DragIndicatorIcon fontSize="small" /></IconButton>
      <Typography component="span" variant="caption" color="text.secondary">{index + 1}/{total}</Typography>
    </TableCell>
    <TableCell><Typography fontSize="0.84rem" fontWeight={600}>{column.label}</Typography><Typography variant="caption" color="text.secondary">{column.name}{column.is_predefined ? ' · 预置' : ' · 自定义'}</Typography></TableCell>
    <TableCell sx={{ fontSize: '0.82rem' }}>{TYPES.find(item => item.value === column.data_type)?.label || column.data_type}</TableCell>
    {/* v2.3.18: 启用、必填与显示范围统一在编辑弹窗内设置，行内只做只读摘要，避免表格过宽。 */}
    <TableCell sx={{ fontSize: '0.78rem', color: 'text.secondary', whiteSpace: 'normal', lineHeight: 1.4 }}>
      {column.is_active ? '启用' : '停用'}
      {column.is_required ? ' · 必填' : ''}
      {column.show_in_form ? ' · 表单' : ''}
      {column.show_in_list ? ' · 列表' : ''}
      {column.show_in_export ? ' · 导出' : ''}
      {column.entry_row === 2 ? ' · 第二行' : ''}
    </TableCell>
    <TableCell align="center">{column.width}</TableCell>
    <TableCell align="center" sx={{ whiteSpace: 'nowrap', p: 0.25 }}><IconButton size="small" color="primary" title="编辑字段" onClick={() => onEdit(column)}><EditIcon fontSize="small" /></IconButton>{!column.is_predefined && <IconButton size="small" color="error" title="删除自定义字段" onClick={() => onDelete(column)}><DeleteOutlineIcon fontSize="small" /></IconButton>}</TableCell>
  </TableRow>;
};

const AdminRdRecordColumns: React.FC = () => {
  const [cols, setCols] = useState<RdRecordColumn[]>([]);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<RdRecordColumn | null>(null);
  const [draft, setDraft] = useState<RdRecordColumnInput>(emptyDraft());
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState(false);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  const load = useCallback(async () => {
    try {
      const response = await getRdRecordColumns();
      if (response.code !== 0) throw new Error(response.message || '读取配置失败');
      setCols((response.data || []).sort((a, b) => a.sort_order - b.sort_order || a.id - b.id));
    } catch (e: any) {
      setError(true); setMessage(e?.message || '读取配置失败');
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const activeCount = useMemo(() => cols.filter(item => item.is_active).length, [cols]);
  const patchDraft = (patch: Partial<RdRecordColumnInput>) => setDraft(previous => ({ ...previous, ...patch }));

  const openCreate = () => {
    setEditing(null);
    setDraft(emptyDraft());
    setDialogOpen(true);
  };

  const openEdit = (column: RdRecordColumn) => {
    setEditing(column);
    setDraft({
      name: column.name, label: column.label, data_type: column.data_type, width: column.width,
      is_required: column.is_required, is_active: column.is_active, show_in_list: column.show_in_list,
      show_in_form: column.show_in_form, show_in_export: column.show_in_export, options: column.options,
      option_detail_rules: column.option_detail_rules,
      default_value: column.default_value, placeholder: column.placeholder,
      applicable_types: column.applicable_types, entry_row: column.entry_row, sort_order: column.sort_order,
    });
    setDialogOpen(true);
  };

  const save = async () => {
    setSaving(true);
    try {
      const response = editing
        ? await updateRdRecordColumn(editing.id, draft)
        : await createRdRecordColumn(draft);
      if (response.code !== 0) throw new Error(response.message || '保存失败');
      setDialogOpen(false);
      setError(false); setMessage(editing ? '字段配置已保存' : '自定义字段已新增');
      await load();
    } catch (e: any) {
      setError(true); setMessage(e?.message || '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const reorderByDrag = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = cols.findIndex(item => item.id === active.id);
    const newIndex = cols.findIndex(item => item.id === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    const next = arrayMove(cols, oldIndex, newIndex);
    setCols(next);
    try {
      const response = await reorderRdRecordColumns(next.map(item => item.id));
      if (response.code !== 0) throw new Error(response.message || '排序保存失败');
      setError(false); setMessage('字段顺序已保存');
      await load();
    } catch (e: any) {
      setError(true); setMessage(e?.message || '排序保存失败');
      await load();
    }
  };

  const remove = async (column: RdRecordColumn) => {
    if (!window.confirm(`删除自定义字段“${column.label}”后，历史记录中的原始值仍会保留。是否继续？`)) return;
    try {
      const response = await deleteRdRecordColumn(column.id);
      if (response.code !== 0) throw new Error(response.message || '删除失败');
      setError(false); setMessage('自定义字段已删除');
      await load();
    } catch (e: any) {
      setError(true); setMessage(e?.message || '删除失败');
    }
  };

  const usesOptions = draft.data_type === 'select' || draft.data_type === 'select_other';
  const optionValues = useMemo(() => parseRdFieldOptions(draft.options), [draft.options]);
  const detailRules = useMemo(() => parseRdOptionDetailRules(draft.option_detail_rules), [draft.option_detail_rules]);
  const setDetailRules = (rules: RdOptionDetailRule[]) => patchDraft({ option_detail_rules: rules.length ? JSON.stringify(rules) : '' });
  const patchDetailRule = (index: number, patch: Partial<RdOptionDetailRule>) => {
    setDetailRules(detailRules.map((rule, itemIndex) => itemIndex === index ? { ...rule, ...patch } : rule));
  };
  const addDetailRule = () => {
    const triggerValue = optionValues.find(option => !detailRules.some(rule => rule.trigger_value === option)) || '';
    if (!triggerValue) return;
    setDetailRules([...detailRules, { trigger_value: triggerValue, label: '补充说明', placeholder: '', required: false }]);
  };
  return (
    <Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1, flexWrap: 'wrap', mb: 1.5 }}>
        <Box>
          <Typography variant="subtitle1" fontWeight={700}>研发送样信息登记配置</Typography>
          <Typography variant="caption" color="text.secondary">启用字段 {activeCount} 个，按住排序图标可拖拽调整顺序；启用、必填与显示范围在“编辑字段”弹窗内设置</Typography>
        </Box>
        <Button variant="contained" size="small" startIcon={<AddIcon />} onClick={openCreate} sx={{ borderRadius: R }}>新增字段</Button>
      </Box>
      <Paper elevation={0} sx={{ borderRadius: R, border: '1px solid rgba(0,0,0,0.1)', overflowX: 'hidden' }}>
        {/* v2.3.18: 启用 / 必填 / 显示范围都收进编辑弹窗，表格固定布局后一屏即可显示完整行。 */}
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={reorderByDrag}><Table size="small" sx={{ width: '100%', tableLayout: 'fixed', minWidth: 0 }}>
          <TableHead>
            <TableRow sx={{ bgcolor: '#f5f5f5' }}>
              <TableCell sx={{ fontWeight: 700, width: 82 }}>排序</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>字段名称</TableCell>
              <TableCell sx={{ fontWeight: 700, width: 110 }}>输入方式</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>显示 / 启用（编辑弹窗内设置）</TableCell>
              <TableCell sx={{ fontWeight: 700, textAlign: 'center', width: 80 }}>宽度</TableCell>
              <TableCell sx={{ fontWeight: 700, textAlign: 'center', width: 96 }}>操作</TableCell>
            </TableRow>
          </TableHead>
          <SortableContext items={cols.map(column => column.id)} strategy={verticalListSortingStrategy}><TableBody>
            {cols.map((column, index) => <SortableColumnRow key={column.id} column={column} index={index} total={cols.length} onEdit={openEdit} onDelete={remove} />)}
            {cols.length === 0 && <TableRow><TableCell colSpan={6} align="center" sx={{ py: 4, color: 'text.secondary' }}>暂无字段配置</TableCell></TableRow>}
          </TableBody></SortableContext>
        </Table></DndContext>
      </Paper>

      <Dialog open={dialogOpen} onClose={() => !saving && setDialogOpen(false)} fullWidth maxWidth="sm">
        <DialogTitle>{editing ? `编辑字段：${editing.label}` : '新增研发送样字段'}</DialogTitle>
        <DialogContent dividers>
          <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '1fr 1fr' }, gap: 1.5, pt: 0.5 }}>
            <TextField label="显示名称" value={draft.label} required onChange={event => patchDraft({ label: event.target.value })} />
            <TextField label="字段标识" value={draft.name} disabled={!!editing} required helperText={editing ? '系统标识不可修改' : '仅小写字母、数字和下划线'} onChange={event => patchDraft({ name: event.target.value })} />
            <FormControl fullWidth>
              <InputLabel>输入方式</InputLabel>
              <Select label="输入方式" value={draft.data_type} disabled={Boolean(editing?.is_predefined)} onChange={event => {
                const data_type = event.target.value as RdRecordColumn['data_type'];
                patchDraft({ data_type, option_detail_rules: data_type === 'select' || data_type === 'select_other' ? draft.option_detail_rules : '' });
              }}>
                {TYPES.map(type => <MenuItem key={type.value} value={type.value}>{type.label}</MenuItem>)}
              </Select>
            </FormControl>
            <TextField label="列表宽度" type="number" value={draft.width ?? 140} inputProps={{ min: 48, max: 500 }} onChange={event => patchDraft({ width: Number(event.target.value) || 48 })} />
            <TextField label="占位提示" value={draft.placeholder || ''} onChange={event => patchDraft({ placeholder: event.target.value })} />
            <TextField label="默认值" value={draft.default_value || ''} onChange={event => patchDraft({ default_value: event.target.value })} />
            <TextField label="适用检测类型" value={draft.applicable_types || ''} placeholder="留空表示全部；多个类型用逗号分隔" onChange={event => patchDraft({ applicable_types: event.target.value })} sx={{ gridColumn: { sm: '1 / -1' } }} />
            {usesOptions && <>
              <TextField label="下拉选项" value={draft.options || ''} placeholder="每行一个选项，或使用逗号分隔" multiline minRows={3} onChange={event => patchDraft({ options: event.target.value })} sx={{ gridColumn: { sm: '1 / -1' } }} />
              <Box sx={{ gridColumn: { sm: '1 / -1' }, border: '1px solid', borderColor: 'divider', borderRadius: R, p: 1.25, display: 'grid', gap: 1 }}>
                <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
                  <Box><Typography fontSize="0.86rem" fontWeight={700}>选项触发补充填写</Typography><Typography variant="caption" color="text.secondary">例如：选择“加急”后填写加急原因；选择“其他”后填写补充说明。</Typography></Box>
                  <Button size="small" variant="outlined" startIcon={<AddIcon />} onClick={addDetailRule} disabled={!optionValues.length || detailRules.length >= optionValues.length}>新增规则</Button>
                </Box>
                {!optionValues.length && <Alert severity="info">请先填写下拉选项，再新增补充规则。</Alert>}
                {detailRules.map((rule, index) => <Box key={`${rule.trigger_value}-${index}`} sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '0.8fr 1fr' }, gap: 1, alignItems: 'center', pt: index ? 1 : 0, borderTop: index ? '1px solid rgba(0,0,0,0.08)' : 'none' }}>
                  <TextField select label="触发选项" size="small" value={rule.trigger_value} onChange={event => patchDetailRule(index, { trigger_value: event.target.value })}>
                    {optionValues.map(option => <MenuItem key={option} value={option}>{option}</MenuItem>)}
                  </TextField>
                  <TextField label="补充项名称" size="small" value={rule.label} onChange={event => patchDetailRule(index, { label: event.target.value })} />
                  <TextField label="输入提示" size="small" value={rule.placeholder || ''} onChange={event => patchDetailRule(index, { placeholder: event.target.value })} sx={{ gridColumn: { sm: '1 / -1' } }} />
                  <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gridColumn: { sm: '1 / -1' } }}>
                    <FormControlLabel control={<Switch size="small" checked={Boolean(rule.required)} onChange={event => patchDetailRule(index, { required: event.target.checked })} />} label="选择后必填" />
                    <IconButton size="small" color="error" title="删除补充规则" onClick={() => setDetailRules(detailRules.filter((_, itemIndex) => itemIndex !== index))}><DeleteOutlineIcon fontSize="small" /></IconButton>
                  </Box>
                </Box>)}
              </Box>
            </>}
            <Box sx={{ gridColumn: { sm: '1 / -1' }, display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
              <FormControlLabel control={<Switch checked={Boolean(draft.is_active)} onChange={event => patchDraft({ is_active: event.target.checked })} />} label="启用字段" />
              <FormControlLabel control={<Switch checked={Boolean(draft.is_required)} onChange={event => patchDraft({ is_required: event.target.checked })} />} label="必填" />
              <FormControlLabel control={<Switch checked={Boolean(draft.show_in_form)} onChange={event => patchDraft({ show_in_form: event.target.checked })} />} label="表单显示" />
              <FormControlLabel control={<Switch checked={Boolean(draft.show_in_list)} onChange={event => patchDraft({ show_in_list: event.target.checked })} />} label="列表显示" />
              <FormControlLabel control={<Switch checked={Boolean(draft.show_in_export)} onChange={event => patchDraft({ show_in_export: event.target.checked })} />} label="导出显示" />
              <FormControlLabel control={<Switch checked={(draft.entry_row ?? 1) === 2} onChange={event => patchDraft({ entry_row: event.target.checked ? 2 : 1 })} />} label="第二行显示" />
            </Box>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDialogOpen(false)} disabled={saving}>取消</Button>
          <Button variant="contained" startIcon={<SaveIcon />} onClick={save} disabled={saving}>保存</Button>
        </DialogActions>
      </Dialog>
      <Snackbar open={Boolean(message)} autoHideDuration={3000} onClose={() => setMessage('')}><Alert severity={error ? 'error' : 'success'} onClose={() => setMessage('')}>{message}</Alert></Snackbar>
    </Box>
  );
};

export default AdminRdRecordColumns;
