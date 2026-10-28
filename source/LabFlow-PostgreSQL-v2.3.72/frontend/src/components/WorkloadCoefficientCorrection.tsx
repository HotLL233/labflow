import { useEffect, useMemo, useRef, useState } from 'react';
import { Alert, Box, Button, Checkbox, Dialog, DialogActions, DialogContent, DialogTitle, LinearProgress, Paper, Stack, Table, TableBody, TableCell, TableHead, TablePagination, TableRow, TextField, Typography, useMediaQuery, useTheme } from '@mui/material';
import dayjs from 'dayjs';
import { applyWorkloadCoefficients, previewWorkloadCoefficients, requireApiSuccess } from '../api/client';
import type { WorkloadCoefficientDifference, WorkloadCoefficientPreview } from '../types';
import { beijingRecordToday, recordDatePreset, validateRecordDateRange } from '../utils/recordDateRange';

const PAGE_SIZES = [20, 50, 100, 200, 500];
const PREPARE_PAGE_SIZE = 500;
const APPLY_BATCH_SIZE = 100;
type CoefficientSelection = Pick<WorkloadCoefficientDifference, 'id' | 'before_workload' | 'after_workload' | 'difference'> & { token: string; expiresAt: string };
const selectionFromPreview = (preview: WorkloadCoefficientPreview, ids?: Set<number>): CoefficientSelection[] => preview.rows.filter(row => !ids || ids.has(row.id)).map(row => ({ id: row.id, before_workload: row.before_workload, after_workload: row.after_workload, difference: row.difference, token: preview.confirmation_token, expiresAt: preview.expires_at }));
const correctionBatches = (rows: CoefficientSelection[]) => {
  const groups = new Map<string, CoefficientSelection[]>();
  for (const row of rows) { const group = groups.get(row.token) ?? []; group.push(row); groups.set(row.token, group); }
  const batches: CoefficientSelection[][] = [];
  for (const group of groups.values()) for (let offset = 0; offset < group.length; offset += APPLY_BATCH_SIZE) batches.push(group.slice(offset, offset + APPLY_BATCH_SIZE));
  return batches;
};
const expiredSelection = (rows: CoefficientSelection[]) => rows.some(row => !Number.isFinite(Date.parse(row.expiresAt)) || Date.parse(row.expiresAt) <= Date.now());
const number = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 12 });
const difference = (value: number) => `${value > 0 ? '+' : ''}${number(value)}`;
const recordLabel = (row: WorkloadCoefficientDifference) => row.business_no || '历史记录（业务编号缺失）';

export default function WorkloadCoefficientCorrection() {
  const theme = useTheme();
  const mobile = useMediaQuery(theme.breakpoints.down('sm'));
  const [filters, setFilters] = useState(() => ({ ...recordDatePreset('recent-30', beijingRecordToday()), keyword: '' }));
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [preview, setPreview] = useState<WorkloadCoefficientPreview | null>(null);
  const [selected, setSelected] = useState<CoefficientSelection[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [reason, setReason] = useState('');
  const [needsPreview, setNeedsPreview] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [prepareProgress, setPrepareProgress] = useState<{ loaded: number; total: number } | null>(null);
  const [applyProgress, setApplyProgress] = useState<{ batch: number; batches: number; completed: number; total: number } | null>(null);
  const mounted = useRef(true);
  const request = useRef(0);
  const previewRequest = useRef<number | null>(null);
  const applying = useRef(false);
  const preparing = useRef(false);
  const invalidated = useRef(false);
  const confirmation = useRef<CoefficientSelection[] | null>(null);
  const controller = useRef<AbortController | null>(null);
  const rangeError = validateRecordDateRange(filters.start, filters.end);
  const selectedIds = useMemo(() => new Set(selected.map(row => row.id)), [selected]);
  const batches = useMemo(() => correctionBatches(selected), [selected]);
  const totals = useMemo(() => selected.reduce((sum, row) => ({ before: sum.before + row.before_workload, after: sum.after + row.after_workload, difference: sum.difference + row.difference }), { before: 0, after: 0, difference: 0 }), [selected]);
  const allSelected = Boolean(preview?.total) && selected.length === preview?.total;
  const earliestExpiry = useMemo(() => selected.length ? new Date(selected.reduce((earliest, row) => Math.min(earliest, Date.parse(row.expiresAt)), Infinity)) : null, [selected]);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; request.current++; previewRequest.current = null; confirmation.current = null; controller.current?.abort(); };
  }, []);

  const changeFilters = (patch: Partial<typeof filters>) => {
    if (applying.current) return;
    request.current++; previewRequest.current = null; confirmation.current = null; controller.current?.abort(); preparing.current = false; invalidated.current = false;
    setFilters(current => ({ ...current, ...patch })); setPage(0); setPreview(null); setSelected([]);
    setConfirmOpen(false); setReason(''); setNeedsPreview(false); setError(''); setSuccess(''); setLoading(false); setPrepareProgress(null); setApplyProgress(null);
  };

  const loadPreview = async (nextPage = page, resetSelection = false, nextSize = pageSize) => {
    if (applying.current || (invalidated.current && !resetSelection)) return;
    const invalid = validateRecordDateRange(filters.start, filters.end);
    if (invalid) { setError(invalid); return; }
    const sequence = ++request.current;
    previewRequest.current = null; confirmation.current = null; preparing.current = false;
    controller.current?.abort();
    const currentController = new AbortController(); controller.current = currentController;
    const fresh = () => mounted.current && sequence === request.current && !currentController.signal.aborted;
    setLoading(true); setPreview(null); setPrepareProgress(null); setConfirmOpen(false); setReason(''); setError(''); setPage(nextPage);
    if (resetSelection) { setSelected([]); setNeedsPreview(false); invalidated.current = false; setApplyProgress(null); }
    try {
      const result = requireApiSuccess(await previewWorkloadCoefficients({ ...filters, keyword: filters.keyword.trim(), page: nextPage + 1, page_size: nextSize }, currentController.signal), '历史系数预览失败');
      if (!fresh()) return;
      if (!result) throw new Error('历史系数预览返回为空，请重试');
      if (nextPage > 0 && nextPage * nextSize >= result.total) { await loadPreview(Math.max(0, Math.ceil(result.total / nextSize) - 1), resetSelection, nextSize); return; }
      previewRequest.current = sequence; setPreview(result);
    } catch (failure) {
      if (fresh()) setError(failure instanceof Error ? failure.message : '历史系数预览失败，请重试');
    } finally { if (fresh()) setLoading(false); }
  };

  const changePageSize = (size: number) => {
    if (applying.current || !PAGE_SIZES.includes(size)) return;
    setPageSize(size); void loadPreview(0, true, size);
  };

  const cancelPreparation = () => {
    if (!preparing.current) return;
    request.current++; controller.current?.abort(); preparing.current = false;
    previewRequest.current = preview ? request.current : null; setPrepareProgress(null);
  };

  const selectAllResults = async () => {
    if (applying.current || preparing.current || loading || invalidated.current || !preview || previewRequest.current !== request.current) return;
    confirmation.current = null; setConfirmOpen(false); setReason(''); setError('');
    if (allSelected) { setSelected([]); return; }
    const sequence = ++request.current;
    previewRequest.current = null; preparing.current = true;
    controller.current?.abort();
    const currentController = new AbortController(); controller.current = currentController;
    const fresh = () => mounted.current && sequence === request.current && !currentController.signal.aborted;
    setPrepareProgress({ loaded: 0, total: preview.total });
    try {
      const prepared: CoefficientSelection[] = [];
      const ids = new Set<number>();
      const visibleRows = new Map(preview.rows.map(row => [row.id, row]));
      let total: number | null = null;
      for (let nextPage = 1; total === null || prepared.length < total; nextPage++) {
        const result = requireApiSuccess(await previewWorkloadCoefficients({ ...filters, keyword: filters.keyword.trim(), page: nextPage, page_size: PREPARE_PAGE_SIZE }, currentController.signal), '全选预览失败');
        if (!fresh()) return;
        if (!result) throw new Error('全选预览返回为空，请重新预览');
        if (total !== null && result.total !== total) throw new Error('准备期间差异记录数量发生变化，请重新预览');
        total = result.total;
        if (result.page !== nextPage || result.page_size !== PREPARE_PAGE_SIZE || result.rows.length !== Math.min(PREPARE_PAGE_SIZE, Math.max(0, total - prepared.length))) throw new Error('全选预览不完整，请重新预览');
        for (const row of result.rows) {
          const visible = visibleRows.get(row.id);
          if (visible && Object.entries(visible).some(([key, value]) => row[key as keyof WorkloadCoefficientDifference] !== value)) throw new Error('当前页记录或方法配置已变化，请重新预览');
          visibleRows.delete(row.id);
        }
        const rows = selectionFromPreview(result);
        for (const row of rows) {
          if (ids.has(row.id)) throw new Error('准备期间记录分页发生变化，请重新预览');
          ids.add(row.id); prepared.push(row);
        }
        setPrepareProgress({ loaded: prepared.length, total });
        if (expiredSelection(rows)) throw new Error('全选准备耗时过长，预览已过期，请缩小范围后重新预览');
      }
      if (!fresh()) return;
      if (expiredSelection(prepared)) throw new Error('全选准备耗时过长，预览已过期，请缩小范围后重新预览');
      if (visibleRows.size > 0) throw new Error('当前页记录已不在差异结果中，请重新预览');
      if (total !== preview.total) throw new Error('差异记录数量已变化，请重新预览后全选');
      previewRequest.current = sequence; setSelected(prepared);
    } catch (failure) {
      if (fresh()) {
        invalidated.current = true; setNeedsPreview(true);
        setError(`${failure instanceof Error ? failure.message : '全选预览失败'}。准备过程没有修改记录，请重新预览；不会使用部分准备结果。`);
      }
    } finally { if (fresh()) { preparing.current = false; setPrepareProgress(null); } }
  };

  const selectRows = (ids: number[]) => {
    if (applying.current || preparing.current || loading || confirmOpen || invalidated.current || !preview || previewRequest.current !== request.current) return;
    const offered = new Set(preview.rows.map(row => row.id));
    confirmation.current = null;
    const choices = selectionFromPreview(preview, new Set(ids));
    setSelected(current => [...current.filter(row => !offered.has(row.id)), ...choices]);
  };

  const openConfirmation = () => {
    if (applying.current || preparing.current || loading || invalidated.current || selected.length === 0 || !preview || previewRequest.current !== request.current) return;
    if (expiredSelection(selected)) { confirmation.current = null; invalidated.current = true; setNeedsPreview(true); setError('选中的预览已过期，请重新预览并勾选。'); return; }
    confirmation.current = selected; setError(''); setConfirmOpen(true); setApplyProgress(null);
  };

  const closeConfirmation = () => {
    if (applying.current) return;
    confirmation.current = null; setConfirmOpen(false);
  };

  const applyCorrection = async () => {
    if (applying.current || preparing.current || loading || invalidated.current || !confirmOpen || confirmation.current !== selected || !preview || previewRequest.current !== request.current || selected.length === 0 || !reason.trim() || reason.trim().length > 500) return;
    if (expiredSelection(selected)) { confirmation.current = null; invalidated.current = true; setNeedsPreview(true); setError('选中的预览已过期，本次未提交，请重新预览并勾选。'); return; }
    applying.current = true; setSaving(true); setError('');
    const sequence = request.current;
    const fresh = () => mounted.current && sequence === request.current;
    let completed = 0, submitted = 0, currentBatchSize = 0;
    let before = 0, after = 0, delta = 0;
    try {
      for (let index = 0; index < batches.length; index++) {
        if (!fresh()) return;
        const batch = batches[index];
        if (expiredSelection(batch)) throw new Error('下一批预览已过期');
        setApplyProgress({ batch: index + 1, batches: batches.length, completed, total: selected.length });
        currentBatchSize = batch.length; submitted += batch.length;
        const result = requireApiSuccess(await applyWorkloadCoefficients({ confirmation_token: batch[0].token, record_ids: batch.map(row => row.id), reason: reason.trim() }), '历史系数修正失败');
        if (!fresh()) return;
        if (!result || result.updated_count !== batch.length) throw new Error('当前批次未收到完整修正结果');
        completed += result.updated_count; before += result.before_workload; after += result.after_workload; delta += result.difference; currentBatchSize = 0;
        setApplyProgress({ batch: index + 1, batches: batches.length, completed, total: selected.length });
      }
      setSuccess(`已分 ${batches.length} 批修正 ${completed} 条记录：工作量 ${number(before)} → ${number(after)}，差额 ${difference(delta)}。`);
      confirmation.current = null; setConfirmOpen(false); setSelected([]); setReason('');
      applying.current = false; setSaving(false);
      await loadPreview(page, true);
    } catch (failure) {
      if (fresh()) {
        previewRequest.current = null; confirmation.current = null; invalidated.current = true;
        setError(`${failure instanceof Error ? failure.message : '修正请求未成功确认'}。已确认成功 ${completed} 条，未确认完成 ${selected.length - completed} 条（当前批 ${currentBatchSize} 条需核对，后续 ${selected.length - submitted} 条未提交）。已成功批次保留，已停止后续提交；请重新预览核对，系统不会自动重试。`);
        setNeedsPreview(true);
      }
    } finally { applying.current = false; if (mounted.current) setSaving(false); }
  };

  const selectedCheckbox = (row: WorkloadCoefficientDifference) => <Checkbox size="small" checked={selectedIds.has(row.id)} disabled={loading || saving || needsPreview || Boolean(prepareProgress)} inputProps={{ 'aria-label': `选择修正 ${recordLabel(row)}，${row.user_name}，${row.method_name}` }} onChange={event => selectRows(event.target.checked ? [...selectedIds, row.id] : [...selectedIds].filter(id => id !== row.id))} />;
  const workloadChange = (row: WorkloadCoefficientDifference) => <><Typography variant="body2">{number(row.before_workload)} → {number(row.after_workload)}</Typography><Typography variant="caption" color={row.difference < 0 ? 'warning.main' : 'text.secondary'}>差额 {difference(row.difference)}</Typography></>;

  return <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mt: 2, minWidth: 0, borderRadius: '2px' }}>
    <Typography variant="h6" fontWeight={800}>历史工作量系数修正</Typography>
    <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, mb: 1.5 }}>仅管理员可操作。按工作量日期查询仍有关联标准方法的有效记录；自定义方法及独立辅助工作不在此范围。</Typography>
    <Alert severity="warning" sx={{ mb: 1.5 }}>当前方法系数与历史快照不同，不代表历史记录一定错误。确认修正会将选中记录改为当前方法系数，并改变对应人员和日期的历史统计；数量、金额倍率和人员归属不变。请先核对业务依据。</Alert>
    <Box component="form" onSubmit={event => { event.preventDefault(); void loadPreview(0, true); }} sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', sm: 'repeat(2,minmax(0,1fr))', lg: '1fr 1fr 1.5fr auto' }, gap: 1.5, alignItems: 'start', mb: 1.5 }}>
      <TextField type="date" label="工作量开始日期" size="small" required value={filters.start} onChange={event => changeFilters({ start: event.target.value })} InputLabelProps={{ shrink: true }} disabled={saving} />
      <TextField type="date" label="工作量结束日期" size="small" required value={filters.end} onChange={event => changeFilters({ end: event.target.value })} InputLabelProps={{ shrink: true }} disabled={saving} />
      <TextField label="关键词" placeholder="记录编号、人员、项目或方法" size="small" value={filters.keyword} onChange={event => changeFilters({ keyword: event.target.value })} inputProps={{ maxLength: 100 }} disabled={saving} />
      <Button type="submit" variant="outlined" disabled={loading || saving || Boolean(prepareProgress) || Boolean(rangeError)} sx={{ minHeight: 40 }}>{loading ? '正在预览…' : '预览差异'}</Button>
    </Box>
    {rangeError && <Typography variant="caption" color="error" sx={{ display: 'block', mb: 1 }}>{rangeError}</Typography>}
    {success && <Alert severity="success" sx={{ mb: 1.5 }}>{success}</Alert>}
    {error && <Alert severity="error" sx={{ mb: 1.5 }}>{error}</Alert>}
    {loading && <LinearProgress sx={{ mb: 1.5 }} />}
    {!preview && !loading && <Typography variant="body2" color="text.secondary">先预览差异，再勾选需要修正的记录；默认不选中任何记录。</Typography>}
    {preview && <>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1} alignItems={{ sm: 'center' }} justifyContent="space-between" sx={{ mb: 1 }}>
        <Typography variant="body2">筛选内有 {preview.total} 条差异，本页 {preview.rows.length} 条，跨页已选 {selected.length} 条。</Typography>
        <Button variant="contained" color="warning" size="small" disabled={loading || saving || needsPreview || Boolean(prepareProgress) || selected.length === 0} onClick={openConfirmation}>复核选中记录</Button>
      </Stack>
      <Stack direction="row" spacing={1} alignItems="center" flexWrap="wrap" sx={{ mb: 1 }}>
        <Box component="label" sx={{ display: 'inline-flex', alignItems: 'center', minWidth: 0 }}><Checkbox size="small" checked={allSelected} indeterminate={selected.length > 0 && !allSelected} disabled={loading || saving || needsPreview || Boolean(prepareProgress) || preview.total === 0} onChange={() => void selectAllResults()} inputProps={{ 'aria-label': '全选筛选结果，包含所有分页' }} /><Typography variant="body2">全选筛选结果（共 {preview.total} 条，包含所有分页）</Typography></Box>
        <Button size="small" variant="outlined" disabled={loading || saving || needsPreview || Boolean(prepareProgress) || preview.rows.length === 0} onClick={() => selectRows(preview.rows.map(row => row.id))}>选择本页（{preview.rows.length} 条）</Button>
        {selected.length > 0 && !prepareProgress && <Button size="small" disabled={loading || saving} onClick={() => { confirmation.current = null; setSelected([]); }}>清除全部勾选</Button>}
      </Stack>
      {prepareProgress && <Box role="status" aria-live="polite" sx={{ mb: 1.5 }}><Typography variant="body2">正在准备全部差异：{prepareProgress.loaded} / {prepareProgress.total} 条，仅预览，尚未修改记录。</Typography><LinearProgress variant={prepareProgress.total > 0 ? 'determinate' : 'indeterminate'} value={prepareProgress.total ? prepareProgress.loaded / prepareProgress.total * 100 : 0} sx={{ my: 1 }} /><Button variant="outlined" size="small" onClick={cancelPreparation}>取消全选准备</Button></Box>}
      {preview.rows.length === 0 ? <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>当前范围没有需要核对的标准方法系数差异。</Typography> : mobile ? <Stack spacing={1}>
        {preview.rows.map(row => <Paper key={row.id} variant="outlined" sx={{ p: 1, minWidth: 0 }}>
          <Stack direction="row" spacing={0.5} alignItems="center"><Box>{selectedCheckbox(row)}</Box><Box sx={{ minWidth: 0 }}><Typography variant="body2" fontWeight={700} sx={{ overflowWrap: 'anywhere' }}>{recordLabel(row)}</Typography><Typography variant="caption">{row.user_name || '未记录人员'} · {dayjs(row.recorded_at).format('YYYY-MM-DD HH:mm')}</Typography></Box></Stack>
          <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 0.75, mt: 0.5 }}>
            {[
              ['方法 / 仪器', `${row.method_name} / ${row.instrument_name || '未关联仪器'}`], ['项目 / 实验室', `${row.project_name || '未记录项目'} / ${row.lab_name || '未记录实验室'}`],
              ['数量', number(row.quantity)], ['系数（原 → 当前方法）', `${number(row.coefficient_snapshot)} → ${number(row.method_coefficient)}`],
            ].map(([label, value]) => <Box key={label} sx={{ border: '1px solid', borderColor: 'divider', p: 0.75, minWidth: 0 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{value}</Typography></Box>)}
            <Box sx={{ gridColumn: '1 / -1', border: '1px solid', borderColor: 'divider', p: 0.75 }}><Typography variant="caption" color="text.secondary">工作量（修正前 → 修正后）</Typography>{workloadChange(row)}</Box>
          </Box>
        </Paper>)}
      </Stack> : <Table size="small" aria-label="历史工作量系数差异预览" sx={{ tableLayout: 'fixed', width: '100%', '& th, & td': { border: '1px solid', borderColor: 'divider', p: 1, overflowWrap: 'anywhere', verticalAlign: 'top' } }}>
        <TableHead><TableRow><TableCell sx={{ width: 40, p: '4px!important' }}><Checkbox size="small" checked={allSelected} indeterminate={selected.length > 0 && !allSelected} disabled={loading || saving || needsPreview || Boolean(prepareProgress) || preview.total === 0} inputProps={{ 'aria-label': '全选全部筛选结果' }} onChange={() => void selectAllResults()} /></TableCell><TableCell sx={{ width: '22%' }}>记录 / 人员 / 日期</TableCell><TableCell sx={{ width: '27%' }}>方法 / 仪器 / 归属</TableCell><TableCell sx={{ width: '9%' }}>数量</TableCell><TableCell sx={{ width: '18%' }}>系数<br />原 → 当前方法</TableCell><TableCell>工作量<br />修正前 → 修正后</TableCell></TableRow></TableHead>
        <TableBody>{preview.rows.map(row => <TableRow key={row.id}><TableCell sx={{ p: '4px!important' }}>{selectedCheckbox(row)}</TableCell><TableCell><Typography variant="body2" fontWeight={700}>{recordLabel(row)}</Typography><Typography variant="body2">{row.user_name || '未记录人员'}</Typography><Typography variant="caption">{dayjs(row.recorded_at).format('YYYY-MM-DD HH:mm')}</Typography></TableCell><TableCell><Typography variant="body2">{row.method_name}</Typography><Typography variant="caption" sx={{ display: 'block' }}>{row.instrument_name || '未关联仪器'}</Typography><Typography variant="caption" color="text.secondary">{row.project_name || '未记录项目'} · {row.lab_name || '未记录实验室'}</Typography></TableCell><TableCell>{number(row.quantity)}</TableCell><TableCell>{number(row.coefficient_snapshot)} → {number(row.method_coefficient)}</TableCell><TableCell>{workloadChange(row)}</TableCell></TableRow>)}</TableBody>
      </Table>}
      <TablePagination component="div" count={preview.total} page={page} rowsPerPage={pageSize} rowsPerPageOptions={PAGE_SIZES} onRowsPerPageChange={event => changePageSize(Number(event.target.value))} onPageChange={(_, nextPage) => void loadPreview(nextPage)} backIconButtonProps={{ disabled: loading || saving || needsPreview || Boolean(prepareProgress) || page === 0 }} nextIconButtonProps={{ disabled: loading || saving || needsPreview || Boolean(prepareProgress) || (page + 1) * pageSize >= preview.total }} SelectProps={{ disabled: loading || saving }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} getItemAriaLabel={type => type === 'next' ? '下一页差异' : '上一页差异'} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap', justifyContent: 'flex-end' }, '& .MuiTablePagination-spacer': { flex: '0 0 auto' }, '& .MuiTablePagination-selectLabel, & .MuiTablePagination-displayedRows': { mx: 0.5 }, '& .MuiTablePagination-input': { mr: 0.5 }, '& .MuiTablePagination-actions': { ml: 0.5 } }} />
      <Typography variant="caption" color="text.secondary">翻页保留勾选；更改筛选、每页数量或重新预览会清除勾选。全选会逐页读取全部差异，仅渲染当前页；确认后每批最多 100 条，遇到冲突或失败即停止。</Typography>
    </>}
    <Dialog open={confirmOpen} onClose={closeConfirmation} fullScreen={mobile} fullWidth maxWidth="sm">
      <DialogTitle>确认修正历史工作量</DialogTitle>
      <DialogContent dividers>
        <Alert severity="warning" sx={{ mb: 1.5 }}>将改变选中记录的历史系数快照及工作量统计。当前方法系数只能作为修正依据，不能证明原历史快照错误；请核对后提交。</Alert>
        <Typography fontWeight={700}>跨页选中 {selected.length} 条 · 共 {batches.length} 批 · 每批最多 {APPLY_BATCH_SIZE} 条</Typography>
        <Typography variant="body2" sx={{ mt: 1 }}>汇总工作量：{number(totals.before)} → {number(totals.after)}</Typography>
        <Typography variant="body2" sx={{ mb: 1 }}>工作量差额：{difference(totals.difference)}</Typography>
        <Alert severity="info" sx={{ mb: 1.5 }}>按预览来源逐批提交。已成功批次会保留；任一批次冲突、过期或失败都会停止，未提交批次不会自动继续。网络中断时当前批次可能已写入，必须重新预览核对，不能直接重试。</Alert>
        <TextField label="修正原因" value={reason} onChange={event => setReason(event.target.value)} required multiline minRows={2} fullWidth disabled={saving || needsPreview} inputProps={{ maxLength: 500 }} helperText={`必填，将写入审计记录；${reason.trim().length}/500 字`} />
        {earliestExpiry && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>最早预览有效期至 {dayjs(earliestExpiry).format('YYYY-MM-DD HH:mm:ss')}；提交每批前检查期限，服务器再次核对该批记录与方法配置。</Typography>}
        {applyProgress && <Box role="status" aria-live="polite" sx={{ mt: 1.5 }}><Typography variant="body2">{saving ? '正在提交' : '提交已停止'}第 {applyProgress.batch} / {applyProgress.batches} 批；已确认成功 {applyProgress.completed} / {applyProgress.total} 条。</Typography><LinearProgress variant="determinate" value={applyProgress.total ? applyProgress.completed / applyProgress.total * 100 : 0} sx={{ mt: 1 }} /></Box>}
        {error && <Alert severity="error" sx={{ mt: 1.5 }}>{error}</Alert>}
      </DialogContent>
      <DialogActions sx={{ flexWrap: 'wrap', gap: 0.5 }}>
        <Button onClick={closeConfirmation} disabled={saving}>{needsPreview ? '关闭' : '取消'}</Button>
        {needsPreview && <Button variant="outlined" onClick={() => void loadPreview(page, true)} disabled={saving}>重新预览并清除勾选</Button>}
        <Button color="warning" variant="contained" onClick={() => void applyCorrection()} disabled={saving || loading || needsPreview || selected.length === 0 || !reason.trim() || reason.trim().length > 500}>{saving ? '正在逐批修正…' : `确认修正 ${selected.length} 条（${batches.length} 批）`}</Button>
      </DialogActions>
    </Dialog>
  </Paper>;
}
