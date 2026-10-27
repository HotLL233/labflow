import { useEffect, useRef, useState } from 'react';
import { Alert, Autocomplete, Box, Button, Checkbox, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, MenuItem, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, TextField, Typography } from '@mui/material';
import { useUser } from '../UserContext';
import { changeAuxiliaryState, getAuxiliaryContext, getAuxiliaryItems, saveAuxiliaryItem, type AuxiliaryItem, type AuxiliaryItemWrite } from '../api/auxiliary';
import ConfirmDialog from './ConfirmDialog';
import RecordActionSettings from './recordTable/RecordActionSettings';

const columns = [
  { key: 'recorded_at', label: '工作量时间' }, { key: 'user_name', label: '归属人员' },
  { key: 'detection_division_name', label: '检测部门' }, { key: 'auxiliary_work_name', label: '辅助事项' },
  { key: 'area_name', label: '区域' }, { key: 'category', label: '辅助类别' }, { key: 'quantity', label: '数量' }, { key: 'coefficient', label: '系数' },
  { key: 'workload', label: '工作量' }, { key: 'notes', label: '备注' },
];
const actions = [{ key: 'edit', label: '编辑' }, { key: 'delete', label: '删除' }, { key: 'confirm', label: '恢复' }];
const emptyItem: AuxiliaryItemWrite = { name: '', area_type_id: null, category: '', coefficient: 1, enabled: true, sort_order: 0, notes: '' };
const errorText = (reason: unknown) => reason instanceof Error ? reason.message : '操作失败，请刷新后重试';

export default function AuxiliaryItemsPanel({ onChanged }: { onChanged?: () => void }) {
  const { user, hasPermission } = useUser();
  const canManage = !!user?.is_admin && hasPermission('manage:auxiliary');
  const [items, setItems] = useState<AuxiliaryItem[]>([]);
  const [areas, setAreas] = useState<{ id: number; name: string }[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [reload, setReload] = useState(0);
  const [itemEditor, setItemEditor] = useState<AuxiliaryItem | 'new' | null>(null);
  const [itemForm, setItemForm] = useState<AuxiliaryItemWrite>({ ...emptyItem });
  const [stateAction, setStateAction] = useState<{ id: number; updated_at: string; restore: boolean } | null>(null);
  const mutationPending = useRef(false);
  const refresh = () => { setReload(value => value + 1); onChanged?.(); };

  useEffect(() => {
    if (!canManage) return;
    let active = true;
    setLoading(true);
    Promise.all([getAuxiliaryItems(true), getAuxiliaryContext()]).then(([result, context]) => { if (active) { setItems(result); setAreas(context.areas); } })
      .catch(reason => { if (active) setError(errorText(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [canManage, user?.id, reload]);

  const saveItem = async () => {
    if (!canManage || mutationPending.current) return;
    if (!itemForm.name.trim() || !Number.isFinite(itemForm.coefficient) || itemForm.coefficient < 0 || !Number.isSafeInteger(itemForm.sort_order) || itemForm.sort_order < -2147483648 || itemForm.sort_order > 2147483647) { setError('事项名称必填，系数须为非负数，排序须为有效整数'); return; }
    mutationPending.current = true; setSaving(true); setError('');
    try {
      await saveAuxiliaryItem(itemForm, itemEditor && itemEditor !== 'new' ? itemEditor.id : undefined);
      setItemEditor(null); setMessage('事项已保存；历史快照保持不变'); refresh();
    } catch (reason) { setError(errorText(reason)); }
    finally { mutationPending.current = false; setSaving(false); }
  };
  const changeState = async (reason?: string) => {
    if (!canManage || mutationPending.current) return;
    if (!stateAction || !reason?.trim()) { setError('请填写操作原因'); return; }
    mutationPending.current = true; setSaving(true); setError('');
    try {
      await changeAuxiliaryState('items', stateAction.id, stateAction.restore, stateAction.updated_at, reason.trim());
      setStateAction(null); setMessage('操作成功；历史记录保持不变'); refresh();
    } catch (failure) { setError(errorText(failure)); }
    finally { mutationPending.current = false; setSaving(false); }
  };

  if (!canManage) return <Alert severity="warning">仅系统管理员可维护辅助事项，请联系管理员配置。</Alert>;

  return <Box>
    <Box sx={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 1, mb: 1.5 }}>
      <Typography variant="h6" fontWeight={700} sx={{ flex: 1 }}>辅助事项配置</Typography>
      <Button variant="outlined" disabled={loading || saving} onClick={() => { setError(''); setReload(value => value + 1); }}>刷新</Button>
      <Button variant="contained" disabled={loading || saving} onClick={() => { setItemForm({ ...emptyItem }); setItemEditor('new'); setError(''); }}>新增公共辅助事项</Button>
    </Box>
    <Alert severity="info" sx={{ mb: 1.5 }}>每个事项独立配置区域、辅助类别和系数，类别不绑定系数。配置仅影响后续新录入，不重算历史快照；旧实验室事项保持原适用范围。</Alert>
    {message && <Alert severity="success" onClose={() => setMessage('')} sx={{ mb: 1 }}>{message}</Alert>}
    {error && !itemEditor && !stateAction && <Alert severity="error" onClose={() => setError('')} sx={{ mb: 1 }}>{error}</Alert>}
    {loading ? <Box sx={{ textAlign: 'center', p: 3 }}><CircularProgress size={26} /></Box> : <TableContainer component={Paper} variant="outlined">
      <Table size="small" sx={{ width: '100%', tableLayout: 'fixed', '& td': { overflowWrap: 'anywhere', verticalAlign: 'top' } }}>
        <TableHead><TableRow>{['事项 / 范围', '区域 / 辅助类别', '系数', '状态 / 排序', '操作'].map(title => <TableCell key={title}>{title}</TableCell>)}</TableRow></TableHead>
        <TableBody>{items.length ? items.map(item => <TableRow key={item.id}>
          <TableCell>{item.name}<Typography variant="caption" display="block" color="text.secondary">{item.scope_type === 'public' ? '公共事项' : item.group_name || '原实验室范围'}</Typography></TableCell>
          <TableCell>{item.area_name || '未分区'}<Typography variant="caption" display="block" color="text.secondary">{item.category || '未分类'}</Typography></TableCell>
          <TableCell>{item.coefficient}</TableCell>
          <TableCell>{item.deleted_at ? '已删除' : item.enabled ? '启用' : '停用'} / {item.sort_order}</TableCell>
          <TableCell><Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
            {!item.deleted_at && <Button size="small" variant="outlined" disabled={saving} onClick={() => { setItemForm({ name: item.name, area_type_id: item.area_type_id, category: item.category, coefficient: item.coefficient, enabled: item.enabled, sort_order: item.sort_order, notes: item.notes, expected_updated_at: item.updated_at }); setItemEditor(item); setError(''); }}>编辑</Button>}
            <Button size="small" variant="outlined" disabled={saving} color={item.deleted_at ? 'primary' : 'error'} onClick={() => { setStateAction({ id: item.id, updated_at: item.updated_at, restore: !!item.deleted_at }); setError(''); }}>{item.deleted_at ? '恢复' : '删除'}</Button>
          </Box></TableCell>
        </TableRow>) : <TableRow><TableCell colSpan={5} align="center" sx={{ py: 4 }}>暂无辅助事项，可新增或通过主数据模板导入</TableCell></TableRow>}</TableBody>
      </Table>
    </TableContainer>}
    <Box sx={{ mt: 2 }}><RecordActionSettings kind="auxiliary" columns={columns} actions={actions} /></Box>
    <Dialog open={!!itemEditor} onClose={saving ? undefined : () => setItemEditor(null)} fullWidth maxWidth="sm">
      <DialogTitle>{itemEditor === 'new' ? '新增公共辅助事项' : '编辑辅助事项'}</DialogTitle>
      <DialogContent>{error && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}<Box sx={{ display: 'grid', gap: 1.5, pt: 1 }}>
        <TextField label="事项名称" size="small" value={itemForm.name} inputProps={{ maxLength: 120 }} onChange={event => setItemForm(previous => ({ ...previous, name: event.target.value }))} />
        <TextField select label="区域" size="small" value={itemForm.area_type_id || 0} onChange={event => setItemForm(previous => ({ ...previous, area_type_id: Number(event.target.value) || null }))}><MenuItem value={0}>未分区</MenuItem>{itemForm.area_type_id && !areas.some(area => area.id === itemForm.area_type_id) && <MenuItem value={itemForm.area_type_id}>{itemEditor && itemEditor !== 'new' ? itemEditor.area_name || '原区域（已停用）' : '原区域（已停用）'}</MenuItem>}{areas.map(area => <MenuItem key={area.id} value={area.id}>{area.name}</MenuItem>)}</TextField>
        <Autocomplete freeSolo size="small" options={Array.from(new Set(items.map(item => item.category).filter(Boolean)))} value={itemForm.category} onChange={(_, value) => setItemForm(previous => ({ ...previous, category: value || '' }))} onInputChange={(_, value) => setItemForm(previous => ({ ...previous, category: value }))} renderInput={input => <TextField {...input} label="辅助类别（可填可选）" placeholder="未分类" inputProps={{ ...input.inputProps, maxLength: 64 }} />} />
        <TextField label="默认系数（0有效）" size="small" type="number" value={itemForm.coefficient} inputProps={{ min: 0, step: 'any' }} onChange={event => setItemForm(previous => ({ ...previous, coefficient: Number(event.target.value) }))} />
        <TextField label="排序" size="small" type="number" value={itemForm.sort_order} onChange={event => setItemForm(previous => ({ ...previous, sort_order: Number(event.target.value) }))} />
        <FormControlLabel control={<Checkbox checked={itemForm.enabled} onChange={event => setItemForm(previous => ({ ...previous, enabled: event.target.checked }))} />} label="启用" />
        <TextField label="备注" size="small" multiline minRows={2} value={itemForm.notes} inputProps={{ maxLength: 2000 }} onChange={event => setItemForm(previous => ({ ...previous, notes: event.target.value }))} />
      </Box></DialogContent>
      <DialogActions><Button onClick={() => setItemEditor(null)} disabled={saving}>取消</Button><Button variant="contained" onClick={saveItem} disabled={saving}>{saving ? '保存中…' : '保存事项'}</Button></DialogActions>
    </Dialog>
    <ConfirmDialog open={!!stateAction} title={stateAction?.restore ? '恢复辅助事项' : '删除辅助事项'} message={error || '事项变更不修改已保存的历史工作量。'} collectReason reasonLabel="操作原因（必填）" confirmText={stateAction?.restore ? '确认恢复' : '确认删除'} loading={saving} onConfirm={changeState} onCancel={() => setStateAction(null)} />
  </Box>;
}
