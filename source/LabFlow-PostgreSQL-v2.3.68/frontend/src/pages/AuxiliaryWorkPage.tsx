import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Alert, Autocomplete, Box, Button, Checkbox, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, MenuItem, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, Tabs, Tab, TextField, Typography } from '@mui/material';
import { useUser } from '../UserContext';
import { changeAuxiliaryState, getAuxiliaryContext, getAuxiliaryItems, getAuxiliaryRecords, saveAuxiliaryItem, saveAuxiliaryRecord, type AuxiliaryContext, type AuxiliaryItem, type AuxiliaryItemWrite, type AuxiliaryRecord, type AuxiliaryRecordWrite } from '../api/auxiliary';
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
import WorkloadAuxiliaryConfig from '../components/WorkloadAuxiliaryConfig';
import RecordActionSettings, { useRecordActionLayout } from '../components/recordTable/RecordActionSettings';
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
const emptyItem: AuxiliaryItemWrite = { name: '', category: '', coefficient: 1, enabled: true, sort_order: 0, notes: '' };
const errorText = (reason: unknown) => reason instanceof Error ? reason.message : '操作失败，请刷新后重试';
const storedDateInput = (value: string) => /(?:[zZ]|[+-]\d{2}:\d{2})$/.test(value)
  ? formatBeijingDateTimeInput(new Date(value).getTime()) : recordTimeDraft(value).slice(0, 19);

export default function AuxiliaryWorkPage({ management = false }: { management?: boolean }) {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const { user, hasPermission } = useUser();
  const [tab, setTab] = useState(management ? 'items' : 'entry');
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
  const [recordEditor, setRecordEditor] = useState<AuxiliaryRecord | 'new' | null>(null);
  const [recordForm, setRecordForm] = useState({ auxiliary_work_item_id: 0, user_id: Number(params.get('subject_user_id')) || user?.id || 0, detection_division_id: 0, group_id: 0, quantity: '1', recorded_at: formatBeijingDateTimeInput(), notes: '' });
  const [itemEditor, setItemEditor] = useState<AuxiliaryItem | 'new' | null>(null);
  const [itemForm, setItemForm] = useState<AuxiliaryItemWrite>({ ...emptyItem });
  const [stateAction, setStateAction] = useState<{ kind: 'items' | 'records'; id: number; updated_at: string; restore: boolean } | null>(null);
  const styleRows = useMemo(() => Object.fromEntries(records.map(record => [`auxiliary:${record.id}`, Object.fromEntries(columns.map(column => [column.key, String(column.key === 'workload' ? record.quantity * record.coefficient : record[column.key as keyof AuxiliaryRecord] ?? '')]))])), [records]);
  const style = useRecordTableStyle('auxiliary_work_records', styleRows);
  const card = useCardLayout(style.spec);
  const actionLayout = useRecordActionLayout('auxiliary');
  const pageSize = style.spec.paging.size || 50;
  const sequence = useRef(0);
  const mutationPending = useRef(false);
  const editingRecord = recordEditor && recordEditor !== 'new' ? recordEditor : null;
  const editorItems = editingRecord && !items.some(item => item.id === editingRecord.auxiliary_work_item_id) ? [...items, { id: editingRecord.auxiliary_work_item_id || 0, name: editingRecord.auxiliary_work_name, coefficient: editingRecord.coefficient, category: editingRecord.category, group_name: editingRecord.group_name, group_id: editingRecord.group_id, enabled: false, deleted_at: null, scope_type: '', sort_order: 0, notes: '', updated_at: '' }] : items;
  const selectedItem = editorItems.find(item => item.id === recordForm.auxiliary_work_item_id);
  const itemChanged = !!editingRecord && recordForm.auxiliary_work_item_id !== (editingRecord.auxiliary_work_item_id || 0);
  const currentCoefficient = editingRecord && !itemChanged ? editingRecord.coefficient : selectedItem?.coefficient;
  const refresh = () => setReload(value => value + 1);

  useEffect(() => {
    let active = true;
    setLoading(true); setError('');
    getAuxiliaryContext().then(async data => {
      const options = await getAuxiliaryItems(data.can_manage_items);
      if (!active) return;
      setContext(data); setItems(options);
      setRecordForm(previous => ({ ...previous, user_id: data.people.some(person => person.id === previous.user_id) ? previous.user_id : data.people[0]?.id || 0, detection_division_id: previous.detection_division_id || (data.divisions.length === 1 ? data.divisions[0].id : 0) }));
      if (!data.can_create && !management) setTab('records');
    }).catch(reason => { if (active) setError(errorText(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [reload, management, user?.id]);

  useEffect(() => {
    if (!context || tab !== 'records') return;
    const seq = ++sequence.current;
    if (range.start && range.end && range.start > range.end) { setError('开始日期不能晚于结束日期'); return; }
    setRecordLoading(true); setError(''); setRecords([]);
    getAuxiliaryRecords({ ...range, keyword: keyword.trim() || undefined, user_id: filterPerson || undefined, include_deleted: includeDeleted, page: page + 1, page_size: pageSize })
      .then(result => { if (seq === sequence.current) { setRecords(result.items); setTotal(result.total); } })
      .catch(reason => { if (seq === sequence.current) setError(errorText(reason)); })
      .finally(() => { if (seq === sequence.current) setRecordLoading(false); });
    return () => { sequence.current++; };
  }, [context, tab, range, keyword, filterPerson, includeDeleted, page, pageSize, reload]);

  const openRecord = (record: AuxiliaryRecord | 'new') => {
    if (record === 'new') setRecordForm(previous => ({ ...previous, quantity: '1', recorded_at: formatBeijingDateTimeInput(), notes: '' }));
    else setRecordForm({ auxiliary_work_item_id: record.auxiliary_work_item_id || 0, user_id: record.user_id || 0, detection_division_id: record.detection_division_id || 0, group_id: record.group_id || 0, quantity: String(record.quantity), recorded_at: storedDateInput(record.recorded_at), notes: record.notes });
    setError(''); setRecordEditor(record);
  };
  const saveRecord = async () => {
    if (mutationPending.current) return;
    const quantity = Number(recordForm.quantity);
    const recordedAt = normalizeWorkloadDateTime(recordForm.recorded_at);
    if (!Number.isSafeInteger(quantity) || quantity <= 0 || !recordedAt || (!editingRecord && (!recordForm.auxiliary_work_item_id || !recordForm.user_id || !recordForm.detection_division_id))) { setError('请选择事项、归属人员、检测部门，填写有效时间和正整数数量'); return; }
    if ((!editingRecord || itemChanged) && (!selectedItem || !selectedItem.enabled || selectedItem.deleted_at)) { setError('所选事项已停用，请重新选择'); return; }
    if (itemChanged && !window.confirm('更换事项将按所选事项当前配置更新名称、分类和系数快照，并改变本条历史工作量。确认更换？')) return;
    const body: AuxiliaryRecordWrite = { quantity, recorded_at: `${recordedAt}+08:00`, notes: recordForm.notes };
    if (recordForm.auxiliary_work_item_id > 0) body.auxiliary_work_item_id = recordForm.auxiliary_work_item_id;
    if (!editingRecord || itemChanged) body.expected_item_updated_at = selectedItem!.updated_at;
    if (editingRecord) body.expected_updated_at = editingRecord.updated_at;
    else { body.user_id = recordForm.user_id; body.detection_division_id = recordForm.detection_division_id; if (selectedItem?.group_id) body.group_id = selectedItem.group_id; }
    mutationPending.current = true; setSaving(true); setError('');
    try { await saveAuxiliaryRecord(body, editingRecord?.id); setRecordEditor(null); setMessage(editingRecord ? '记录已更新' : '辅助工作量已录入'); refresh(); }
    catch (reason) { setError(errorText(reason)); }
    finally { mutationPending.current = false; setSaving(false); }
  };
  const saveItem = async () => {
    if (mutationPending.current) return;
    if (!itemForm.name.trim() || !Number.isFinite(itemForm.coefficient) || itemForm.coefficient < 0 || !Number.isSafeInteger(itemForm.sort_order)) { setError('事项名称必填，系数须为非负数，排序须为整数'); return; }
    mutationPending.current = true; setSaving(true); setError('');
    try { await saveAuxiliaryItem(itemForm, itemEditor && itemEditor !== 'new' ? itemEditor.id : undefined); setItemEditor(null); setMessage('事项已保存；历史快照保持不变'); refresh(); }
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

  return <Box sx={{ display: 'flex', minWidth: 0, gap: 2 }}>
    {management && <ManageNav activeKey="auxiliary" />}
    <Box sx={{ minWidth: 0, flex: 1 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1.5 }}><Button variant="outlined" className="labflow-back-button" onClick={() => navigate(management ? '/manage' : '/workload')}>返回</Button><Typography variant="h5" fontWeight={700} sx={{ flex: 1 }}>辅助工作</Typography>{user?.is_admin && <WorkloadAuxiliaryConfig onSaved={refresh} />}</Box>
      <Typography color="text.secondary" sx={{ fontSize: style.spec.table.auxFontSize, mb: 1.5 }}>辅助工作量 = 数量 × 保存的系数；计入人员总工作量，不分摊实验室、项目、仪器或金额。历史方法辅助记录仍在原入口维护。</Typography>
      {hasPermission('stats:portal:workload') && <Button size="small" variant="outlined" sx={{ mb: 1.5 }} onClick={() => navigate('/stats/workload')}>查看历史与当前汇总（类型 → 辅助工作）</Button>}
      {message && <Alert severity="success" onClose={() => setMessage('')} sx={{ mb: 1 }}>{message}</Alert>}
      {error && !recordEditor && !itemEditor && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}
      {loading && !context ? <CircularProgress /> : context && <>
        <Tabs value={tab} onChange={(_, value) => { setTab(value); setError(''); }} sx={{ mb: 2 }} variant="fullWidth"><Tab value="entry" label="录入工作量" disabled={!context.can_create} /><Tab value="records" label="辅助记录" />{context.can_manage_items && <Tab value="items" label="事项管理" />}</Tabs>
        {tab === 'entry' && <Paper variant="outlined" sx={{ p: 2 }}><Typography fontWeight={600} sx={{ mb: 1 }}>独立录入辅助工作</Typography><Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>无需选择研发项目、检测方法或仪器；归属人员和部门均按当前账号授权核验。</Typography><Button variant="contained" onClick={() => openRecord('new')} disabled={!context.can_create}>录入辅助工作量</Button></Paper>}
        {tab === 'records' && <>
          <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mb: 1.5 }}><TextField size="small" type="date" label="开始日期" InputLabelProps={{ shrink: true }} value={range.start} onChange={event => { setRange(previous => ({ ...previous, start: event.target.value })); setPage(0); }} /><TextField size="small" type="date" label="结束日期" InputLabelProps={{ shrink: true }} value={range.end} onChange={event => { setRange(previous => ({ ...previous, end: event.target.value })); setPage(0); }} /><Button variant="outlined" onClick={() => { setRange(recordDatePreset('recent-30', beijingRecordToday())); setPage(0); }}>近30天</Button><Button variant="outlined" onClick={() => { setRange({ start: '', end: '' }); setPage(0); }}>全部历史</Button><TextField size="small" label="搜索事项、备注或人员" value={keyword} onChange={event => { setKeyword(event.target.value); setPage(0); }} /><Autocomplete size="small" sx={{ minWidth: 170 }} options={context.record_people || context.people} value={(context.record_people || context.people).find(person => person.id === filterPerson) || null} getOptionLabel={person => person.username} onChange={(_, value) => { setFilterPerson(value?.id || 0); setPage(0); }} renderInput={input => <TextField {...input} label="归属人员" />} />{(context.can_edit_own || context.can_manage_records) && <FormControlLabel control={<Checkbox checked={includeDeleted} onChange={event => { setIncludeDeleted(event.target.checked); setPage(0); }} />} label="含已删除" />}</Box>
          <Typography variant="caption" color="text.secondary">筛选覆盖当前日期及权限范围内全部辅助记录，每页条数只控制显示。</Typography>
          <TableStyleBar api={style} columns={columns} rows={records.map(record => ({ key: `auxiliary:${record.id}`, label: record.auxiliary_work_name }))} dateColumns={['recorded_at']} cardLayout={card} hasCardLayout hasActionStyle actionColorKeys={['edit', 'delete', 'confirm']} onPageSizeChange={() => setPage(0)} summaryDefaultRows={summaryRows} />
          {recordLoading ? <Box sx={{ p: 3, textAlign: 'center' }}><CircularProgress size={26} /></Box> : !records.length ? <Typography sx={{ p: 3, textAlign: 'center' }}>暂无符合条件的记录</Typography> : card ? <Box sx={{ display: 'grid', gap: 1 }}>{records.map((record, index) => <Paper key={`auxiliary:${record.id}`} sx={{ ...cardContainerSx(style.spec), ...style.resolver.rowSx(`auxiliary:${record.id}`), p: 1 }}><RecordCardSummary card={style.spec.card} defaultRows={summaryRows} auxFontSize={style.spec.table.auxFontSize} fields={visibleColumns.filter(column => column.key !== '_action').map(column => ({ key: column.key, label: column.label, value: <Box component="span" sx={style.resolver.visualSx(column.key, `auxiliary:${record.id}`, index)}>{valueFor(record, column.key)}</Box> }))} />{!style.spec.hiddenColumns.includes('_action') && recordActions(record)}</Paper>)}</Box> : <TableContainer component={Paper} variant="outlined" sx={{ maxHeight: '68vh', overflowX: 'hidden' }}><Table size="small" stickyHeader={style.spec.table.stickyHeader} sx={{ ...style.tableSx, tableLayout: 'fixed', width: '100%', minWidth: 0 }}><colgroup>{visibleColumns.map(column => <col key={column.key} style={{ width: widths[column.key].desktop }} />)}</colgroup><TableHead><TableRow>{visibleColumns.map(column => <TableCell key={column.key} sx={{ ...style.resolver.headerSx(column.key), width: widths[column.key].desktop }}><Box sx={style.resolver.headerFlowSx(column.key)}>{column.label}</Box></TableCell>)}</TableRow></TableHead><TableBody>{records.map((record, index) => <TableRow key={`auxiliary:${record.id}`}>{visibleColumns.map(column => <TableCell key={column.key} sx={style.resolver.cellSx(column.key, `auxiliary:${record.id}`, index)}><Box sx={{ maxHeight: 160, overflow: 'auto', overflowWrap: 'anywhere', ...style.resolver.flowSx(column.key, `auxiliary:${record.id}`, index) }}>{valueFor(record, column.key)}</Box></TableCell>)}</TableRow>)}</TableBody></Table></TableContainer>}
          <TablePagination component="div" count={total} page={page} rowsPerPage={pageSize} rowsPerPageOptions={[20, 50, 100, 200]} onPageChange={(_, value) => setPage(value)} onRowsPerPageChange={event => { style.setPageSize(Number(event.target.value)); setPage(0); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} />
        </>}
        {tab === 'items' && context.can_manage_items && <>
          <Box sx={{ display: 'flex', gap: 1, mb: 1 }}><Button variant="contained" onClick={() => { setItemForm({ ...emptyItem }); setItemEditor('new'); setError(''); }}>新增公共辅助事项</Button></Box>
          <Alert severity="info" sx={{ mb: 1 }}>配置仅影响后续新录入；修改名称、系数、分类或停用事项均不重算历史记录。旧实验室事项保持原适用范围。</Alert>
          <TableContainer component={Paper} variant="outlined"><Table size="small" sx={{ width: '100%', tableLayout: 'fixed', '& td': { overflowWrap: 'anywhere', verticalAlign: 'top' } }}><TableHead><TableRow>{['事项 / 范围', '子类型 / 系数', '状态 / 排序', '操作'].map(title => <TableCell key={title}>{title}</TableCell>)}</TableRow></TableHead><TableBody>{items.map(item => <TableRow key={item.id}><TableCell>{item.name}<Typography variant="caption" display="block" color="text.secondary">{item.scope_type === 'public' ? '公共事项' : item.group_name || '原实验室范围'}</Typography></TableCell><TableCell>{item.category || '未分类'}<br />系数 {item.coefficient}</TableCell><TableCell>{item.deleted_at ? '已删除' : item.enabled ? '启用' : '停用'} / {item.sort_order}</TableCell><TableCell><Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>{!item.deleted_at && <Button size="small" variant="outlined" onClick={() => { setItemForm({ name: item.name, category: item.category, coefficient: item.coefficient, enabled: item.enabled, sort_order: item.sort_order, notes: item.notes, expected_updated_at: item.updated_at }); setItemEditor(item); setError(''); }}>编辑</Button>}<Button size="small" variant="outlined" color={item.deleted_at ? 'primary' : 'error'} onClick={() => setStateAction({ kind: 'items', id: item.id, updated_at: item.updated_at, restore: !!item.deleted_at })}>{item.deleted_at ? '恢复' : '删除'}</Button></Box></TableCell></TableRow>)}</TableBody></Table></TableContainer>
          {user?.is_admin && <Box sx={{ mt: 2 }}><RecordActionSettings kind="auxiliary" columns={columns.filter(column => column.key !== '_action')} actions={actions} /></Box>}
        </>}
      </>}
      <Dialog open={!!recordEditor} onClose={saving ? undefined : () => setRecordEditor(null)} fullWidth maxWidth="sm"><DialogTitle>{editingRecord ? '编辑辅助记录' : '录入辅助工作量'}</DialogTitle><DialogContent>
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
      </DialogContent><DialogActions><Button onClick={refresh} disabled={saving || loading}>刷新可选事项</Button><Button onClick={() => setRecordEditor(null)} disabled={saving}>取消</Button><Button variant="contained" onClick={saveRecord} disabled={saving || loading}>{saving ? '保存中…' : '确认录入'}</Button></DialogActions></Dialog>
      <Dialog open={!!itemEditor} onClose={saving ? undefined : () => setItemEditor(null)} fullWidth maxWidth="sm"><DialogTitle>{itemEditor === 'new' ? '新增公共辅助事项' : '编辑辅助事项'}</DialogTitle><DialogContent>{error && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}<Box sx={{ display: 'grid', gap: 1.5, pt: 1 }}><TextField label="事项名称" size="small" value={itemForm.name} inputProps={{ maxLength: 120 }} onChange={event => setItemForm(previous => ({ ...previous, name: event.target.value }))} /><TextField label="辅助子类型" placeholder="未分类" size="small" value={itemForm.category} inputProps={{ maxLength: 64 }} onChange={event => setItemForm(previous => ({ ...previous, category: event.target.value }))} /><TextField label="默认系数（0有效）" size="small" type="number" value={itemForm.coefficient} inputProps={{ min: 0, step: 'any' }} onChange={event => setItemForm(previous => ({ ...previous, coefficient: Number(event.target.value) }))} /><TextField label="排序" size="small" type="number" value={itemForm.sort_order} onChange={event => setItemForm(previous => ({ ...previous, sort_order: Number(event.target.value) }))} /><FormControlLabel control={<Checkbox checked={itemForm.enabled} onChange={event => setItemForm(previous => ({ ...previous, enabled: event.target.checked }))} />} label="启用" /><TextField label="备注" size="small" multiline minRows={2} value={itemForm.notes} onChange={event => setItemForm(previous => ({ ...previous, notes: event.target.value }))} /></Box></DialogContent><DialogActions><Button onClick={() => setItemEditor(null)} disabled={saving}>取消</Button><Button variant="contained" onClick={saveItem} disabled={saving}>{saving ? '保存中…' : '保存事项'}</Button></DialogActions></Dialog>
      <ConfirmDialog open={!!stateAction} title={stateAction?.restore ? '恢复辅助数据' : '删除辅助数据'} message={error || (stateAction?.kind === 'items' ? '事项变更不修改已保存的历史工作量。' : '此操作会改变人员统计，请核对后填写原因。')} collectReason reasonLabel="操作原因（必填）" confirmText={stateAction?.restore ? '确认恢复' : '确认删除'} loading={saving} onConfirm={changeState} onCancel={() => setStateAction(null)} />
    </Box>
  </Box>;
}
