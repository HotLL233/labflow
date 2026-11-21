import { useEffect, useRef, useState } from 'react';
import { Alert, Autocomplete, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle, LinearProgress, MenuItem, Paper, Stack, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, TextField, Typography } from '@mui/material';
import { getMethods, requireApiSuccess } from '../api/client';
import { applyMethodAssociation, diagnoseWorkloadCoefficients, previewMethodAssociation, type CoefficientDiagnosis, type CoefficientDiagnosisRow, type MethodAssociationPreview } from '../api/workloadGovernance';
import type { Method } from '../types';
import { beijingRecordToday, recordDatePreset, validateRecordDateRange } from '../utils/recordDateRange';

const number = (value: number | null | undefined) => value == null || !Number.isFinite(value) ? '未知' : value.toLocaleString('zh-CN', { maximumFractionDigits: 12 });
const methodLabel = (method: Method) => `${method.full_name || method.name} · ${method.instrument_name || method.instrument_code || '未绑定仪器'} · 系数 ${number(method.coefficient)}${method.is_active ? '' : ' · 已停用'}`;
const failureText = (error: unknown) => error instanceof Error ? error.message : '操作失败，请重新查询';

export default function WorkloadCoefficientDiagnostics({ onChanged }: { onChanged?: () => void }) {
  const [filters, setFilters] = useState(() => ({ ...recordDatePreset('recent-30', beijingRecordToday()), keyword: '', status: '' }));
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [diagnosis, setDiagnosis] = useState<CoefficientDiagnosis | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [associationRow, setAssociationRow] = useState<CoefficientDiagnosisRow | null>(null);
  const [methods, setMethods] = useState<Method[]>([]);
  const [selectedMethod, setSelectedMethod] = useState<Method | null>(null);
  const [associationPreview, setAssociationPreview] = useState<MethodAssociationPreview | null>(null);
  const [associationLoading, setAssociationLoading] = useState(false);
  const [reason, setReason] = useState('');
  const mounted = useRef(true);
  const sequence = useRef(0);
  const associationSequence = useRef(0);
  const controller = useRef<AbortController | null>(null);
  const associationController = useRef<AbortController | null>(null);
  const pending = useRef(false);
  const confirmation = useRef<MethodAssociationPreview | null>(null);
  const rangeError = validateRecordDateRange(filters.start, filters.end);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; sequence.current++; associationSequence.current++; confirmation.current = null; controller.current?.abort(); associationController.current?.abort(); };
  }, []);

  const clearAssociation = () => {
    associationSequence.current++; associationController.current?.abort(); confirmation.current = null;
    setAssociationRow(null); setSelectedMethod(null); setAssociationPreview(null); setReason(''); setAssociationLoading(false);
  };
  const changeFilters = (patch: Partial<typeof filters>) => {
    if (pending.current) return;
    sequence.current++; controller.current?.abort(); clearAssociation();
    setFilters(previous => ({ ...previous, ...patch })); setDiagnosis(null); setPage(0); setLoading(false); setError(''); setMessage('');
  };
  const loadDiagnosis = async (nextPage = page, nextSize = pageSize) => {
    if (pending.current) return;
    const invalid = validateRecordDateRange(filters.start, filters.end);
    if (invalid) { setError(invalid); return; }
    const request = ++sequence.current;
    controller.current?.abort(); clearAssociation();
    const abort = new AbortController(); controller.current = abort;
    const fresh = () => mounted.current && sequence.current === request && !abort.signal.aborted;
    setLoading(true); setDiagnosis(null); setPage(nextPage); setError('');
    try {
      const result = await diagnoseWorkloadCoefficients({ ...filters, status: filters.status || undefined, keyword: filters.keyword.trim(), page: nextPage + 1, page_size: nextSize }, abort.signal);
      if (!fresh()) return;
      if (nextPage > 0 && nextPage * nextSize >= result.total) { await loadDiagnosis(Math.max(0, Math.ceil(result.total / nextSize) - 1), nextSize); return; }
      setDiagnosis(result);
    } catch (failure) { if (fresh()) setError(failureText(failure)); }
    finally { if (fresh()) setLoading(false); }
  };
  const openAssociation = async (row: CoefficientDiagnosisRow) => {
    if (pending.current || loading || !row.can_reassociate) return;
    clearAssociation(); const request = ++associationSequence.current;
    setAssociationRow(row); setMethods([]); setAssociationLoading(true); setError('');
    try {
      const result = requireApiSuccess(await getMethods(), '方法库加载失败');
      if (mounted.current && request === associationSequence.current) setMethods(result || []);
    } catch (failure) { if (mounted.current && request === associationSequence.current) setError(failureText(failure)); }
    finally { if (mounted.current && request === associationSequence.current) setAssociationLoading(false); }
  };
  const chooseMethod = (method: Method | null) => {
    if (pending.current) return;
    associationSequence.current++; associationController.current?.abort(); confirmation.current = null;
    setSelectedMethod(method); setAssociationPreview(null); setReason(''); setAssociationLoading(false); setError('');
  };
  const previewAssociation = async () => {
    if (pending.current || associationLoading || !associationRow || !selectedMethod || selectedMethod.id === associationRow.method_id) return;
    const request = ++associationSequence.current;
    associationController.current?.abort();
    const abort = new AbortController(); associationController.current = abort;
    const fresh = () => mounted.current && request === associationSequence.current && !abort.signal.aborted;
    confirmation.current = null; setAssociationLoading(true); setAssociationPreview(null); setReason(''); setError('');
    try {
      const result = await previewMethodAssociation({ record_id: associationRow.id, method_id: selectedMethod.id }, abort.signal);
      if (!fresh()) return;
      if (result.record_id !== associationRow.id || result.new_method_id !== selectedMethod.id || result.before_workload !== result.after_workload) throw new Error('关联预览与所选记录不一致，请重新查询');
      confirmation.current = result; setAssociationPreview(result);
    } catch (failure) { if (fresh()) setError(failureText(failure)); }
    finally { if (fresh()) setAssociationLoading(false); }
  };
  const applyAssociation = async () => {
    if (!mounted.current || pending.current || associationLoading || !associationPreview || confirmation.current !== associationPreview || !associationRow || !selectedMethod || !reason.trim() || reason.trim().length > 500) return;
    if (associationPreview.record_id !== associationRow.id || associationPreview.new_method_id !== selectedMethod.id || !Number.isFinite(Date.parse(associationPreview.expires_at)) || Date.parse(associationPreview.expires_at) <= Date.now()) { confirmation.current = null; setAssociationPreview(null); setError('关联预览已失效，请重新预览'); return; }
    pending.current = true; setSaving(true); setError('');
    try {
      const result = await applyMethodAssociation({ confirmation_token: associationPreview.confirmation_token, reason: reason.trim() });
      if (!mounted.current) return;
      if (result.updated_count !== 1) throw new Error('未收到完整关联结果，请重新诊断核对');
      setMessage('方法关联已调整，原名称、仪器、人员、日期及系数快照均保留。请重新预览系数差异，单独决定是否修正。');
      clearAssociation(); onChanged?.(); pending.current = false; setSaving(false); await loadDiagnosis(page);
    } catch (failure) { if (mounted.current) { confirmation.current = null; setAssociationPreview(null); setError(`${failureText(failure)}。结果不确定时请重新诊断核对，不要重复提交。`); } }
    finally { pending.current = false; if (mounted.current) setSaving(false); }
  };

  return <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mb: 2, minWidth: 0, borderRadius: '2px' }}>
    <Typography variant="h6" fontWeight={800}>历史系数全量诊断</Typography>
    <Typography variant="body2" color="text.secondary" sx={{ my: 1 }}>检查所选日期内工作记录及独立辅助，区分系数一致、可修正差异、未关联、已删除方法和辅助来源。“0条差异”不代表所有历史记录已核对。无法归入日期范围的无效日期会单列异常。</Typography>
    <Box component="form" onSubmit={event => { event.preventDefault(); void loadDiagnosis(0); }} sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mb: 1.5 }}>
      <TextField size="small" type="date" label="开始日期" value={filters.start} disabled={saving} InputLabelProps={{ shrink: true }} onChange={event => changeFilters({ start: event.target.value })} />
      <TextField size="small" type="date" label="结束日期" value={filters.end} disabled={saving} InputLabelProps={{ shrink: true }} onChange={event => changeFilters({ end: event.target.value })} />
      <TextField size="small" label="关键词" value={filters.keyword} disabled={saving} onChange={event => changeFilters({ keyword: event.target.value })} inputProps={{ maxLength: 100 }} sx={{ minWidth: { xs: '100%', sm: 220 } }} />
      <TextField select size="small" label="诊断状态" value={filters.status} disabled={saving} onChange={event => changeFilters({ status: event.target.value })} sx={{ minWidth: 170 }}><MenuItem value="">全部状态</MenuItem>{[['difference', '系数差异'], ['equal', '系数一致'], ['unbound', '未关联'], ['method_deleted', '方法已删除'], ['method_missing', '方法失联'], ['custom', '明确自定义'], ['auxiliary', '辅助工作'], ['record_deleted', '记录已删除'], ['invalid', '数据异常']].map(([key, label]) => <MenuItem key={key} value={key}>{label}</MenuItem>)}</TextField>
      <Button type="submit" variant="outlined" disabled={loading || saving || !!rangeError}>全量诊断</Button>
    </Box>
    {rangeError && <Alert severity="error" sx={{ mb: 1 }}>{rangeError}</Alert>}
    {message && <Alert severity="success" sx={{ mb: 1 }}>{message}</Alert>}
    {error && <Alert severity="error" sx={{ mb: 1 }}>{error}</Alert>}
    {loading && <LinearProgress />}
    {!diagnosis && !loading && <Typography variant="body2" color="text.secondary">查询只读取记录，不会自动更换方法或重算历史。</Typography>}
    {diagnosis && <>
      <Stack direction="row" gap={0.75} flexWrap="wrap" sx={{ my: 1 }}>{diagnosis.counts.map(item => <Chip key={item.status} size="small" variant="outlined" label={`${item.label} ${item.count}`} />)}</Stack>
      <TableContainer><Table size="small" aria-label="历史系数全量诊断" sx={{ minWidth: 650, '& td, & th': { verticalAlign: 'top', overflowWrap: 'anywhere' } }}><TableHead><TableRow><TableCell>记录 / 人员 / 日期</TableCell><TableCell>历史方法 / 当前关联</TableCell><TableCell>数量 / 原系数 / 当前系数</TableCell><TableCell>诊断结果</TableCell><TableCell>操作</TableCell></TableRow></TableHead><TableBody>
        {diagnosis.rows.map(row => <TableRow key={`${row.origin}:${row.id}`}><TableCell>{row.business_no || '历史记录'}<Typography variant="caption" display="block">{row.user_name} · {row.recorded_at.replace('T', ' ').slice(0, 16)}</Typography></TableCell><TableCell>{row.method_name || '未记录方法'}<Typography variant="caption" display="block">当前：{row.current_method_name || '未关联'}{row.method_active === false ? '（停用）' : ''}</Typography><Typography variant="caption">{row.instrument_name || '未绑定仪器'}</Typography></TableCell><TableCell>{number(row.quantity)} / {number(row.coefficient_snapshot)} / {number(row.method_coefficient)}<Typography variant="caption" display="block">原工作量 {number(row.before_workload)}</Typography></TableCell><TableCell>{row.status_label}</TableCell><TableCell>{row.can_reassociate && <Button size="small" variant="outlined" disabled={loading || saving} onClick={() => void openAssociation(row)}>核对方法关联</Button>}</TableCell></TableRow>)}
        {!diagnosis.rows.length && <TableRow><TableCell colSpan={5} align="center">当前筛选无记录</TableCell></TableRow>}
      </TableBody></Table></TableContainer>
      <TablePagination component="div" count={diagnosis.total} page={page} rowsPerPage={pageSize} rowsPerPageOptions={[20, 50, 100, 200, 500]} onPageChange={(_, next) => void loadDiagnosis(next)} onRowsPerPageChange={event => { const size = Number(event.target.value); setPageSize(size); void loadDiagnosis(0, size); }} labelRowsPerPage="每页" backIconButtonProps={{ disabled: loading || saving || page === 0 }} nextIconButtonProps={{ disabled: loading || saving || (page + 1) * pageSize >= diagnosis.total }} SelectProps={{ disabled: loading || saving }} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap' } }} />
    </>}
    <Dialog open={!!associationRow} onClose={saving ? undefined : clearAssociation} fullWidth maxWidth="sm"><DialogTitle>核对历史方法关联</DialogTitle><DialogContent dividers>
      <Alert severity="warning" sx={{ mb: 1.5 }}>原方法名称、仪器、人员、部门、日期及工作量快照保留，但方法维度和按当前方法单价计算的金额报表可能变化。请核对预计金额影响；系数仍须单独预览确认。</Alert>
      <Typography variant="body2" sx={{ mb: 1 }}>{associationRow?.business_no || '历史记录'} · {associationRow?.user_name} · 原方法：{associationRow?.method_name || '未记录'}</Typography>
      <Autocomplete options={methods} getOptionKey={method => method.id} getOptionLabel={methodLabel} isOptionEqualToValue={(a, b) => a.id === b.id} value={selectedMethod} onChange={(_, method) => chooseMethod(method)} disabled={saving || associationLoading} renderInput={input => <TextField {...input} label="选择真实方法（名称、仪器、系数）" size="small" />} />
      {associationLoading && <LinearProgress sx={{ mt: 1 }} />}
      {associationPreview && <Box sx={{ mt: 1.5 }}>
        <Typography variant="body2">关联：{associationPreview.old_method_name || '未关联'} → {associationPreview.new_method_name}</Typography>
        <Typography variant="body2">保留原系数 {number(associationPreview.coefficient_snapshot)}，工作量 {number(associationPreview.before_workload)} → {number(associationPreview.after_workload)}</Typography>
        <Typography variant="body2" sx={{ mt: 1 }}>单价（原 → 新）：{number(associationPreview.old_unit_price)} → {number(associationPreview.new_unit_price)}；原金额倍率：{number(associationPreview.multiplier)}</Typography>
        <Typography variant="body2">预计金额（关联前 → 关联后）：{number(associationPreview.before_amount)} → {number(associationPreview.after_amount)}</Typography>
        <Typography variant="body2">预计金额差额：{associationPreview.amount_difference != null && Number.isFinite(associationPreview.amount_difference) ? `${associationPreview.amount_difference > 0 ? '+' : ''}${number(associationPreview.amount_difference)}` : '无法核算'}</Typography>
        {(associationPreview.old_unit_price == null || associationPreview.new_unit_price == null) && <Alert severity="info" sx={{ mt: 1 }}>单价缺失，预计金额无法核算；未知值不会按0计算。</Alert>}
        <TextField label="关联调整原因" required fullWidth multiline minRows={2} size="small" value={reason} disabled={saving} onChange={event => setReason(event.target.value)} inputProps={{ maxLength: 500 }} sx={{ mt: 1.5 }} />
      </Box>}
      {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
    </DialogContent><DialogActions><Button disabled={saving} onClick={clearAssociation}>关闭</Button><Button variant="outlined" disabled={saving || associationLoading || !selectedMethod || selectedMethod.id === associationRow?.method_id} onClick={() => void previewAssociation()}>预览关联变化</Button><Button variant="contained" color="warning" disabled={saving || associationLoading || !associationPreview || !reason.trim()} onClick={() => void applyAssociation()}>{saving ? '确认中…' : '确认调整关联'}</Button></DialogActions></Dialog>
  </Paper>;
}
