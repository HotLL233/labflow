import { useEffect, useRef, useState } from 'react';
import { Alert, Autocomplete, Box, Button, Checkbox, Dialog, DialogActions, DialogContent, DialogTitle, LinearProgress, Paper, Stack, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, TextField, Typography } from '@mui/material';
import { getAuxiliaryItems, type AuxiliaryItem } from '../api/auxiliary';
import { applyLegacyAuxiliary, getLegacyAuxiliaryBatches, getLegacyAuxiliaryRecords, previewLegacyAuxiliary, previewLegacyAuxiliaryUndo, undoLegacyAuxiliary, type LegacyAuxiliaryBatch, type LegacyAuxiliaryBatches, type LegacyAuxiliaryPage, type LegacyAuxiliaryPreview, type LegacyAuxiliaryRow } from '../api/legacyAuxiliary';
import { beijingRecordToday, recordDatePreset, validateRecordDateRange } from '../utils/recordDateRange';

const number = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 12 });
const failureText = (failure: unknown) => failure instanceof Error ? failure.message : '操作失败，请重新查询';
const itemLabel = (item: AuxiliaryItem) => `${item.name} · ${item.area_name || '未分区'} · ${item.category || '未分类'} · 系数 ${number(item.coefficient)}${item.enabled ? '' : ' · 停用'}`;
type Assignment = { record: LegacyAuxiliaryRow; item_id: number };
type Review = { kind: 'link' | 'undo'; batch_id?: string; preview: LegacyAuxiliaryPreview };
const expired = (preview: LegacyAuxiliaryPreview) => !Number.isFinite(Date.parse(preview.expires_at)) || Date.parse(preview.expires_at) <= Date.now();

export default function LegacyAuxiliaryMigration({ authorized, onChanged }: { authorized: boolean; onChanged?: () => void }) {
  const [filters, setFilters] = useState(() => ({ ...recordDatePreset('recent-30', beijingRecordToday()), keyword: '' }));
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [records, setRecords] = useState<LegacyAuxiliaryPage | null>(null);
  const [items, setItems] = useState<AuxiliaryItem[]>([]);
  const [assignments, setAssignments] = useState<Record<number, Assignment>>({});
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [review, setReview] = useState<Review | null>(null);
  const [reason, setReason] = useState('');
  const [batches, setBatches] = useState<LegacyAuxiliaryBatches | null>(null);
  const [batchPage, setBatchPage] = useState(0);
  const [batchLoading, setBatchLoading] = useState(false);
  const mounted = useRef(true);
  const sequence = useRef(0);
  const batchSequence = useRef(0);
  const reviewSequence = useRef(0);
  const controller = useRef<AbortController | null>(null);
  const batchController = useRef<AbortController | null>(null);
  const reviewController = useRef<AbortController | null>(null);
  const pending = useRef(false);
  const confirmation = useRef<Review | null>(null);
  const selected = Object.values(assignments);
  const rangeError = validateRecordDateRange(filters.start, filters.end);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; sequence.current++; batchSequence.current++; reviewSequence.current++; confirmation.current = null; controller.current?.abort(); batchController.current?.abort(); reviewController.current?.abort(); };
  }, []);

  const clearReview = () => { reviewSequence.current++; reviewController.current?.abort(); confirmation.current = null; setReview(null); setReason(''); setLoading(false); };
  const changeFilters = (patch: Partial<typeof filters>) => {
    if (pending.current) return;
    sequence.current++; controller.current?.abort(); clearReview();
    setFilters(previous => ({ ...previous, ...patch })); setPage(0); setRecords(null); setAssignments({}); setLoading(false); setError(''); setMessage('');
  };
  const loadRecords = async (nextPage = page, nextSize = pageSize, reset = false) => {
    if (!authorized || pending.current) return;
    const invalid = validateRecordDateRange(filters.start, filters.end);
    if (invalid) { setError(invalid); return; }
    const request = ++sequence.current; controller.current?.abort(); clearReview();
    const abort = new AbortController(); controller.current = abort;
    const fresh = () => mounted.current && request === sequence.current && !abort.signal.aborted;
    setLoading(true); setRecords(null); setPage(nextPage); setError('');
    if (reset) setAssignments({});
    try {
      const [result, options] = await Promise.all([getLegacyAuxiliaryRecords({ ...filters, keyword: filters.keyword.trim(), page: nextPage + 1, page_size: nextSize }, abort.signal), getAuxiliaryItems(false)]);
      if (!fresh()) return;
      if (nextPage > 0 && nextPage * nextSize >= result.total) { await loadRecords(Math.max(0, Math.ceil(result.total / nextSize) - 1), nextSize, reset); return; }
      setRecords(result); setItems(options.filter(item => !item.deleted_at));
      setAssignments(previous => Object.fromEntries(Object.entries(previous).filter(([id, assignment]) => options.some(item => item.id === assignment.item_id && !item.deleted_at) && !result.rows.some(row => row.id === Number(id) && row.linked_item_id != null))));
    } catch (failure) { if (fresh()) setError(failureText(failure)); }
    finally { if (fresh()) setLoading(false); }
  };
  const assignItem = (record: LegacyAuxiliaryRow, item: AuxiliaryItem | null) => {
    if (!authorized || pending.current || loading || record.linked_item_id != null) return;
    if (item && !assignments[record.id] && selected.length >= 100) { setError('每批最多100条，请先完成当前批次'); return; }
    clearReview(); setError('');
    setAssignments(previous => { const next = { ...previous }; if (item) next[record.id] = { record, item_id: item.id }; else delete next[record.id]; return next; });
  };
  const loadBatches = async (nextPage = batchPage) => {
    if (!authorized || pending.current) return;
    const request = ++batchSequence.current; batchController.current?.abort();
    const abort = new AbortController(); batchController.current = abort;
    const fresh = () => mounted.current && request === batchSequence.current && !abort.signal.aborted;
    setBatchLoading(true); setBatchPage(nextPage); setError('');
    try {
      const result = await getLegacyAuxiliaryBatches({ page: nextPage + 1, page_size: 20 }, abort.signal);
      if (!fresh()) return;
      if (nextPage > 0 && nextPage * 20 >= result.total) { await loadBatches(Math.max(0, Math.ceil(result.total / 20) - 1)); return; }
      setBatches(result);
    } catch (failure) { if (fresh()) setError(failureText(failure)); }
    finally { if (fresh()) setBatchLoading(false); }
  };
  const prepareReview = async (batch?: LegacyAuxiliaryBatch) => {
    if (!authorized || pending.current || loading || (batch ? !!batch.revoked_at : selected.length === 0 || selected.length > 100)) return;
    const request = ++reviewSequence.current; reviewController.current?.abort();
    const abort = new AbortController(); reviewController.current = abort;
    const fresh = () => mounted.current && request === reviewSequence.current && !abort.signal.aborted;
    confirmation.current = null; setLoading(true); setReview(null); setReason(''); setError('');
    try {
      const result = batch ? await previewLegacyAuxiliaryUndo(batch.id, abort.signal) : await previewLegacyAuxiliary(selected.map(assignment => ({ record_id: assignment.record.id, item_id: assignment.item_id })), abort.signal);
      if (!fresh()) return;
      if (result.rows.length !== result.record_count || (!batch && (result.record_count !== selected.length || new Set(result.rows.map(row => row.id)).size !== selected.length || result.rows.some(row => assignments[row.id]?.item_id !== row.item_id)))) throw new Error('关联预览与当前选择不一致，请重新查询');
      if (expired(result)) throw new Error('关联预览已过期，请重新查询');
      const prepared: Review = { kind: batch ? 'undo' : 'link', batch_id: batch?.id, preview: result };
      confirmation.current = prepared; setReview(prepared);
    } catch (failure) { if (fresh()) setError(failureText(failure)); }
    finally { if (fresh()) setLoading(false); }
  };
  const confirmReview = async () => {
    if (!authorized || !mounted.current || pending.current || loading || !review || confirmation.current !== review || !reason.trim() || reason.trim().length > 500) return;
    if (expired(review.preview)) { clearReview(); setError('关联预览已过期，请重新预览'); return; }
    pending.current = true; setSaving(true); setError('');
    try {
      const body = { confirmation_token: review.preview.confirmation_token, reason: reason.trim() };
      const result = review.kind === 'undo' ? await undoLegacyAuxiliary(review.batch_id!, body) : await applyLegacyAuxiliary(body);
      if (!mounted.current) return;
      if (result.record_count !== review.preview.record_count) throw new Error('未收到完整批次结果，请重新查询核对');
      setMessage(`${review.kind === 'undo' ? '已撤销本批关联' : '已完成关联'} ${result.record_count} 条，原数量 ${number(result.total_quantity)}、原工作量 ${number(result.total_workload)} 均保留，未重算系数。`);
      clearReview(); setAssignments({}); onChanged?.(); pending.current = false; setSaving(false);
      await Promise.all([loadRecords(page, pageSize, true), loadBatches(0)]);
    } catch (failure) { if (mounted.current) { clearReview(); setError(`${failureText(failure)}。结果不确定时请重新查询批次与记录，不要重复提交。`); } }
    finally { pending.current = false; if (mounted.current) setSaving(false); }
  };

  if (!authorized) return <Alert severity="warning">仅具备辅助配置及业务数据导入权限的系统管理员可关联历史辅助。</Alert>;

  return <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mt: 2, minWidth: 0, borderRadius: '2px' }}>
    <Typography variant="h6" fontWeight={800}>历史辅助关联迁移</Typography>
    <Alert severity="info" sx={{ my: 1.5 }}>将旧辅助记录关联到独立事项，保留原人员、日期、数量、系数和来源链，不复制记录、不重复计量。旧方法、项目和实验室仍保留历史引用，应停用或归档；主数据导入请选择“更新/新增（保留未列数据）”。关联成功不允许完整覆盖删除仍被引用的旧字典，系数调整需要另行复核。</Alert>
    <Box component="form" onSubmit={event => { event.preventDefault(); void loadRecords(0, pageSize, true); }} sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mb: 1 }}>
      <TextField label="开始日期" type="date" size="small" InputLabelProps={{ shrink: true }} value={filters.start} disabled={saving} onChange={event => changeFilters({ start: event.target.value })} />
      <TextField label="结束日期" type="date" size="small" InputLabelProps={{ shrink: true }} value={filters.end} disabled={saving} onChange={event => changeFilters({ end: event.target.value })} />
      <TextField label="搜索原业务号、人员或方法" size="small" value={filters.keyword} disabled={saving} inputProps={{ maxLength: 100 }} onChange={event => changeFilters({ keyword: event.target.value })} sx={{ minWidth: { xs: '100%', sm: 260 } }} />
      <Button type="submit" variant="outlined" disabled={loading || saving || !!rangeError}>查询旧辅助记录</Button>
    </Box>
    {rangeError && <Alert severity="error" sx={{ mb: 1 }}>{rangeError}</Alert>}
    {message && <Alert severity="success" sx={{ mb: 1 }}>{message}</Alert>}
    {error && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}
    {loading && <LinearProgress sx={{ mb: 1 }} />}
    <Stack direction="row" gap={1} flexWrap="wrap" alignItems="center" sx={{ my: 1 }}><Typography variant="body2">跨页已选 {selected.length} / 100 条，每条须明确选择目标事项。</Typography><Button size="small" variant="outlined" disabled={loading || saving || !selected.length} onClick={() => void prepareReview()}>预览关联与对账</Button><Button size="small" disabled={loading || saving || !selected.length} onClick={() => { clearReview(); setAssignments({}); }}>清除选择</Button></Stack>
    {records && <><TableContainer><Table size="small" aria-label="历史辅助关联候选" sx={{ minWidth: 660, '& td, & th': { verticalAlign: 'top', overflowWrap: 'anywhere' } }}><TableHead><TableRow><TableCell>选择</TableCell><TableCell>原记录 / 人员 / 日期</TableCell><TableCell>原方法 / 识别依据</TableCell><TableCell>数量 / 原系数 / 原工作量</TableCell><TableCell sx={{ width: '34%' }}>关联到独立事项</TableCell></TableRow></TableHead><TableBody>{records.rows.map(row => <TableRow key={row.id}><TableCell><Checkbox size="small" checked={!!assignments[row.id] || row.linked_item_id != null} disabled={!assignments[row.id] || loading || saving || row.linked_item_id != null} inputProps={{ 'aria-label': `取消选择 ${row.business_no || '历史记录'}` }} onChange={() => assignItem(row, null)} /></TableCell><TableCell>{row.business_no || '历史记录'}<Typography variant="caption" display="block">{row.user_name} · {row.recorded_at.replace('T', ' ').slice(0, 16)}</Typography></TableCell><TableCell>{row.method_name || '未记录方法'}<Typography variant="caption" display="block" color="text.secondary">{row.evidence}</Typography></TableCell><TableCell>{number(row.quantity)} / {number(row.coefficient)} / {number(row.workload)}</TableCell><TableCell>{row.linked_item_id != null ? <Typography variant="body2">已关联：{row.linked_item_name}，无需重复迁移</Typography> : <Autocomplete size="small" options={items} getOptionKey={item => item.id} getOptionLabel={itemLabel} isOptionEqualToValue={(a, b) => a.id === b.id} value={items.find(item => item.id === assignments[row.id]?.item_id) || null} disabled={loading || saving} onChange={(_, item) => assignItem(row, item)} renderInput={input => <TextField {...input} label="选择辅助事项" />} />}</TableCell></TableRow>)}{!records.rows.length && <TableRow><TableCell colSpan={5} align="center">当前范围没有旧辅助候选；不会按同名普通方法自动迁移。</TableCell></TableRow>}</TableBody></Table></TableContainer><TablePagination component="div" count={records.total} page={page} rowsPerPage={pageSize} rowsPerPageOptions={[20, 50, 100, 200]} onPageChange={(_, next) => void loadRecords(next)} onRowsPerPageChange={event => { const size = Number(event.target.value); setPageSize(size); void loadRecords(0, size, true); }} labelRowsPerPage="每页" backIconButtonProps={{ disabled: loading || saving || page === 0 }} nextIconButtonProps={{ disabled: loading || saving || (page + 1) * pageSize >= records.total }} SelectProps={{ disabled: loading || saving }} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap' } }} /></>}
    <Stack direction="row" gap={1} alignItems="center" justifyContent="space-between" sx={{ mt: 2, mb: 1 }}><Typography fontWeight={700}>迁移批次与撤销</Typography><Button variant="outlined" size="small" disabled={saving || batchLoading} onClick={() => void loadBatches(0)}>查询批次</Button></Stack>
    {batchLoading && <LinearProgress />}
    {batches && <><TableContainer><Table size="small" aria-label="历史辅助迁移批次" sx={{ minWidth: 520, '& td': { verticalAlign: 'top' } }}><TableHead><TableRow><TableCell>时间 / 操作人 / 原因</TableCell><TableCell>条数 / 数量 / 原工作量</TableCell><TableCell>状态 / 操作</TableCell></TableRow></TableHead><TableBody>{batches.rows.map(batch => <TableRow key={batch.id}><TableCell>{batch.created_at.replace('T', ' ').slice(0, 16)} · {batch.created_by_name}<Typography variant="caption" display="block">{batch.reason}</Typography></TableCell><TableCell>{batch.record_count} / {number(batch.total_quantity)} / {number(batch.total_workload)}</TableCell><TableCell>{batch.revoked_at ? '已撤销' : <Button variant="outlined" color="warning" size="small" disabled={saving || loading || batchLoading} onClick={() => void prepareReview(batch)}>预览撤销关联</Button>}</TableCell></TableRow>)}{!batches.rows.length && <TableRow><TableCell colSpan={3} align="center">暂无迁移批次</TableCell></TableRow>}</TableBody></Table></TableContainer><TablePagination component="div" count={batches.total} page={batchPage} rowsPerPage={20} rowsPerPageOptions={[20]} onPageChange={(_, next) => void loadBatches(next)} labelRowsPerPage="每页" backIconButtonProps={{ disabled: batchLoading || saving || batchPage === 0 }} nextIconButtonProps={{ disabled: batchLoading || saving || (batchPage + 1) * 20 >= batches.total }} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap' } }} /></>}
    <Dialog open={!!review} onClose={saving ? undefined : clearReview} fullWidth maxWidth="md"><DialogTitle>{review?.kind === 'undo' ? '确认撤销本批关联' : '确认历史辅助关联与对账'}</DialogTitle><DialogContent dividers>
      <Alert severity="warning" sx={{ mb: 1 }}>本次只{review?.kind === 'undo' ? '撤销关联' : '增加关联'}，不修改原记录或历史系数。预览有效期10分钟；记录或事项配置变化后需重新预览。</Alert>
      <Typography fontWeight={700}>{review?.preview.record_count} 条 · 数量 {number(review?.preview.total_quantity || 0)} · 工作量 {number(review?.preview.total_workload || 0)} → {number(review?.preview.total_workload || 0)}</Typography>
      <TableContainer sx={{ maxHeight: '40vh', my: 1 }}><Table size="small" sx={{ minWidth: 460 }}><TableHead><TableRow><TableCell>原记录 / 原方法</TableCell><TableCell>关联事项 / 区域 / 分类</TableCell><TableCell>原数量 / 原系数 / 原工作量</TableCell></TableRow></TableHead><TableBody>{review?.preview.rows.map(row => <TableRow key={row.id}><TableCell>{row.business_no || '历史记录'}<Typography variant="caption" display="block">{row.method_name}</Typography></TableCell><TableCell>{row.item_name}<Typography variant="caption" display="block">{row.area_name || '未分区'} / {row.category || '未分类'}</Typography></TableCell><TableCell>{number(row.quantity)} / {number(row.coefficient)} / {number(row.workload)}</TableCell></TableRow>)}</TableBody></Table></TableContainer>
      <TextField label={review?.kind === 'undo' ? '撤销原因' : '关联迁移原因'} required fullWidth size="small" multiline minRows={2} value={reason} disabled={saving} onChange={event => setReason(event.target.value)} inputProps={{ maxLength: 500 }} />
    </DialogContent><DialogActions><Button disabled={saving} onClick={clearReview}>取消</Button><Button color="warning" variant="contained" disabled={saving || loading || !review || !reason.trim()} onClick={() => void confirmReview()}>{saving ? '确认中…' : review?.kind === 'undo' ? '确认撤销本批关联' : '确认关联，保留原工作量'}</Button></DialogActions></Dialog>
  </Paper>;
}
