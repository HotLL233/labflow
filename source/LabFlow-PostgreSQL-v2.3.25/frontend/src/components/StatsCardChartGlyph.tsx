import React from 'react';
import { Box } from '@mui/material';

interface Props {
  type: 'line' | 'bar' | 'pie' | 'stack';
  color: string;
}

const StatsCardChartGlyph: React.FC<Props> = ({ type, color }) => {
  if (type === 'pie') {
    return <Box sx={{ width: 52, height: 52, borderRadius: '50%', background: `conic-gradient(${color} 0 66%, #e2e7eb 66% 100%)`, position: 'relative', '&:after': { content: '""', position: 'absolute', inset: 11, bgcolor: '#fff', borderRadius: '50%' } }} />;
  }
  if (type === 'line') {
    return (
      <svg width="74" height="48" viewBox="0 0 74 48" aria-hidden="true">
        <path d="M2 42H72" stroke="#dfe4e8" /><polyline points="3,38 16,29 28,33 40,17 53,24 70,7" fill="none" stroke={color} strokeWidth="2.5" />
      </svg>
    );
  }
  return (
    <Box sx={{ width: 74, height: 48, display: 'flex', flexDirection: type === 'stack' ? 'column' : 'row', alignItems: type === 'stack' ? 'flex-start' : 'flex-end', justifyContent: 'center', gap: 0.5 }}>
      {[46, 70, 56, 82].map((size, index) => (
        <Box key={index} sx={type === 'stack' ? { height: 7, width: `${size}%`, bgcolor: color, opacity: 0.72 } : { width: 10, height: `${size}%`, bgcolor: color, opacity: 0.72 }} />
      ))}
    </Box>
  );
};

export default StatsCardChartGlyph;
