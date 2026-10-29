import React from 'react';
import { Box, Chip, CircularProgress, IconButton, TextField, Typography } from '@mui/material';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import { actionColorSx, type ActionColorPalette } from '../utils/recordTableStyle';
import { FONT } from '../constants/designTokens';

interface WorkloadEntryRowProps {
  name: string;
  title: React.ReactNode;
  coefficient: number | string;
  tags?: React.ReactNode;
  description?: React.ReactNode;
  extraControl?: React.ReactNode;
  quantity: string | number;
  onQuantityChange: (value: string) => void;
  onSubmit: () => void;
  loading: boolean;
  submitted: boolean;
  disabled?: boolean;
  invalid: boolean;
  isMobile?: boolean;
  actionColors: ActionColorPalette;
  fontSize?: number;
  auxFontSize?: number | string;
}

/** 展示和数量交互共用；归属、倍率、快照及写入校验仍由各业务入口负责。 */
export default function WorkloadEntryRow({ name, title, coefficient, tags, description, extraControl, quantity, onQuantityChange, onSubmit, loading, submitted, disabled = false, invalid, isMobile, actionColors, fontSize, auxFontSize = FONT.xs }: WorkloadEntryRowProps) {
  const actionState = submitted ? 'completed' : loading ? 'loading' : disabled || invalid ? 'disabled' : 'idle';
  const direction = isMobile === undefined ? { xs: 'column' as const, sm: 'row' as const } : isMobile ? 'column' : 'row';
  const contentWidth = isMobile === undefined ? { xs: '100%', sm: 'auto' } : isMobile ? '100%' : undefined;
  const controlColumns = extraControl
    ? (isMobile === undefined ? { xs: 'minmax(0,1fr) minmax(0,1fr) 40px', sm: '112px 86px 40px' } : isMobile ? 'minmax(0,1fr) minmax(0,1fr) 40px' : '112px 86px 40px')
    : '86px 40px';
  return <Box data-workload-entry-row sx={{ display: 'flex', alignItems: 'center', gap: 1.5, py: 1.5, px: 2, mb: 1, borderRadius: '2px', flexDirection: direction, background: submitted ? 'linear-gradient(145deg,#e8f5e9,#f1f8e9)' : 'linear-gradient(145deg,#ffffff,#fafafa)', border: '1px solid', borderColor: submitted ? '#a5d6a7' : 'rgba(0,0,0,0.06)', borderLeft: '4px solid', borderLeftColor: submitted ? '#43a047' : '#1e88e5', boxShadow: '0 2px 12px rgba(0,0,0,0.04)', transition: 'all 0.3s', '&:hover': { boxShadow: '0 4px 20px rgba(0,0,0,0.08)', transform: 'translateY(-1px)' } }}>
    <Box sx={{ flex: 1, minWidth: 0, width: contentWidth }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
        <Typography variant="body1" fontWeight={600} sx={{ overflowWrap: 'anywhere', fontSize }}>{title}</Typography>
        <Chip label={`工作量系数 ${coefficient}`} size="small" variant="outlined" sx={{ borderRadius: '2px', height: 22, fontSize: auxFontSize, borderColor: '#1e88e5', color: '#1e88e5', fontWeight: 600 }} />
        {tags}
      </Box>
      {description && <Typography variant="caption" color="text.secondary" sx={{ overflowWrap: 'anywhere', display: 'block', mt: 0.3, fontSize: auxFontSize }}>{description}</Typography>}
    </Box>
    <Box sx={{ display: 'grid', gridTemplateColumns: controlColumns, alignItems: 'center', gap: 1, width: contentWidth, flexShrink: 0 }}>
      {extraControl}
      <TextField type="number" size="small" label="数量" value={quantity} onChange={event => onQuantityChange(event.target.value)} inputProps={{ min: 1, step: 1, 'aria-label': `${name}数量`, style: { textAlign: 'center', minWidth: 0 } }} sx={{ minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: '2px', '& fieldset': { borderColor: '#1976d2' }, '&:hover fieldset': { borderColor: '#0d47a1' } } }} disabled={disabled || loading} onKeyDown={event => { if (event.key === 'Enter' && !disabled && !loading && !submitted && !invalid) onSubmit(); }} />
      <IconButton title={submitted ? '已录入' : '确认录入'} aria-label={`${name}${submitted ? '已录入' : '确认录入'}`} onClick={onSubmit} disabled={disabled || loading || submitted || invalid} sx={{ borderRadius: '50%', bgcolor: submitted ? '#e8f5e9' : '#e3f2fd', color: submitted ? '#43a047' : '#1e88e5', '&:disabled': { color: submitted ? '#43a047' : 'rgba(0,0,0,0.2)', bgcolor: submitted ? '#e8f5e9' : 'transparent' }, ...actionColorSx(actionColors, 'workload', actionState) }} size="medium">
        {loading ? <CircularProgress size={24} sx={{ color: actionColors.workload?.loading?.color || '#1e88e5' }} /> : submitted ? <CheckCircleIcon className="animate-checkmark" /> : <CheckCircleOutlineIcon />}
      </IconButton>
    </Box>
  </Box>;
}
