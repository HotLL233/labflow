import { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Checkbox, Dialog, DialogActions, DialogContent, DialogTitle, LinearProgress, Paper, Stack, Table, TableBody, TableCell, TableHead, TablePagination, TableRow, TextField, Typography, useMediaQuery, useTheme } from '@mui/material';
import dayjs from 'dayjs';
import { applyWorkloadCoefficients, previewWorkloadCoefficients, requireApiSuccess } from '../api/client';
import type { WorkloadCoefficientDifference, WorkloadCoefficientPreview } from '../types';
import { beijingRecordToday, recordDatePreset, validateRecordDateRange } from '../utils/recordDateRange';

const PAGE_SIZE = 50;
const number = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 12 });
const difference = (value: number) => `${value > 0 ? '+' : ''}${number(value)}`;
const recordLabel = (row: WorkloadCoefficientDifference) => row.business_no || '历史记录（业务编号缺失）';

export default function WorkloadCoefficientCorrection() {
  const theme = useTheme();
  const mobile = useMediaQuery(theme.breakpoints.down('sm'));
  const [filters, setFilters] = useState(() => ({ ...recordDatePreset('recent-30', beijingRecordToday()), keyword: '' }));
  const [page, setPage] = useState(0);
  const [preview, setPreview] = useState<WorkloadCoefficientPreview | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [reason, setReason] = useState('');
  const [needsPreview, setNeedsPreview] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const mounted = useRef(true);
  const request = useRef(0);
  const previewRequest = useRef<number | null>(null);
  const applying = useRef(false);
  const controller = useRef<AbortController | null>(null);
  const rangeError = validateRecordDateRange(filters.start, filters.end);
  const selectedRows = preview?.rows.filter(row => selected.includes(row.id)) ?? [];
  const totals = selectedRows.reduce((sum, row) => ({ before: sum.before + row.before_workload, after: sum.after + row.after_workload, difference: sum.difference + row.difference }), { before: 0, after: 0, difference: 0 });

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; request.current++; previewRequest.current = null; controller.current?.abort(); };
  }, []);

  const changeFilters = (patch: Partial<typeof filters>) => {
    if (applying.current) return;
    request.current++; previewRequest.current = null; controller.current?.abort();
    setFilters(current => ({ ...current, ...patch })); setPage(0); setPreview(null); setSelected([]);
    setConfirmOpen(false); setReason(''); setNeedsPreview(false); setError(''); setSuccess(''); setLoading(false);
  };

  const loadPreview = async (nextPage = page) => {
    if (applying.current) return;
    const invalid = validateRecordDateRange(filters.start, filters.end);
    if (invalid) { setError(invalid); return; }
    const sequence = ++request.current;
    previewRequest.current = null;
    controller.current?.abort();
    const currentController = new AbortController(); controller.current = currentController;
    const fresh = () => mounted.current && sequence === request.current && !currentController.signal.aborted;
    setLoading(true); setPreview(null); setSelected([]); setConfirmOpen(false); setReason(''); setNeedsPreview(false); setError(''); setPage(nextPage);
    try {
      const result = requireApiSuccess(await previewWorkloadCoefficients({ ...filters, keyword: filters.keyword.trim(), page: nextPage + 1, page_size: PAGE_SIZE }, currentController.signal), '历史系数预览失败');
      if (!fresh()) return;
      if (!result) throw new Error('历史系数预览返回为空，请重试');
      if (nextPage > 0 && nextPage * PAGE_SIZE >= result.total) { await loadPreview(Math.max(0, Math.ceil(result.total / PAGE_SIZE) - 1)); return; }
      previewRequest.current = sequence; setPreview(result);
    } catch (failure) {
      if (fresh()) setError(failure instanceof Error ? failure.message : '历史系数预览失败，请重试');
    } finally { if (fresh()) setLoading(false); }
  };

  const selectRows = (ids: number[]) => {
    if (applying.current || loading || confirmOpen || !preview || previewRequest.current !== request.current) return;
    const offered = new Set(preview.rows.map(row => row.id));
    setSelected([...new Set(ids)].filter(id => offered.has(id)));
  };

  const openConfirmation = () => {
    if (applying.current || loading || needsPreview || selectedRows.length === 0 || !preview || previewRequest.current !== request.current) return;
    setError(''); setConfirmOpen(true);
  };

  const applyCorrection = async () => {
    if (applying.current || loading || needsPreview || !confirmOpen || !preview || previewRequest.current !== request.current || selectedRows.length === 0 || !reason.trim() || reason.trim().length > 500) return;
    applying.current = true; setSaving(true); setError('');
    const sequence = request.current;
    const fresh = () => mounted.current && sequence === request.current;
    try {
      const result = requireApiSuccess(await applyWorkloadCoefficients({ confirmation_token: preview.confirmation_token, record_ids: selectedRows.map(row => row.id), reason: reason.trim() }), '历史系数修正失败');
      if (!fresh()) return;
      if (!result) throw new Error('未收到修正结果，请重新预览核对');
      setSuccess(`已修正 ${result.updated_count} 条记录：工作量 ${number(result.before_workload)} → ${number(result.after_workload)}，差额 ${difference(result.difference)}。`);
      setConfirmOpen(false); setSelected([]); setReason('');
      applying.current = false; setSaving(false);
      await loadPreview(page);
    } catch (failure) {
      if (fresh()) {
        previewRequest.current = null;
        setError(`${failure instanceof Error ? failure.message : '修正请求未成功确认'}。请重新预览后核对，系统没有自动重试。`);
        setNeedsPreview(true);
      }
    } finally { applying.current = false; if (mounted.current) setSaving(false); }
  };

  const selectedCheckbox = (row: WorkloadCoefficientDifference) => <Checkbox size="small" checked={selected.includes(row.id)} disabled={loading || saving || needsPreview} inputProps={{ 'aria-label': `选择修正 ${recordLabel(row)}，${row.user_name}，${row.method_name}` }} onChange={event => selectRows(event.target.checked ? [...selected, row.id] : selected.filter(id => id !== row.id))} />;
  const workloadChange = (row: WorkloadCoefficientDifference) => <><Typography variant="body2">{number(row.before_workload)} → {number(row.after_workload)}</Typography><Typography variant="caption" color={row.difference < 0 ? 'warning.main' : 'text.secondary'}>差额 {difference(row.difference)}</Typography></>;

  return <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mt: 2, minWidth: 0, borderRadius: '2px' }}>
    <Typography variant="h6" fontWeight={800}>历史工作量系数修正</Typography>
    <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, mb: 1.5 }}>仅管理员可操作。按工作量日期查询仍有关联标准方法的有效记录；自定义方法及独立辅助工作不在此范围。</Typography>
    <Alert severity="warning" sx={{ mb: 1.5 }}>当前方法系数与历史快照不同，不代表历史记录一定错误。确认修正会将选中记录改为当前方法系数，并改变对应人员和日期的历史统计；数量、金额倍率和人员归属不变。请先核对业务依据。</Alert>
    <Box component="form" onSubmit={event => { event.preventDefault(); void loadPreview(0); }} sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', sm: 'repeat(2,minmax(0,1fr))', lg: '1fr 1fr 1.5fr auto' }, gap: 1.5, alignItems: 'start', mb: 1.5 }}>
      <TextField type="date" label="工作量开始日期" size="small" required value={filters.start} onChange={event => changeFilters({ start: event.target.value })} InputLabelProps={{ shrink: true }} disabled={saving} />
      <TextField type="date" label="工作量结束日期" size="small" required value={filters.end} onChange={event => changeFilters({ end: event.target.value })} InputLabelProps={{ shrink: true }} disabled={saving} />
      <TextField label="关键词" placeholder="记录编号、人员、项目或方法" size="small" value={filters.keyword} onChange={event => changeFilters({ keyword: event.target.value })} inputProps={{ maxLength: 100 }} disabled={saving} />
      <Button type="submit" variant="outlined" disabled={loading || saving || Boolean(rangeError)} sx={{ minHeight: 40 }}>{loading ? '正在预览…' : '预览差异'}</Button>
    </Box>
    {rangeError && <Typography variant="caption" color="error" sx={{ display: 'block', mb: 1 }}>{rangeError}</Typography>}
    {success && <Alert severity="success" sx={{ mb: 1.5 }}>{success}</Alert>}
    {error && <Alert severity="error" sx={{ mb: 1.5 }}>{error}</Alert>}
    {loading && <LinearProgress sx={{ mb: 1.5 }} />}
    {!preview && !loading && <Typography variant="body2" color="text.secondary">先预览差异，再勾选需要修正的记录；默认不选中任何记录。</Typography>}
    {preview && <>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1} alignItems={{ sm: 'center' }} justifyContent="space-between" sx={{ mb: 1 }}>
        <Typography variant="body2">筛选内有 {preview.total} 条差异，本页 {preview.rows.length} 条，已选 {selectedRows.length} 条。</Typography>
        <Button variant="contained" color="warning" size="small" disabled={loading || saving || needsPreview || selectedRows.length === 0} onClick={openConfirmation}>复核选中记录</Button>
      </Stack>
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
        <TableHead><TableRow><TableCell sx={{ width: 40, p: '4px!important' }}><Checkbox size="small" checked={preview.rows.length > 0 && selectedRows.length === preview.rows.length} indeterminate={selectedRows.length > 0 && selectedRows.length < preview.rows.length} disabled={loading || saving || needsPreview} inputProps={{ 'aria-label': '选择本页全部差异记录' }} onChange={event => selectRows(event.target.checked ? preview.rows.map(row => row.id) : [])} /></TableCell><TableCell sx={{ width: '22%' }}>记录 / 人员 / 日期</TableCell><TableCell sx={{ width: '27%' }}>方法 / 仪器 / 归属</TableCell><TableCell sx={{ width: '9%' }}>数量</TableCell><TableCell sx={{ width: '18%' }}>系数<br />原 → 当前方法</TableCell><TableCell>工作量<br />修正前 → 修正后</TableCell></TableRow></TableHead>
        <TableBody>{preview.rows.map(row => <TableRow key={row.id}><TableCell sx={{ p: '4px!important' }}>{selectedCheckbox(row)}</TableCell><TableCell><Typography variant="body2" fontWeight={700}>{recordLabel(row)}</Typography><Typography variant="body2">{row.user_name || '未记录人员'}</Typography><Typography variant="caption">{dayjs(row.recorded_at).format('YYYY-MM-DD HH:mm')}</Typography></TableCell><TableCell><Typography variant="body2">{row.method_name}</Typography><Typography variant="caption" sx={{ display: 'block' }}>{row.instrument_name || '未关联仪器'}</Typography><Typography variant="caption" color="text.secondary">{row.project_name || '未记录项目'} · {row.lab_name || '未记录实验室'}</Typography></TableCell><TableCell>{number(row.quantity)}</TableCell><TableCell>{number(row.coefficient_snapshot)} → {number(row.method_coefficient)}</TableCell><TableCell>{workloadChange(row)}</TableCell></TableRow>)}</TableBody>
      </Table>}
      <TablePagination component="div" count={preview.total} page={page} rowsPerPage={PAGE_SIZE} rowsPerPageOptions={[]} onPageChange={(_, nextPage) => void loadPreview(nextPage)} backIconButtonProps={{ disabled: loading || saving || page === 0 }} nextIconButtonProps={{ disabled: loading || saving || (page + 1) * PAGE_SIZE >= preview.total }} labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} getItemAriaLabel={type => type === 'next' ? '下一页差异' : '上一页差异'} sx={{ '& .MuiTablePagination-toolbar': { px: 0 }, '& .MuiTablePagination-displayedRows': { mx: 1 } }} />
      <Typography variant="caption" color="text.secondary">更改筛选、翻页或重新预览会清除勾选；每次仅修正本页明确选中的记录。方法配置或记录变动后需重新预览。</Typography>
    </>}
    <Dialog open={confirmOpen} onClose={() => !applying.current && setConfirmOpen(false)} fullScreen={mobile} fullWidth maxWidth="sm">
      <DialogTitle>确认修正历史工作量</DialogTitle>
      <DialogContent dividers>
        <Alert severity="warning" sx={{ mb: 1.5 }}>将改变选中记录的历史系数快照及工作量统计。当前方法系数只能作为修正依据，不能证明原历史快照错误；请核对后提交。</Alert>
        <Typography fontWeight={700}>选中 {selectedRows.length} 条 · 工作量 {number(totals.before)} → {number(totals.after)}</Typography>
        <Typography variant="body2" sx={{ mb: 1 }}>工作量差额：{difference(totals.difference)}</Typography>
        <Typography variant="body2" sx={{ overflowWrap: 'anywhere', mb: 1.5 }}>{selectedRows.map(row => `${recordLabel(row)}（${row.user_name || '未记录人员'} · ${row.method_name}）`).join('、')}</Typography>
        <TextField label="修正原因" value={reason} onChange={event => setReason(event.target.value)} required multiline minRows={2} fullWidth disabled={saving || needsPreview} inputProps={{ maxLength: 500 }} helperText={`必填，将写入审计记录；${reason.trim().length}/500 字`} />
        {preview && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>预览有效期至 {preview.expires_at}；提交时服务器再次核对记录与方法配置。</Typography>}
        {error && <Alert severity="error" sx={{ mt: 1.5 }}>{error}</Alert>}
      </DialogContent>
      <DialogActions sx={{ flexWrap: 'wrap', gap: 0.5 }}>
        <Button onClick={() => setConfirmOpen(false)} disabled={saving}>取消</Button>
        {needsPreview && <Button variant="outlined" onClick={() => void loadPreview(page)} disabled={saving}>重新预览并清除勾选</Button>}
        <Button color="warning" variant="contained" onClick={() => void applyCorrection()} disabled={saving || loading || needsPreview || selectedRows.length === 0 || !reason.trim() || reason.trim().length > 500}>{saving ? '正在修正…' : '确认修正选中记录'}</Button>
      </DialogActions>
    </Dialog>
  </Paper>;
}
