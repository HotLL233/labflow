import { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Chip, CircularProgress, Paper, Stack, Typography } from '@mui/material';
import { confirmAiAction, getAiAction, type AiPublicAction } from '../api/ai';
import { aiActionNeedsRecovery, aiActionRecordRoute, aiActionStatusLabel, asAiAction, canConfirmAiAction } from '../utils/aiActions';

export default function AIActionCard({ action, allowed, disabled, onChange, onBusyChange, onNavigate }: { action: AiPublicAction; allowed: boolean; disabled: boolean; onChange: (action: AiPublicAction) => void; onBusyChange: (busy: boolean) => void; onNavigate: (route: string) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [now, setNow] = useState(Date.now());
  const pending = useRef<{ controller: AbortController; confirming: boolean }>();
  const attempted = useRef(action.status !== 'pending');
  const change = useRef(onChange); change.current = onChange;
  const notifyBusy = useRef(onBusyChange); notifyBusy.current = onBusyChange;
  const current = useRef(action); current.current = action;
  useEffect(() => {
    const remaining = Date.parse(action.expires_at) - Date.now();
    if (remaining <= 0) { setNow(Date.now()); return; }
    const timer = window.setTimeout(() => setNow(Date.now()), Math.min(remaining + 1, 2147483647));
    return () => window.clearTimeout(timer);
  }, [action.expires_at]);
  useEffect(() => () => {
    if (pending.current?.confirming) change.current({ ...current.current, status: 'unknown', can_confirm: false });
    pending.current?.controller.abort();
    notifyBusy.current(false);
  }, []);
  const request = async (confirm: boolean) => {
    if (pending.current || disabled || !allowed || (confirm && (attempted.current || !canConfirmAiAction(action, allowed)))) return;
    const controller = new AbortController(); pending.current = { controller, confirming: confirm };
    setBusy(true); onBusyChange(true); setError('');
    if (confirm) { attempted.current = true; onChange({ ...action, status: 'processing', can_confirm: false }); }
    try {
      const result = asAiAction(await (confirm ? confirmAiAction(action.id, controller.signal) : getAiAction(action.id, controller.signal)));
      if (controller.signal.aborted) return;
      if (!result || result.id !== action.id) throw new Error('服务器返回的提交结果无法核对。');
      if (attempted.current && result.status === 'pending') {
        onChange({ ...result, status: 'unknown', can_confirm: false });
        setError('本批已发起过提交，当前结果仍未明确。请核对已有记录，不要再次提交本批。');
      } else onChange(result);
    } catch (reason) {
      if (controller.signal.aborted) return;
      if (confirm) onChange({ ...action, status: 'unknown', can_confirm: false });
      setError(confirm ? '提交结果暂时无法确认。请先查询本批结果并核对记录，不要重新提交同一批内容。' : reason instanceof Error ? reason.message : '结果读取失败，请稍后查询。');
    } finally {
      if (!controller.signal.aborted) { pending.current = undefined; setBusy(false); onBusyChange(false); }
    }
  };
  const expired = action.expired || Date.parse(action.expires_at) <= now;
  const recover = aiActionNeedsRecovery(action);
  const label = aiActionStatusLabel({ ...action, expired });
  return <Paper variant="outlined" sx={{ mt: 1.25, p: 1.25, bgcolor: 'action.hover', overflowWrap: 'anywhere' }}>
    <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, flexWrap: 'wrap' }}><Typography variant="subtitle2">核对并确认提交</Typography><Chip size="small" label={label} /></Box>
    <Typography variant="body2" sx={{ mt: 0.75 }}>{action.summary}</Typography>
    <Stack spacing={1} sx={{ my: 1 }}>{action.items.map(item => <Box key={item.index} sx={{ borderTop: 1, borderColor: 'divider', pt: 0.75 }}><Typography variant="body2" fontWeight={600}>{item.index + 1}. {item.title}</Typography><Box component="dl" sx={{ m: 0, mt: 0.5, display: 'grid', gridTemplateColumns: 'minmax(60px, 100px) minmax(0, 1fr)', columnGap: 1, rowGap: 0.25 }}>{item.preview.map((field, index) => <Box key={index} sx={{ display: 'contents' }}><Typography component="dt" variant="caption" color="text.secondary">{field.label}</Typography><Typography component="dd" variant="body2" sx={{ m: 0, whiteSpace: 'pre-wrap' }}>{field.value}</Typography></Box>)}</Box>{item.issues.length > 0 && <Alert severity="warning" sx={{ mt: 0.5 }}>{item.issues.join('；')}</Alert>}{action.results.filter(result => result.index === item.index).map(result => {
      const route = result.status === 'succeeded' ? aiActionRecordRoute(result.route) : null;
      return <Alert key={result.index} severity={result.status === 'succeeded' ? 'success' : result.status === 'unknown' ? 'warning' : 'error'} sx={{ mt: 0.75 }}>{result.message || (result.status === 'succeeded' ? '提交成功' : result.status === 'unknown' ? '结果待核对' : '未能提交')}{route && <Button size="small" onClick={() => onNavigate(route)}>查看记录</Button>}</Alert>;
    })}</Box>)}</Stack>
    {error && <Alert severity="warning" sx={{ mb: 1 }}>{error}</Alert>}
    {recover && <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 1 }}>仅查询本批已有结果，不会再次提交。结果未知时请先核对记录。</Typography>}
    {!recover && action.status === 'pending' && <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 1 }}>{expired ? '预览已过期，请重新核对信息后生成新预览。' : '请逐项核对实际人员、来源、数量和计量信息；点击后一次确认整批提交。'}</Typography>}
    {recover ? <Button size="small" variant="outlined" disabled={busy || disabled || !allowed} onClick={() => void request(false)} startIcon={busy ? <CircularProgress size={14} /> : undefined}>{busy ? pending.current?.confirming ? '正在提交' : '正在查询' : '查询本批结果'}</Button>
      : action.status === 'pending' && <Button size="small" variant="contained" disabled={busy || disabled || !canConfirmAiAction(action, allowed, now)} onClick={() => void request(true)} startIcon={busy ? <CircularProgress size={14} /> : undefined}>{busy ? '正在提交' : `确认提交本批（${action.items.length}项）`}</Button>}
  </Paper>;
}
