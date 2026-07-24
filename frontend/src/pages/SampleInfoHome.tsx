import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, Paper, Grid, useMediaQuery, useTheme, IconButton, Button, CircularProgress, Alert } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import BarChartIcon from '@mui/icons-material/BarChart';
import ScienceIcon from '@mui/icons-material/Science';
import ListAltIcon from '@mui/icons-material/ListAlt';
import { getSampleInfoTypes } from '../api/client';
import type { SampleInfoType } from '../types';
import { useUser } from '../UserContext';


const R = '2px';

// 为不同检测类型分配一个合适的图标（通用科学图标，颜色取自数据库 color 字段）
const pickIcon = () => <ScienceIcon sx={{ fontSize: 48 }} />;

const SampleInfoHome: React.FC = () => {
  const n = useNavigate();
  const { hasPermission } = useUser();
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));

  const [cats, setCats] = useState<SampleInfoType[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    let active = true;
    setLoading(true);
    getSampleInfoTypes()
      .then((r) => { if (active && r.code === 0 && r.data) setCats(r.data); })
      .catch((e: any) => { if (active) setError(e.message || '加载检测类型失败'); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);

  return (
    <Box sx={{ maxWidth: 900, mx: 'auto', mt: { xs: 1, md: 3 }, px: { xs: 1, md: 2 } }}>
      {/* 顶部标题栏 */}
    
      <Box sx={{ display: 'flex', alignItems: 'center', mb: 3, gap: 1 }}>
        <IconButton onClick={() => n('/')} size="small"><ArrowBackIcon /></IconButton>
        <Box sx={{ flex: 1 }}>
          <Typography variant="h5" fontWeight={700} color="#2e7d32">样品信息登记</Typography>
          <Typography variant="body2" color="text.secondary">ICP、热分析、质谱等原样品基础信息填写</Typography>
        </Box>
        {hasPermission('stats:portal:sample-info') && <Button variant="outlined" startIcon={<BarChartIcon />} onClick={() => n('/sample-info/stats')}
          sx={{ borderRadius: R, borderColor: '#2e7d32', color: '#2e7d32', '&:hover': { borderColor: '#1b5e20', bgcolor: 'rgba(46,125,50,0.04)' } }}>
          查看统计
        </Button>}
      </Box>
    
    
      {loading ? (
        <Box sx={{ textAlign: 'center', py: 8 }}><CircularProgress size={36} /></Box>
      ) : error ? (
        <Alert severity="error" sx={{ borderRadius: R }}>{error}</Alert>
      ) : cats.length === 0 ? (
        <Alert severity="info" sx={{ borderRadius: R }}>
          暂无检测类型，请在「管理 → 样品信息登记管理」中添加检测类型。
        </Alert>
      ) : (
        <>
          {/* 大类卡片（从数据库读取） */}
    
          <Grid container spacing={3} sx={{ mb: 4 }}>
            {cats.map((cat) => (
              <Grid item xs={12} sm={6} key={cat.type_key}>
                <Paper
                  elevation={0}
                  onClick={() => n(`/sample-info/entry?type=${encodeURIComponent(cat.type_key)}`)}
                  sx={{
                    p: { xs: 3, md: 4 }, borderRadius: R, cursor: 'pointer',
                    background: `linear-gradient(145deg, ${cat.color}22, ${cat.color}11)`,
                    border: `2px solid ${cat.color}`,
                    boxShadow: `0 8px 32px ${cat.color}22`,
                    transition: 'all 0.2s',
                    '&:hover': { transform: 'translateY(-4px)', boxShadow: `0 12px 40px ${cat.color}33` },
                    textAlign: 'center',
                  }}
                >
                  <Box sx={{ mb: 1.5, color: cat.color }}>{pickIcon()}</Box>
                  <Typography variant="h6" fontWeight={700} color={cat.color} gutterBottom>{cat.label}</Typography>
                  <Typography variant="body2" color="text.secondary">{cat.description || cat.type_key}</Typography>
                </Paper>
              </Grid>
            ))}
          </Grid>
    
        </>
      )}
    
      {/* 底部链接 */}
      <Box sx={{ textAlign: 'center' }}>
        <Paper
          elevation={0}
          onClick={() => n('/sample-info/entry')}
          sx={{
            display: 'inline-flex', alignItems: 'center', gap: 1, px: 3, py: 1.5, borderRadius: R, cursor: 'pointer',
            border: '1px solid #2e7d32', color: '#2e7d32',
            transition: 'all 0.2s',
            '&:hover': { bgcolor: '#e8f5e9' },
          }}
        >
          <ListAltIcon sx={{ fontSize: 22 }} />
          <Typography variant="body2" fontWeight={600}>查看全部登记记录</Typography>
        </Paper>
      </Box>
    </Box>
  );

};
export default SampleInfoHome;
