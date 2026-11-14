import { useState } from 'react';
import { Alert, Box, Button, Typography } from '@mui/material';
import { aiDraftFieldLabels, type AiEntryDraft } from '../utils/aiDraftBridge';

/** Transfer needs a second, explicit review in the original form. Never submits. */
export default function AiDraftReview({ draft, onApply, onDismiss, disabled = false }: {
  draft: AiEntryDraft | null;
  onApply: (draft: AiEntryDraft) => void | Promise<void>;
  onDismiss: () => void;
  disabled?: boolean;
}) {
  const [error, setError] = useState('');
  const [applying, setApplying] = useState(false);
  if (!draft) return null;
  return <Alert severity="info" sx={{ mb: 2 }}>
    <Typography fontWeight={700}>{draft.title} · 尚未提交</Typography>
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: '4px 18px', my: 1 }}>
      {Object.entries(draft.labels).map(([key, value]) => <Typography key={key} variant="body2" sx={{ overflowWrap: 'anywhere' }}>{aiDraftFieldLabels[key] || key}：{String(value) || '待填写'}</Typography>)}
    </Box>
    {draft.issues.map((issue, index) => <Typography key={index} variant="body2" color="warning.main">{issue}</Typography>)}
    <Typography variant="body2">核对后添加到表单，再使用原来的提交按钮。已有输入会保留。</Typography>
    {error && <Typography role="alert" color="error" variant="body2">{error}</Typography>}
    <Box sx={{ display: 'flex', gap: 1, mt: 1 }}>
      <Button size="small" variant="outlined" disabled={disabled || applying} onClick={async () => { setApplying(true); setError(''); try { await onApply(draft); onDismiss(); } catch (reason) { setError(reason instanceof Error ? reason.message : '添加失败，请重新核对'); } finally { setApplying(false); } }}>核对并添加到表单</Button>
      <Button size="small" disabled={applying} onClick={onDismiss}>放弃此草稿</Button>
    </Box>
  </Alert>;
}
