import React from 'react';
import { Paper, Typography, Avatar, Box, useMediaQuery, useTheme } from '@mui/material';
import ReceiptLongIcon from '@mui/icons-material/ReceiptLong';

interface RecordsCardProps { pendingCount: number; onClick: () => void; themeColor?: string; }

const RecordsCard: React.FC<RecordsCardProps> = ({ pendingCount, onClick, themeColor = '#1976d2' }) => {
  const theme = useTheme(); const isMobile = useMediaQuery(theme.breakpoints.down('sm'));
  return (
    <Paper onClick={onClick} sx={{ position: 'relative', p: isMobile ? 1.5 : 3, borderRadius: '2px', textAlign: 'center', cursor: 'pointer', background: 'linear-gradient(145deg,#ffffff,#f5f5f5)', border: '1px solid rgba(0,0,0,0.06)', boxShadow: '0 4px 20px rgba(0,0,0,0.06)', transition: 'all 0.3s', '&:hover': { transform: 'translateY(-4px)', boxShadow: `0 12px 30px ${themeColor}25`, borderColor: `${themeColor}50` } }}>
      {Number(pendingCount) > 0 && (
        <Box sx={{ position: 'absolute', top: 8, right: 8, minWidth: 24, height: 24, borderRadius: '50%', bgcolor: '#f57f17', color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '0.75rem', fontWeight: 700, zIndex: 1 }}>{pendingCount}</Box>
      )}
      <Avatar sx={{ width: isMobile ? 32 : 64, height: isMobile ? 32 : 64, mx: 'auto', mb: isMobile ? 1 : 2, bgcolor: `${themeColor}18` }}>
        <ReceiptLongIcon sx={{ fontSize: isMobile ? 20 : 36, color: themeColor }} />
      </Avatar>
      <Typography variant={isMobile ? 'caption' : 'h6'} fontWeight={700} sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', mb: isMobile ? 0.5 : 1.5, lineHeight: 1.3 }}>
        研发送样记录
      </Typography>
    </Paper>
  );
};
export default RecordsCard;
