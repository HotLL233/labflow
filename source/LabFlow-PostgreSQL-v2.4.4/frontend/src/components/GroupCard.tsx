import React from 'react';
import { Paper, Typography, Avatar, Box, Button, useMediaQuery, useTheme } from '@mui/material';
import ScienceIcon from '@mui/icons-material/Science';
import type { ProjectGroup } from '../types';
import { BUSINESS_COLORS, FONT } from '../constants/designTokens';
interface GroupCardProps { group: ProjectGroup; onClick: () => void; themeColor?: string; onViewRecords?: () => void; }
const GroupCard: React.FC<GroupCardProps> = ({ group, onClick, themeColor, onViewRecords }) => { 
  const theme = useTheme(); const isMobile = useMediaQuery(theme.breakpoints.down('sm'));
  // v2.3.64：去掉按 group.id 轮换的十色彩板，未传色时统一回落到分析检测主色。
  const color = themeColor || BUSINESS_COLORS.analysis.main;
  return (<Paper className="card-hover" onClick={onClick} role="button" tabIndex={0} aria-label={`进入${group.name}`} onKeyDown={event => { if (event.target === event.currentTarget && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); onClick(); } }} sx={{ 
    position: 'relative', minWidth: 0,
    p: isMobile ? 1.5 : 3, borderRadius: '2px', textAlign: 'center', cursor: 'pointer', 
    background: 'linear-gradient(145deg,#ffffff,#f5f5f5)', border: '1px solid rgba(0,0,0,0.06)', 
    boxShadow: '0 4px 20px rgba(0,0,0,0.06)', transition: 'all 0.3s', 
    '&:hover': { transform: 'translateY(-4px)', boxShadow: `0 12px 30px ${color}25`, borderColor: `${color}50` }, 
    '&:active': { transform: 'scale(0.98)' }
  }}>
    {!!group.returned_sender_names && (
      <Box sx={{ mr: 3.5, mb: isMobile ? 0.75 : 1.25, px: 0.75, py: 0.4, borderRadius: '2px', bgcolor: '#c62828', color: '#fff', fontSize: FONT.xs, fontWeight: 700, lineHeight: 1.4, textAlign: 'left', overflowWrap: 'anywhere' }}>
        退回：{group.returned_sender_names}
      </Box>
    )}
    {Number(group.rd_record_count) > 0 && (
      <Box sx={{ position: 'absolute', top: 8, right: 8, minWidth: 24, height: 24, borderRadius: '50%', bgcolor: '#f57f17', color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: FONT.sm, fontWeight: 700, zIndex: 1 }}>
        {group.rd_record_count}
      </Box>
    )}
    <Avatar sx={{ width: isMobile ? 32 : 64, height: isMobile ? 32 : 64, mx: 'auto', mb: isMobile ? 1 : 2, bgcolor: `${color}18` }}>
    <ScienceIcon sx={{ fontSize: isMobile ? 20 : 36, color }} />
  </Avatar>
  <Typography variant={isMobile ? 'caption' : 'h6'} fontWeight={700} sx={{ 
    display: 'block', maxWidth: '100%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', mb: isMobile ? 0.5 : 1.5, lineHeight: 1.3 
  }}>{group.name}</Typography>
  {onViewRecords && <Button variant="outlined" size="small" onClick={event => { event.stopPropagation(); onViewRecords(); }} aria-label={`查看${group.name}的送样记录`} sx={{ minHeight: 40, minWidth: 0, width: '100%', px: 0.5, borderRadius: '2px', color, borderColor: `${color}70`, fontSize: isMobile ? '0.68rem' : FONT.base, lineHeight: 1.4 }}>{isMobile ? '送样记录' : '查看送样记录'}</Button>}
  </Paper>); };
export default GroupCard;
