import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Alert, Autocomplete, Box, Button, Checkbox, Chip, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, IconButton, MenuItem, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, Tabs, Tab, TextField, Typography } from '@mui/material';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import { useUser } from '../UserContext';
import { changeAuxiliaryState, getAuxiliaryContext, getAuxiliaryItems, getAuxiliaryRecords, saveAuxiliaryRecord, type AuxiliaryContext, type AuxiliaryItem, type AuxiliaryRecord, type AuxiliaryRecordWrite } from '../api/auxiliary';
import { beijingRecordToday, recordDatePreset } from '../utils/recordDateRange';
import { recordTimeDraft } from '../utils/rdRecordFilters';
import { formatBeijingDateTimeInput, normalizeWorkloadDateTime } from '../utils/sampleWorkloadDateTime';
import { actionColorSx, cardContainerSx, formatDateByPattern } from '../utils/recordTableStyle';
import { getAdaptiveColumnWidths } from '../utils/adaptiveColumns';
import { useRecordTableStyle } from '../components/recordTable/useRecordTableStyle';
import { useCardLayout } from '../components/recordTable/useCardLayout';
import TableStyleBar from '../components/recordTable/TableStyleBar';
import RecordCardSummary from '../components/recordTable/RecordCardSummary';
import ConfirmDialog from '../components/ConfirmDialog';
import { useRecordActionLayout } from '../components/recordTable/RecordActionSettings';
import AuxiliaryItemsPanel from '../components/AuxiliaryItemsPanel';
import { placeActionColumn, sortActionItems } from '../utils/recordActionLayout';
import ManageNav from '../components/ManageNav';

const columns = [
  { key: 'recorded_at', label: '工作量时间' }, { key: 'user_name', label: '归属人员' },
  { key: 'detection_division_name', label: '检测部门' }, { key: 'auxiliary_work_name', label: '辅助事项' },
  { key: 'category', label: '子类型' }, { key: 'quantity', label: '数量' }, { key: 'coefficient', label: '系数' },
  { key: 'workload', label: '工作量' }, { key: 'notes', label: '备注' }, { key: '_action', label: '操作' },
];
const summaryRows = [['auxiliary_work_name', 'workload'], ['user_name', 'recorded_at'], ['category', 'quantity', 'coefficient'], ['detection_division_name', 'notes']];
const actions = [{ key: 'edit', label: '编辑' }, { key: 'delete', label: '删除' }, { key: 'confirm', label: '恢复' }];
const errorText = (reason: unknown) => reason instanceof Error ? reason.message : '操作失败，请刷新后重试';
const storedDateInput = (value: string) => /(?:[zZ]|[+-]\d{2}:\d{2})$/.test(value)
  ? formatBeijingDateTimeInput(new Date(value).getTime()) : recordTimeDraft(value).slice(0, 19);

export default function AuxiliaryWorkPage({ management = false }: { management?: boolean }) {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const { user, hasPermission } = useUser();
  const [tab, setTab] = useState('entry');
  const [context, setContext] = useState<AuxiliaryContext | null>(null);
  const [items, setItems] = useState<AuxiliaryItem[]>([]);
  const [records, setRecords] = useState<AuxiliaryRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [recordLoading, setRecordLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [reload, setReload] = useState(0);
  const [range, setRange] = useState(() => recordDatePreset('recent-30', beijingRecordToday()));
  const [keyword, setKeyword] = useState('');
  const [filterPerson, setFilterPerson] = useState(0);
  const [includeDeleted, setIncludeDeleted] = useState(false);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);
  const [entryForm, setEntryForm] = useState({ user_id: Number(params.get('subject_user_id')) || user?.id || 0, detection_division_id: 0, recorded_at: formatBeijingDateTimeInput() });
  const [entryDrafts, setEntryDrafts] = useState<Record<number, { quantity: string; notes: string }>>({});
  const [entryKeyword, setEntryKeyword] = useState('');
  const [entryCategory, setEntryCategory] = useState('');
  const [savingItem, setSavingItem] = useState(0);
  const [submittedItems, setSubmittedItems] = useState<number[]>([]);
  const [recordEditor, setRecordEditor] = useState<AuxiliaryRecord | null>(null);
  const [recordForm, setRecordForm] = useState({ auxiliary_work_item_id: 0, user_id: Number(params.get('subject_user_id')) || user?.id || 0, detection_division_id: 0, group_id: 0, quantity: '1', recorded_at: formatBeijingDateTimeInput(), notes: '' });
  const [stateAction, setStateAction] = useState<{ kind: 'records'; id: number; updated_at: string; restore: boolean } | null>(null);
  const styleRows = useMemo(() => Object.fromEntries(records.map(record => [`auxiliary:${record.id}`, Object.fromEntries(columns.map(column => [column.key, String(column.key === 'workload' ? record.quantity * record.coefficient : record[column.key as keyof AuxiliaryRecord] ?? '')]))])), [records]);
  const style = useRecordTableStyle('auxiliary_work_records', styleRows);
  const card = useCardLayout(style.spec);
  const actionLayout = useRecordActionLayout('auxiliary');
  const pageSize = style.spec.paging.size || 50;
  const sequence = useRef(0);
  const mutationPending = useRef(false);
  const editingRecord = recordEditor;
  const editorItems = editingRecord && !items.some(item => item.id === editingRecord.auxiliary_work_item_id) ? [...items, { id: editingRecord.auxiliary_work_item_id || 0, name: editingRecord.auxiliary_work_name, coefficient: editingRecord.coefficient, category: editingRecord.category, group_name: editingRecord.group_name, group_id: editingRecord.group_id, enabled: false, deleted_at: null, scope_type: '', sort_order: 0, notes: '', updated_at: '' }] : items;
  const selectedItem = editorItems.find(item => item.id === recordForm.auxiliary_work_item_id);
  const itemChanged = !!editingRecord && recordForm.auxiliary_work_item_id !== (editingRecord.auxiliary_work_item_id || 0);
  const currentCoefficient = editingRecord && !itemChanged ? editingRecord.coefficient : selectedItem?.coefficient;
  const refresh = () => setReload(value => value + 1);
  const today = beijingRecordToday();
  const hasDrafts = Object.values(entryDrafts).some(draft => draft.quantity !== '' || draft.notes.trim() !== '');
  const availableItems = items.filter(item => item.enabled && !item.deleted_at);
  const categories = Array.from(new Set(availableItems.map(item => item.category || '未分类')));
  const filteredItems = availableItems.filter(item => (!entryCategory || (item.category || '未分类') === entryCategory) && (!entryKeyword.trim() || `${item.name} ${item.category} ${item.group_name} ${item.notes}`.toLowerCase().includes(entryKeyword.trim().toLowerCase())));
  const leavePage = (path: string) => {
    if (hasDrafts && !window.confirm('有尚未提交的数量或备注，离开将丢失这些输入。确认离开？')) return;
    navigate(path);
  };

  useEffect(() => {
    if (!hasDrafts) return;
    const confirmUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', confirmUnload);
    return () => window.removeEventListener('beforeunload', confirmUnload);
  }, [hasDrafts]);

  useEffect(() => { setSubmittedItems([]); }, [entryForm.user_id, entryForm.detection_division_id, entryForm.recorded_at]);

  useEffect(() => {
    if (management) return;
    let active = true;
    setLoading(true); setError('');
    getAuxiliaryContext().then(async data => {
      const options = await getAuxiliaryItems(data.can_manage_items);
      if (!active) return;
      setContext(data); setItems(options);
      setEntryForm(previous => ({ ...previous, user_id: data.people.some(person => person.id === previous.user_id) ? previous.user_id : data.people[0]?.id || 0, detection_division_id: data.divisions.some(division => division.id === previous.detection_division_id) ? previous.detection_division_id : (data.divisions.length === 1 ? data.divisions[0].id : 0) }));
      if (!data.can_create && !management) setTab('records');
    }).catch(reason => { if (active) setError(errorText(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [reload, management, user?.id]);

  useEffect(() => {
    if (!context || management) return;
    const seq = ++sequence.current;
    const entry = tab === 'entry';
    if (entry && !entryForm.user_id) { setRecords([]); setTotal(0); setRecordLoading(false); return; }
    const currentRange = entry ? { start: today, end: today } : range;
    if (currentRange.start && currentRange.end && currentRange.start > currentRange.end) { setError('开始日期不能晚于结束日期'); setRecords([]); setTotal(0); setRecordLoading(false); return; }
    setRecordLoading(true); setError(''); setRecords([]);
    const recordQuery = { ...currentRange, keyword: entry ? undefined : keyword.trim() || undefined, user_id: entry ? entryForm.user_id : filterPerson || undefined, include_deleted: !entry && includeDeleted, page: page + 1, page_size: pageSize };
    getAuxiliaryRecords(recordQuery)
      .then(result => { if (seq === sequence.current) { setRecords(result.items); setTotal(result.total); } })
      .catch(reason => { if (seq === sequence.current) setError(errorText(reason)); })
      .finally(() => { if (seq === sequence.current) setRecordLoading(false); });
    return () => { sequence.current++; };
  }, [context, management, tab, today, entryForm.user_id, range, keyword, filterPerson, includeDeleted, page, pageSize, reload]);

  const openRecord = (record: AuxiliaryRecord) => {
    setRecordForm({ auxiliary_work_item_id: record.auxiliary_work_item_id || 0, user_id: record.user_id || 0, detection_division_id: record.detection_division_id || 0, group_id: record.group_id || 0, quantity: String(record.quantity), recorded_at: storedDateInput(record.recorded_at), notes: record.notes });
    setError(''); setRecordEditor(record);
  };
  const updateDraft = (id: number, field: 'quantity' | 'notes', value: string) => {
    setEntryDrafts(previous => ({ ...previous, [id]: { ...(previous[id] || { quantity: '', notes: '' }), [field]: value } }));
    setSubmittedItems(previous => previous.filter(itemId => itemId !== id));
  };
  const saveEntry = async (item: AuxiliaryItem) => {
    if (mutationPending.current) return;
    const draft = entryDrafts[item.id];
    const quantity = Number(draft?.quantity);
    const recordedAt = normalizeWorkloadDateTime(entryForm.recorded_at);
    if (!context?.can_create || !context.people.some(person => person.id === entryForm.user_id) || !context.divisions.some(division => division.id === entryForm.detection_division_id) || !Number.isSafeInteger(quantity) || quantity <= 0 || !recordedAt) { setError('请选择归属人员、检测部门，填写有效时间和正整数数量'); return; }
    if (!item.enabled || item.deleted_at) { setError('所选事项已停用，请刷新后重试'); return; }
    const body: AuxiliaryRecordWrite = { auxiliary_work_item_id: item.id, expected_item_updated_at: item.updated_at, user_id: entryForm.user_id, detection_division_id: entryForm.detection_division_id, quantity, recorded_at: `${recordedAt}+08:00`, notes: draft?.notes || '' };
    if (item.group_id) body.group_id = item.group_id;
    mutationPending.current = true; setSaving(true); setSavingItem(item.id); setError('');
    try {
      await saveAuxiliaryRecord(body);
      setEntryDrafts(previous => { const next = { ...previous }; delete next[item.id]; return next; });
      setSubmittedItems(previous => [...previous.filter(id => id !== item.id), item.id]);
      setPage(0); setMessage(`${item.name} × ${quantity} 已录入`); refresh();
    } catch (reason) { setError(errorText(reason)); }
    finally { mutationPending.current = false; setSaving(false); setSavingItem(0); }
  };
  const saveRecord = async () => {
    if (mutationPending.current || !editingRecord) return;
    const quantity = Number(recordForm.quantity);
    const recordedAt = normalizeWorkloadDateTime(recordForm.recorded_at);
    if (!Number.isSafeInteger(quantity) || quantity <= 0 || !recordedAt) { setError('请填写有效时间和正整数数量'); return; }
    if (itemChanged && (!selectedItem || !selectedItem.enabled || selectedItem.deleted_at)) { setError('所选事项已停用，请重新选择'); return; }
    if (itemChanged && !window.confirm('更换事项将按所选事项当前配置更新名称、分类和系数快照，并改变本条历史工作量。确认更换？')) return;
    const body: AuxiliaryRecordWrite = { quantity, recorded_at: `${recordedAt}+08:00`, notes: recordForm.notes };
    if (recordForm.auxiliary_work_item_id > 0) body.auxiliary_work_item_id = recordForm.auxiliary_work_item_id;
    if (itemChanged) body.expected_item_updated_at = selectedItem!.updated_at;
    body.expected_updated_at = editingRecord.updated_at;
    mutationPending.current = true; setSaving(true); setError('');
    try { await saveAuxiliaryRecord(body, editingRecord.id); setRecordEditor(null); setMessage('记录已更新'); refresh(); }
    catch (reason) { setError(errorText(reason)); }
    finally { mutationPending.current = false; setSaving(false); }
  };
  const changeState = async (reason?: string) => {
    if (mutationPending.current) return;
    if (!stateAction || !reason?.trim()) { setError('请填写操作原因'); return; }
    mutationPending.current = true; setSaving(true); setError('');
    try { await changeAuxiliaryState(stateAction.kind, stateAction.id, stateAction.restore, stateAction.updated_at, reason.trim()); setStateAction(null); setMessage('操作成功'); refresh(); }
    catch (failure) { setError(errorText(failure)); }
    finally { mutationPending.current = false; setSaving(false); }
  };
  const recordActions = (record: AuxiliaryRecord) => <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap', '& .MuiButton-root': { fontSize: style.spec.table.actionFontSize, width: style.spec.table.actionButtonWidth || undefined, minWidth: 0, maxWidth: '100%', minHeight: card ? 40 : 28, px: 0.75 } }}>
    {record.linked_work_record_id ? <Typography variant="caption" color="text.secondary">历史关联记录，请在原工作量入口维护</Typography> : sortActionItems(actions, actionLayout.config.button_order).map(action => {
      const allowed = action.key === 'edit' ? record.can_edit : action.key === 'delete' ? record.can_delete : record.can_restore;
      if (!allowed) return null;
      return <Button key={action.key} size="small" variant="outlined" disabled={saving} sx={actionColorSx(style.spec.actionColors, action.key)} onClick={() => action.key === 'edit' ? openRecord(record) : setStateAction({ kind: 'records', id: record.id, updated_at: record.updated_at, restore: action.key === 'confirm' })}>{action.label}</Button>;
    })}
  </Box>;
  const valueFor = (record: AuxiliaryRecord, key: string): React.ReactNode => {
    if (key === '_action') return recordActions(record);
    if (key === 'workload') return (record.quantity * record.coefficient).toLocaleString('zh-CN', { maximumFractionDigits: 8 });
    if (key === 'recorded_at') return formatDateByPattern(record.recorded_at, style.spec.columns.recorded_at?.dateFormat) || record.recorded_at.replace('T', ' ').slice(0, 19);
    if (key === 'auxiliary_work_name') return <>{record.auxiliary_work_name}{record.deleted_at && <Typography variant="caption" color="error">（已删除）</Typography>}</>;
    return String(record[key as keyof AuxiliaryRecord] ?? '') || (key === 'category' ? '未分类' : '—');
  };
  const visibleColumns = placeActionColumn(columns.filter(column => !style.spec.hiddenColumns.includes(column.key)).map(column => column.key), actionLayout.config.before_column).filter(key => !style.spec.hiddenColumns.includes(key)).map(key => columns.find(column => column.key === key)!);
  const widths = getAdaptiveColumnWidths(records, visibleColumns.map(column => ({ key: column.key, header: column.label, fixed: style.spec.columns[column.key]?.width, min: 64, max: column.key === '_action' ? 180 : 160, getValue: record => column.key === 'workload' ? record.quantity * record.coefficient : record[column.key as keyof AuxiliaryRecord] })));

  if (management) return <Box sx={{ display: 'flex', minWidth: 0, gap: 2 }}><ManageNav activeKey="auxiliary" /><Box sx={{ minWidth: 0, flex: 1 }}><AuxiliaryItemsPanel /></Box></Box>;

  return <Box sx={{ display: 'flex', minWidth: 0, gap: 2 }}>
    <Box sx={{ minWidth: 0, flex: 1 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1.5, flexWrap: 'wrap' }}><Button variant="outlined" className="labflow-back-button" onClick={() => leavePage(`/workload${user?.is_analysis_public_account && entryForm.user_id ? `?subject_user_id=${entryForm.user_id}` : ''}`)}>返回</Button><Box sx={{ flex: 1 }}><Typography variant="h5" fontWeight={700}>辅助工作录入</Typography><Typography variant="caption" color="text.secondary">选择辅助事项并录入数量</Typography></Box>{hasPermission('stats:portal:workload') && <Button size="small" variant="outlined" onClick={() => leavePage('/stats/workload')}>查看汇总</Button>}</Box>
      {message && <Alert severity="success" onClose={() => setMessage('')} sx={{ mb: 1 }}>{message}</Alert>}
      {error && !recordEditor && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}
      {loading && !context ? <CircularProgress /> : context && <>
        <Tabs value={tab} onChange={(_, value) => { setTab(value); setPage(0); setError(''); }} sx={{ mb: 2 }}><Tab value="entry" label="录入工作量" disabled={!context.can_create} /><Tab value="records" label="辅助记录" /></Tabs>
        {tab === 'entry' && <>
          <Box sx={{ display: 'flex', gap: 2, mb: 2, flexWrap: 'wrap', alignItems: 'center', '& .MuiOutlinedInput-root': { borderRadius: '2px' } }}>
            <TextField select label="工作量归属人员" size="small" disabled={saving || context.people.length < 2} value={entryForm.user_id} onChange={event => { setEntryForm(previous => ({ ...previous, user_id: Number(event.target.value) })); setPage(0); }} sx={{ width: { xs: '100%', sm: 160 } }}>{context.people.map(person => <MenuItem key={person.id} value={person.id}>{person.username}</MenuItem>)}</TextField>
            <TextField select label="检测部门" size="small" disabled={saving || context.divisions.length < 2} value={entryForm.detection_division_id} onChange={event => setEntryForm(previous => ({ ...previous, detection_division_id: Number(event.target.value) }))} sx={{ width: { xs: '100%', sm: 180 } }}><MenuItem value={0}>请选择部门</MenuItem>{context.divisions.map(division => <MenuItem key={division.id} value={division.id}>{division.name}</MenuItem>)}</TextField>
            <TextField label="日期时间（北京时间）" type="datetime-local" size="small" disabled={saving} value={entryForm.recorded_at} onChange={event => setEntryForm(previous => ({ ...previous, recorded_at: event.target.value }))} InputLabelProps={{ shrink: true }} inputProps={{ step: 1 }} sx={{ width: { xs: '100%', sm: 240 } }} />
          </Box>
          <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center', mb: 2 }}>
            <TextField label="搜索辅助事项" size="small" value={entryKeyword} onChange={event => setEntryKeyword(event.target.value)} sx={{ width: { xs: '100%', sm: 220 } }} />
            <Chip label={`全部 (${availableItems.length})`} color={!entryCategory ? 'primary' : 'default'} onClick={() => setEntryCategory('')} sx={{ borderRadius: '2px' }} />
            {categories.map(category => <Chip key={category} label={`${category} (${availableItems.filter(item => (item.category || '未分类') === category).length})`} color={entryCategory === category ? 'primary' : 'default'} onClick={() => setEntryCategory(category)} sx={{ borderRadius: '2px' }} />)}
          </Box>
          {filteredItems.map(item => {
            const draft = entryDrafts[item.id] || { quantity: '', notes: '' };
            const submitted = submittedItems.includes(item.id);
            const submitting = savingItem === item.id;
            const invalid = !Number.isSafeInteger(Number(draft.quantity)) || Number(draft.quantity) <= 0;
            return <Box key={item.id} sx={{ display: 'flex', alignItems: 'center', gap: 1.5, py: 1.5, px: 2, mb: 1, borderRadius: '2px', flexDirection: { xs: 'column', sm: 'row' }, background: submitted ? 'linear-gradient(145deg,#e8f5e9,#f1f8e9)' : 'linear-gradient(145deg,#ffffff,#fafafa)', border: '1px solid', borderColor: submitted ? '#a5d6a7' : 'rgba(0,0,0,0.06)', borderLeft: '4px solid', borderLeftColor: submitted ? '#43a047' : '#1e88e5', boxShadow: '0 2px 12px rgba(0,0,0,0.04)' }}>
              <Box sx={{ flex: 1, minWidth: 0, width: { xs: '100%', sm: 'auto' } }}><Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}><Typography fontWeight={600} sx={{ overflowWrap: 'anywhere', fontSize: style.spec.body.fontSize }}>{item.name}</Typography><Chip label={`工作量系数 ${item.coefficient}`} size="small" variant="outlined" color="primary" sx={{ height: 22, borderRadius: '2px', fontSize: style.spec.table.auxFontSize }} /><Chip label={item.category || '未分类'} size="small" sx={{ height: 20, borderRadius: '2px', fontSize: style.spec.table.auxFontSize }} />{item.group_name && <Chip label={item.group_name} size="small" variant="outlined" sx={{ height: 20, borderRadius: '2px', fontSize: style.spec.table.auxFontSize }} />}</Box>{item.notes && <Typography color="text.secondary" sx={{ mt: 0.3, fontSize: style.spec.table.auxFontSize, overflowWrap: 'anywhere' }}>{item.notes}</Typography>}</Box>
              <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr) minmax(72px,0.5fr) 40px', sm: 'minmax(100px,180px) 86px 40px' }, alignItems: 'center', gap: 1, width: { xs: '100%', sm: 'auto' }, flexShrink: 0 }}>
                <TextField label="备注（选填）" size="small" value={draft.notes} disabled={saving} onChange={event => updateDraft(item.id, 'notes', event.target.value)} inputProps={{ maxLength: 2000 }} sx={{ minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: '2px' } }} />
                <TextField label="数量" type="number" size="small" value={draft.quantity} disabled={saving} onChange={event => updateDraft(item.id, 'quantity', event.target.value)} onKeyDown={event => { if (event.key === 'Enter') void saveEntry(item); }} inputProps={{ min: 1, step: 1, 'aria-label': `${item.name}数量`, style: { textAlign: 'center', minWidth: 0 } }} sx={{ minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: '2px', '& fieldset': { borderColor: '#1976d2' } } }} />
                <IconButton title={submitted ? '已录入' : '确认录入'} aria-label={`${item.name}${submitted ? '已录入' : '确认录入'}`} onClick={() => void saveEntry(item)} disabled={saving || invalid || !context.can_create} sx={{ borderRadius: '50%', bgcolor: submitted ? '#e8f5e9' : '#e3f2fd', color: submitted ? '#43a047' : '#1e88e5', '&:disabled': { color: submitted ? '#43a047' : 'rgba(0,0,0,0.2)', bgcolor: submitted ? '#e8f5e9' : 'transparent' }, ...actionColorSx(style.spec.actionColors, 'workload', submitting ? 'loading' : submitted ? 'completed' : invalid ? 'disabled' : 'idle') }}>{submitting ? <CircularProgress size={24} /> : submitted ? <CheckCircleIcon /> : <CheckCircleOutlineIcon />}</IconButton>
              </Box>
            </Box>;
          })}
          {!filteredItems.length && <Typography color="text.secondary" textAlign="center" sx={{ py: 3 }}>{availableItems.length ? '没有匹配的辅助事项' : '暂无可录入辅助事项，请由管理员在主数据配置中维护'}</Typography>}
          <Typography color="text.secondary" sx={{ fontSize: style.spec.table.auxFontSize, mt: 1, mb: 3 }}>工作量 = 数量 × 保存的系数；归属人员与操作账号分别保存。事项配置调整不重算历史记录。</Typography>
          <Typography variant="h6" fontWeight={700} sx={{ mb: 1 }}>今日记录 <Typography component="span" color="text.secondary" sx={{ fontSize: style.spec.table.auxFontSize }}>共 {total} 条</Typography></Typography>
        </>}
        {(tab === 'entry' || tab === 'records') && <>
          {tab === 'records' && <>
          <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mb: 1.5 }}><TextField size="small" type="date" label="开始日期" InputLabelProps={{ shrink: true }} value={range.start} onChange={event => { setRange(previous => ({ ...previous, start: event.target.value })); setPage(0); }} /><TextField size="small" type="date" label="结束日期" InputLabelProps={{ shrink: true }} value={range.end} onChange={event => { setRange(previous => ({ ...previous, end: event.target.value })); setPage(0); }} /><Button variant="outlined" onClick={() => { setRange(recordDatePreset('recent-30', beijingRecordToday())); setPage(0); }}>近30天</Button><Button variant="outlined" onClick={() => { setRange({ start: '', end: '' }); setPage(0); }}>全部历史</Button><TextField size="small" label="搜索事项、备注或人员" value={keyword} onChange={event => { setKeyword(event.target.value); setPage(0); }} /><Autocomplete size="small" sx={{ minWidth: 170 }} options={context.record_people || context.people} value={(context.record_people || context.people).find(person => person.id === filterPerson) || null} getOptionLabel={person => person.username} onChange={(_, value) => { setFilterPerson(value?.id || 0); setPage(0); }} renderInput={input => <TextField {...input} label="归属人员" />} />{(context.can_edit_own || context.can_manage_records) && <FormControlLabel control={<Checkbox checked={includeDeleted} onChange={event => { setIncludeDeleted(event.target.checked); setPage(0); }} />} label="含已删除" />}</Box>
          <Typography variant="caption" color="text.secondary">筛选覆盖当前日期及权限范围内全部辅助记录，每页条数只控制显示。</Typography>
          </>}
          <TableStyleBar api={style} columns={columns} rows={records.map(record => ({ key: `auxiliary:${record.id}`, label: record.auxiliary_work_name }))} dateColumns={['recorded_at']} cardLayout={card} hasCardLayout hasActionStyle actionColorKeys={['workload', 'edit', 'delete', 'confirm']} onPageSizeChange={() => setPage(0)} summaryDefaultRows={summaryRows} />
          {recordLoading ? <Box sx={{ p: 3, textAlign: 'center' }}><CircularProgress size={26} /></Box> : !records.length ? <Typography sx={{ p: 3, textAlign: 'center' }}>暂无符合条件的记录</Typography> : card ? <Box sx={{ display: 'grid', gap: 1 }}>{records.map((record, index) => <Paper key={`auxiliary:${record.id}`} sx={{ ...cardContainerSx(style.spec), ...style.resolver.rowSx(`auxiliary:${record.id}`), p: 1 }}><RecordCardSummary card={style.spec.card} defaultRows={summaryRows} auxFontSize={style.spec.table.auxFontSize} fields={visibleColumns.filter(column => column.key !== '_action').map(column => ({ key: column.key, label: column.label, value: <Box component="span" sx={style.resolver.visualSx(column.key, `auxiliary:${record.id}`, index)}>{valueFor(record, column.key)}</Box> }))} />{!style.spec.hiddenColumns.includes('_action') && recordActions(record)}</Paper>)}</Box> : <TableContainer component={Paper} variant="outlined" sx={{ maxHeight: '68vh', overflowX: 'hidden' }}><Table size="small" stickyHeader={style.spec.table.stickyHeader} sx={{ ...style.tableSx, tableLayout: 'fixed', width: '100%', minWidth: 0 }}><colgroup>{visibleColumns.map(column => <col key={column.key} style={{ width: widths[column.key].desktop }} />)}</colgroup><TableHead><TableRow>{visibleColumns.map(column => <TableCell key={column.key} sx={{ ...style.resolver.headerSx(column.key), width: widths[column.key].desktop }}><Box sx={style.resolver.headerFlowSx(column.key)}>{column.label}</Box></TableCell>)}</TableRow></TableHead><TableBody>{records.map((record, index) => <TableRow key={`auxiliary:${record.id}`}>{visibleColumns.map(column => <TableCell key={column.key} sx={style.resolver.cellSx(column.key, `auxiliary:${record.id}`, index)}><Box sx={{ maxHeight: 160, overflow: 'auto', overflowWrap: 'anywhere', ...style.resolver.flowSx(column.key, `auxiliary:${record.id}`, index) }}>{valueFor(record, column.key)}</Box></TableCell>)}</TableRow>)}</TableBody></Table></TableContainer>}
          <TablePagination component="div" count={total} page={page} rowsPerPage={pageSize} rowsPerPageOptions={[20, 50, 100, 200]} onPageChange={(_, value) => setPage(value)} onRowsPerPageChange={event => { style.setPageSize(Number(event.target.value)); setPage(0); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} />
        </>}
      </>}
      <Dialog open={!!recordEditor} onClose={saving ? undefined : () => setRecordEditor(null)} fullWidth maxWidth="sm"><DialogTitle>编辑辅助记录</DialogTitle><DialogContent>
        {error && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,minmax(0,1fr))' }, gap: 1.5, pt: 1 }}>
          <Autocomplete sx={{ gridColumn: '1 / -1' }} disabled={!!editingRecord && !editingRecord.auxiliary_work_item_id} options={editorItems.filter(item => (!item.deleted_at && item.enabled) || item.id === editingRecord?.auxiliary_work_item_id)} getOptionLabel={item => `${item.name}${item.group_name ? `（${item.group_name}）` : ''}`} value={selectedItem || null} isOptionEqualToValue={(a, b) => a.id === b.id} onChange={(_, item) => setRecordForm(previous => ({ ...previous, auxiliary_work_item_id: item?.id || 0 }))} renderInput={input => <TextField {...input} label="辅助事项（输入关键词筛选）" required size="small" />} />
          <TextField select label="工作量归属人员" size="small" disabled={!!editingRecord || (context?.people.length || 0) < 2} value={recordForm.user_id} onChange={event => setRecordForm(previous => ({ ...previous, user_id: Number(event.target.value) }))}>{editingRecord && !context?.people.some(person => person.id === editingRecord.user_id) && <MenuItem value={editingRecord.user_id || 0}>{editingRecord.user_name}</MenuItem>}{context?.people.map(person => <MenuItem key={person.id} value={person.id}>{person.username}</MenuItem>)}</TextField>
          <TextField select label="检测部门" size="small" disabled={!!editingRecord} value={recordForm.detection_division_id} onChange={event => setRecordForm(previous => ({ ...previous, detection_division_id: Number(event.target.value) }))}><MenuItem value={0}>请选择部门</MenuItem>{editingRecord?.detection_division_id && !context?.divisions.some(division => division.id === editingRecord.detection_division_id) && <MenuItem value={editingRecord.detection_division_id}>{editingRecord.detection_division_name}</MenuItem>}{context?.divisions.map(division => <MenuItem key={division.id} value={division.id}>{division.name}</MenuItem>)}</TextField>
          <TextField label="工作量时间（北京时间）" size="small" type="datetime-local" value={recordForm.recorded_at} InputLabelProps={{ shrink: true }} inputProps={{ step: 1 }} onChange={event => setRecordForm(previous => ({ ...previous, recorded_at: event.target.value }))} />
          <TextField label="数量（正整数）" size="small" type="number" value={recordForm.quantity} inputProps={{ min: 1, step: 1 }} onChange={event => setRecordForm(previous => ({ ...previous, quantity: event.target.value }))} />
          <TextField label={editingRecord && !itemChanged ? '已保存系数' : '当前事项系数'} size="small" value={currentCoefficient ?? '—'} InputProps={{ readOnly: true }} /><TextField label="工作量预览" size="small" value={Number.isFinite(Number(recordForm.quantity)) && currentCoefficient != null ? (Number(recordForm.quantity) * currentCoefficient).toLocaleString('zh-CN', { maximumFractionDigits: 8 }) : '—'} InputProps={{ readOnly: true }} />
          <TextField sx={{ gridColumn: '1 / -1' }} label="备注" size="small" multiline minRows={2} value={recordForm.notes} inputProps={{ maxLength: 2000 }} onChange={event => setRecordForm(previous => ({ ...previous, notes: event.target.value }))} />
        </Box><Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 1 }}>当前操作账号：{user?.username}。{editingRecord ? '仅改数量、日期或备注时保留历史系数和分类。' : '归属人员与实际操作账号分别保存，不录入金额。'}</Typography>
      </DialogContent><DialogActions><Button onClick={refresh} disabled={saving || loading}>刷新可选事项</Button><Button onClick={() => setRecordEditor(null)} disabled={saving}>取消</Button><Button variant="contained" onClick={saveRecord} disabled={saving || loading}>{saving ? '保存中…' : '保存修改'}</Button></DialogActions></Dialog>
      <ConfirmDialog open={!!stateAction} title={stateAction?.restore ? '恢复辅助数据' : '删除辅助数据'} message={error || '此操作会改变人员统计，请核对后填写原因。'} collectReason reasonLabel="操作原因（必填）" confirmText={stateAction?.restore ? '确认恢复' : '确认删除'} loading={saving} onConfirm={changeState} onCancel={() => setStateAction(null)} />
    </Box>
  </Box>;
}
