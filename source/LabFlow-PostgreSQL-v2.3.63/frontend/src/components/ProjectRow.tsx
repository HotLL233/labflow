import React, { useState } from 'react';
import { Box, Typography, TextField, IconButton, CircularProgress, Paper, Chip } from '@mui/material';
import CheckCircleIcon from '@mui/icons-material/CheckCircle'; import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import type { Project } from '../types';

interface ProjectRowProps { project: Project; onSubmit: (projectId: number, quantity: number) => Promise<boolean>; }

const getInstrumentColor = (project: Project): string => {
  const text = (project.group_name + project.name + (project.full_name || '')).toLowerCase();
  if (text.includes('液相')) return '#1e88e5'; if (text.includes('气相')) return '#43a047'; return '#9e9e9e';
};

const typeColorMap: Record<string, 'info'|'success'|'warning'|'primary'|'default'> = {
  '液相': 'info', '气相': 'success', '理化': 'warning', '检测类型': 'primary',
};

const ProjectRow: React.FC<ProjectRowProps> = ({ project, onSubmit }) => {
  const [q, setQ] = useState<number | ''>(''); const [l, setL] = useState(false); const [s, setS] = useState(false);
  const c = getInstrumentColor(project);

  const h = async () => {
    if (q === '' || Number(q) < 1) return; setL(true);
    const ok = await onSubmit(project.id, Number(q));
    setL(false);
    if (ok) { setS(true); setTimeout(() => { setS(false); setQ(''); }, 2000); }
  };

  return (
    <Paper elevation={0} sx={{
      display: 'flex', alignItems: 'center', gap: 1.5, py: 1.5, px: 2, mb: 1, borderRadius: '2px',
      background: s ? 'linear-gradient(145deg,#e8f5e9,#f1f8e9)' : 'linear-gradient(145deg,#ffffff,#fafafa)',
      border: '1px solid', borderColor: s ? '#a5d6a7' : 'rgba(0,0,0,0.06)',
      borderLeft: `4px solid ${s ? '#43a047' : c}`,
      boxShadow: '0 2px 12px rgba(0,0,0,0.04)',
      transition: 'all 0.3s',
      '&:hover': { boxShadow: '0 4px 20px rgba(0,0,0,0.08)', transform: 'translateY(-1px)' }
    }}>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <Typography variant="body1" fontWeight={600} sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {project.name}
          </Typography>
          <Chip label={`×${(project.coefficient ?? 1).toFixed(1)}`} size="small" variant="outlined" sx={{ borderRadius: '2px', height: 22, fontSize: '0.7rem', borderColor: c, color: c, fontWeight: 600 }} />
          {project.method_type && (
            <Chip label={project.method_type} size="small" color={typeColorMap[project.method_type] || 'default'} sx={{ borderRadius: '2px', height: 20, fontSize: '0.65rem' }} />
          )}
        </Box>
        {project.full_name && (
          <Typography variant="caption" color="text.secondary" sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block', mt: 0.3 }}>
            {project.full_name}
          </Typography>
        )}
      </Box>
      <TextField type="number" size="small" value={q} onChange={e => setQ(e.target.value === '' ? '' : Number(e.target.value))}
        inputProps={{ min: 1, style: { textAlign: 'center', width: 60 } }}
        sx={{ width: 80, '& .MuiOutlinedInput-root': { borderRadius: '2px', '& fieldset': { borderColor: 'rgba(0,0,0,0.1)' } } }}
        disabled={l || s} onKeyDown={e => { if (e.key === 'Enter') h(); }} />
      <IconButton onClick={h} disabled={l || s || q === '' || Number(q) < 1}
        sx={{ borderRadius: '50%', bgcolor: s ? '#e8f5e9' : `${c}14`, color: s ? '#43a047' : c, '&:disabled': { color: 'rgba(0,0,0,0.2)', bgcolor: 'transparent' } }} size="medium">
        {l ? <CircularProgress size={24} sx={{ color: c }} /> : s ? <CheckCircleIcon className="animate-checkmark" /> : <CheckCircleOutlineIcon />}
      </IconButton>
    </Paper>
  );
};

export default ProjectRow;
