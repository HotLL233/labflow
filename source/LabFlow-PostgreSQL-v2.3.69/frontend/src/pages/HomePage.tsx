import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, Paper, useMediaQuery, useTheme, Button } from '@mui/material';
import ScienceIcon from '@mui/icons-material/Science';
import BarChartIcon from '@mui/icons-material/BarChart';
import AssignmentIcon from '@mui/icons-material/Assignment';
import { useUser } from '../UserContext';
import { getSetting } from '../api/client';
import type { HomeCard } from '../types';
import { BUSINESS_COLORS, isBusinessKey, type BusinessColor } from '../constants/designTokens';
import AnnouncementSlot from '../components/AnnouncementSlot';


const BORDER_RADIUS = '2px';

/** 卡片标识到业务分类的映射，用于兼容 v2.3.64 之前未保存 business 的旧配置。 */
const CARD_KEY_BUSINESS: Record<string, keyof typeof BUSINESS_COLORS> = {
  sample: 'sample',
  workload: 'analysis',
  'sample-info': 'info',
};

/**
 * v2.3.64：解析卡片业务预设色。按配置里的 business 取，其次按 key 推断，
 * 都无法识别时返回 null。
 * v2.3.65：预设降级为兜底——卡片自身的 gradient/border/titleColor 自由色值
 * 优先于预设生效，三者逐个独立判断，缺哪个字段才回落哪个预设值。
 */
function resolveCardColor(card: HomeCard): BusinessColor | null {
  if (isBusinessKey(card.business)) return BUSINESS_COLORS[card.business];
  const mapped = CARD_KEY_BUSINESS[card.key];
  return mapped ? BUSINESS_COLORS[mapped] : null;
}

// MUI Icon 名称到组件的映射
const iconMap: Record<string, React.ReactNode> = {
  Science: <ScienceIcon sx={{ fontSize: 56, mb: 1.5 }} />,
  BarChart: <BarChartIcon sx={{ fontSize: 56, mb: 1.5 }} />,
  Assignment: <AssignmentIcon sx={{ fontSize: 56, mb: 1.5 }} />,
};

const defaultCards: HomeCard[] = [
  {
    key: 'sample', title: '研发送样', subtitle: '送样录入 · 查看记录', path: '/sample',
    perm: 'entry:sample', icon: 'Science', business: 'sample',
  },
  {
    key: 'workload', title: '分析检测', subtitle: '检测录入 · 统计 · 管理', path: '/workload',
    perm: 'entry:workload', icon: 'BarChart', business: 'analysis',
  },
  {
    key: 'sample-info', title: '样品信息登记', subtitle: 'ICP · 热分析 · 质谱等样品信息填写', path: '/sample-info',
    perm: 'entry:sample-info', icon: 'Assignment', business: 'info',
  },
];

const HomePage: React.FC = () => {
  const navigate = useNavigate();
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));
  const { hasPermission, isLoggedIn } = useUser();
  // 兜底：默认卡片（fetch 失败时保证有内容）
  const [cards, setCards] = useState<HomeCard[]>(defaultCards);
  const [logoText, setLogoText] = useState('知微');
  const [primaryColor, setPrimaryColor] = useState('#667eea');

  useEffect(() => {
    // 读取首页卡片配置
    getSetting('home-cards')
      .then(data => {
        const raw = data?.data?.value;
        if (raw) {
          try {
            const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw;
            if (Array.isArray(parsed) && parsed.length > 0) {
              setCards(parsed);
            } else {
              setCards(defaultCards);
            }
          } catch {
            setCards(defaultCards);
          }
        } else {
          setCards(defaultCards);
        }
      })
      .catch(() => { setCards(defaultCards); });

    // 读取主题配置（logoText / primaryColor）
    fetch('/api/settings/theme')
      .then(response => response.json())
      .then(data => {
        const raw = data?.data?.value;
        if (raw) {
          try {
            const themeConfig = typeof raw === 'string' ? JSON.parse(raw) : raw;
            if (themeConfig.logoText) setLogoText(themeConfig.logoText);
            if (themeConfig.primaryColor) setPrimaryColor(themeConfig.primaryColor);
          } catch {}
        }
      })
      .catch(() => {});
  }, []);

  const visibleCards = cards.filter((card) => hasPermission(card.perm));

  return (
    <Box sx={{ maxWidth: 900, mx: 'auto', mt: { xs: 2, md: 6 } }}>
      <AnnouncementSlot position="home" mode="modal" />
      {/* 编辑模式切换 */}

      {/* Header */}

      <Box sx={{ textAlign: 'center', mb: 5 }}>
        <Typography variant="h3" fontWeight={800} sx={{ fontFamily: '"方正舒体", "FZSJ-KAFJT", "KaiTi", "STKaiti", "cursive", serif', color: primaryColor, mb: 1, fontSize: { xs: '2.8rem', md: '4rem' } }}>
          {logoText}
        </Typography>
      </Box>


      {visibleCards.length === 0 ? (
        <Box sx={{ textAlign: 'center', mt: 6 }}>
          {isLoggedIn ? (
            <Typography variant="h6" color="text.secondary">您暂无可用功能入口，请联系管理员</Typography>
          ) : (
            <>
              <Typography variant="h6" color="text.secondary" sx={{ mb: 2 }}>请先登录系统</Typography>
              <Button variant="contained" onClick={() => navigate('/login')} sx={{ px: 4, py: 1.2, borderRadius: '2px' }}>
                去登录
              </Button>
            </>
          )}
        </Box>
      ) : (

        <Box sx={{ display: 'flex', flexDirection: isMobile ? 'column' : 'row', gap: 3, flexWrap: 'wrap', justifyContent: 'center', alignItems: isMobile ? 'center' : undefined, mb: 4 }}>
          {visibleCards.map((card) => {
            const biz = resolveCardColor(card);
            // v2.3.65：自由色值优先，业务预设兜底（v2.3.64 曾整体反向）。
            const background = card.gradient || (biz ? `linear-gradient(145deg,${biz.soft},${biz.line})` : undefined);
            const borderColor = card.border || (biz ? biz.main : '#e0e0e0');
            const titleColor = card.titleColor || (biz ? biz.main : '#374151');
            return (
              <Paper
                key={card.path}
                elevation={0}
                onClick={() => navigate(card.path)}
                sx={{
                  flex: isMobile ? '1 1 100%' : '1 1 240px', maxWidth: isMobile ? '100%' : 320, p: { xs: 3, md: 4 }, borderRadius: BORDER_RADIUS, cursor: 'pointer',
                  background,
                  border: `2px solid ${borderColor}`,
                  boxShadow: `0 8px 32px ${borderColor}1f`,
                  transition: 'all 0.2s',
                  '&:hover': { transform: 'translateY(-4px)', boxShadow: `0 12px 40px ${borderColor}33` },
                  textAlign: 'center', width: isMobile ? '100%' : undefined,
                }}
              >
                {iconMap[card.icon] ? (
                  React.cloneElement(iconMap[card.icon] as React.ReactElement, { sx: { fontSize: 56, color: titleColor, mb: 1.5 } })
                ) : (
                  <ScienceIcon sx={{ fontSize: 56, color: titleColor, mb: 1.5 }} />
                )}
                <Typography variant="h5" fontWeight={700} sx={{ color: titleColor }} gutterBottom>{card.title}</Typography>
                <Typography variant="body2" color="text.secondary">{card.subtitle}</Typography>
              </Paper>
            );
          })}
        </Box>

      )}


      <Box sx={{ textAlign: 'center' }}>
        <Typography variant="body1" color="text.secondary">选择功能入口，开始操作</Typography>
      </Box>

    </Box>
  );

};
export default HomePage;
