import React, { useState, useEffect } from 'react';
import { Box, Paper, Typography } from '@mui/material';
import BarChartIcon from '@mui/icons-material/BarChart';
import ReceiptLongIcon from '@mui/icons-material/ReceiptLong';
import PeopleIcon from '@mui/icons-material/People';
import FolderIcon from '@mui/icons-material/Folder';
import ScienceIcon from '@mui/icons-material/Science';
import MemoryIcon from '@mui/icons-material/Memory';
import type { StatsSummary, TypeStats, ProjectStats, InstrumentStats, UserStats, StatCard } from '../types';
import type { TabValue } from '../pages/StatsPage';
import StatsMiniChart, { type MiniChartPoint } from './StatsMiniChart';
import { getSetting } from '../api/client';
import { BUSINESS_COLORS, type BusinessColor } from '../constants/designTokens';

interface StatsCardsProps {
  summary: StatsSummary;
  trendDetails?: StatsSummary['details'];
  byType?: TypeStats[];
  byProject?: ProjectStats[];
  byInstrument?: InstrumentStats[];
  byUser?: UserStats[];
  onCardClick?: (tab: TabValue) => void;
  themeColor?: string;
  variant?: 'workload' | 'rd';
  activeTab?: TabValue | null;
  canOpenTab?: (tab: TabValue) => boolean;
  compact?: boolean;
}

interface CardItemDef {
  key: string;
  label: string;
  color: string;
  gradient: string;
  icon: React.ReactNode;
  tab: TabValue;
  value: number;
  subtitle?: string;
  chartType: 'line' | 'bar' | 'pie';
  chartData: MiniChartPoint[];
}

const fallbackIconMap: Record<string, React.ReactNode> = {
  total: <BarChartIcon />,
  records: <ReceiptLongIcon />,
  users: <PeopleIcon />,
  project: <FolderIcon />,
  project_count: <FolderIcon />,
  lab: <ScienceIcon />,
  lab_count: <ScienceIcon />,
  type: <MemoryIcon />,
  type_count: <MemoryIcon />,
};

/**
 * v2.3.64：统计卡默认配色统一走业务色的同色系深浅渐变。
 * v2.3.65：恢复每卡自定义——配置里填了色值就以配置为准，
 * 该字段没填才回落业务预设。
 */
const themedCard = (label: string, biz: BusinessColor): Omit<StatCard, 'key'> => ({
  label,
  color: biz.main,
  gradient: `linear-gradient(135deg,${biz.main},${biz.accent})`,
});

const variantCards: Record<'workload' | 'rd', Record<string, Omit<StatCard, 'key'>>> = {
  workload: {
    total: themedCard('检测数量', BUSINESS_COLORS.analysis),
    records: themedCard('检测记录', BUSINESS_COLORS.analysis),
    users: themedCard('检测人数', BUSINESS_COLORS.analysis),
    project_count: themedCard('项目数', BUSINESS_COLORS.analysis),
    lab_count: themedCard('实验室数', BUSINESS_COLORS.analysis),
    type_count: themedCard('方法类型', BUSINESS_COLORS.analysis),
  },
  rd: {
    total: themedCard('送样数量', BUSINESS_COLORS.sample),
    records: themedCard('送样记录', BUSINESS_COLORS.sample),
    users: themedCard('送样人数', BUSINESS_COLORS.sample),
    project_count: themedCard('项目数', BUSINESS_COLORS.sample),
    lab_count: themedCard('承接实验室', BUSINESS_COLORS.sample),
    type_count: themedCard('方法类型', BUSINESS_COLORS.sample),
  },
};

/**
 * 迷你图色板：图表需要区分数据系列，因此给每条业务线一组同色相六阶，
 * 而不是跨色相的任意六色。扇区之间仍有明度差，可分辨且不脱离业务色。
 */
const miniChartPalettes: Record<'workload' | 'rd', string[]> = {
  workload: ['#1A237E', '#283593', '#303F9F', '#3949AB', '#5C6BC0', '#9FA8DA'],
  rd: ['#7C2D12', '#9A3412', '#C2410C', '#EA580C', '#F97316', '#FDBA74'],
};

const defaultCardItems = (variant: 'workload' | 'rd'): StatCard[] =>
  ['total', 'records', 'users'].map(key => ({ key, ...variantCards[variant][key] }));

/**
 * v2.3.65：加载已保存的统计卡配置。自定义色值/显示名优先，
 * 对应字段留空时才回落业务预设。
 * 注意：v2.3.63/v2.3.64 此函数曾用内置值整体覆盖配置，
 * 导致管理页填的颜色和显示名都不生效（死配置），本版恢复配置优先。
 */
const applyVariantCards = (cards: StatCard[], variant: 'workload' | 'rd'): StatCard[] =>
  cards.map(card => {
    const themed = variantCards[variant][card.key];
    if (!themed) return card;
    return {
      ...card,
      label: card.label || themed.label,
      color: card.color || themed.color,
      gradient: card.gradient || themed.gradient,
    };
  });

const tabMap: Record<string, TabValue> = {
  total: 'week',
  records: 'user-log',
  users: 'sheet6',
  project: 'sheet8',
  project_count: 'sheet8',
  lab: 'sheet7',
  lab_count: 'sheet7',
  type: 'sheet9',
  type_count: 'sheet9',
};

const valueMap = (summary: StatsSummary, key: string, labCount: number, typeCount: number): number => {
  switch (key) {
    case 'total': return summary.total_quantity;
    case 'records': return summary.total_records;
    case 'users': return summary.user_count;
    case 'project':
    case 'project_count': return summary.project_count;
    case 'lab':
    case 'lab_count': return labCount;
    case 'type':
    case 'type_count': return typeCount;
    default: return 0;
  }
};

const StatsCards: React.FC<StatsCardsProps> = ({
  summary,
  trendDetails,
  byType,
  byProject,
  byInstrument,
  byUser,
  onCardClick,
  themeColor,
  variant = 'workload',
  activeTab,
  canOpenTab,
  compact = false,
}) => {
  const [statCards, setStatCards] = useState<StatCard[]>(() => defaultCardItems(variant));
  const typeCount = byType?.length || 0;
  const projectCount = byProject?.length || 0;
  const labCount = new Set(byProject?.map(p => p.group_name).filter(Boolean)).size;
  const labTotals = [...(byProject || []).reduce((map, item) => {
    const name = item.group_name || '未分组';
    map.set(name, (map.get(name) || 0) + item.total_quantity);
    return map;
  }, new Map<string, number>())].map(([name, value]) => ({ name, value }));

  const miniChart = (key: string): { type: 'line' | 'bar' | 'pie'; data: MiniChartPoint[] } => {
    if (key === 'total') return { type: 'line', data: (trendDetails || summary.details || []).map(item => ({ name: item.period, value: item.total_quantity })) };
    if (key === 'records') return { type: 'bar', data: (trendDetails || summary.details || []).map(item => ({ name: item.period, value: item.record_count })) };
    if (key === 'users') return { type: 'bar', data: (byUser || []).slice(0, 8).map(item => ({ name: item.user_name, value: item.total_quantity })) };
    if (key === 'project' || key === 'project_count') return { type: 'bar', data: (byProject || []).slice(0, 8).map(item => ({ name: item.project_name, value: item.total_quantity })) };
    if (key === 'lab' || key === 'lab_count') return { type: 'pie', data: labTotals };
    if (key === 'type' || key === 'type_count') return { type: 'pie', data: (byType || []).map(item => ({ name: item.instrument_type, value: item.total_quantity })) };
    return { type: 'line', data: (byInstrument || []).slice(0, 8).map(item => ({ name: item.instrument, value: item.total_quantity })) };
  };

  useEffect(() => {
    getSetting('stats-cards')
      .then(d => {
        if (d.data?.value) {
          try { setStatCards(applyVariantCards(JSON.parse(d.data.value), variant)); } catch {}
        }
      })
      .catch(() => setStatCards(defaultCardItems(variant)));
  }, [variant]);

  const cardItems: CardItemDef[] = statCards.map(sc => {
    const chart = miniChart(sc.key);
    return {
      key: sc.key,
      label: sc.label,
      color: sc.color,
      gradient: sc.gradient,
      icon: fallbackIconMap[sc.key] || <BarChartIcon />,
      tab: tabMap[sc.key] || 'week',
      value: valueMap(summary, sc.key, labCount, typeCount),
      chartType: chart.type,
      chartData: chart.data,
    };
  });

  if (statCards.length <= 3) {
    const extraTheme = variantCards[variant];
    const extraBuiltins: CardItemDef[] = [
      { key: 'project_count', label: extraTheme.project_count.label, color: extraTheme.project_count.color, gradient: extraTheme.project_count.gradient, icon: <FolderIcon />, tab: 'sheet8', value: summary.project_count, chartType: 'bar', chartData: miniChart('project_count').data },
      { key: 'lab_count', label: extraTheme.lab_count.label, color: extraTheme.lab_count.color, gradient: extraTheme.lab_count.gradient, icon: <ScienceIcon />, tab: 'sheet7', value: labCount, subtitle: `${projectCount} 个项目`, chartType: 'pie', chartData: miniChart('lab_count').data },
      { key: 'type_count', label: extraTheme.type_count.label, color: extraTheme.type_count.color, gradient: extraTheme.type_count.gradient, icon: <MemoryIcon />, tab: 'sheet9', value: typeCount, subtitle: byType?.map(t => `${t.instrument_type}:${t.total_quantity}`).join(', '), chartType: 'pie', chartData: miniChart('type_count').data },
    ];
    extraBuiltins.forEach(eb => cardItems.push(eb));
  }

  return (
    <Box data-testid={compact ? 'stats-kpis' : undefined} sx={{ display: 'grid', gridTemplateColumns: compact ? { xs: 'repeat(2, minmax(0,1fr))', sm: 'repeat(3, minmax(0,1fr))', md: 'repeat(6, minmax(0,1fr))' } : { xs: 'repeat(2, 1fr)', sm: 'repeat(4, 1fr)', md: 'repeat(6, 1fr)' }, gap: compact ? { xs: 1, sm: 1.5 } : 2, mb: compact ? 2 : 3, px: compact ? 0 : 1 }}>
      {cardItems.map(({ key, label, color, gradient, icon, tab, value, subtitle, chartType, chartData }) => {
        const clickable = canOpenTab?.(tab) ?? true;
        return (
          <Paper
            key={key}
            elevation={0}
            role={compact && onCardClick ? 'button' : undefined}
            tabIndex={compact && onCardClick ? (clickable ? 0 : -1) : undefined}
            aria-disabled={compact && onCardClick ? !clickable : undefined}
            aria-pressed={compact && onCardClick ? activeTab === tab : undefined}
            aria-label={compact ? `${label}：${value}${clickable ? '' : '，无此报表查看权限'}` : undefined}
            onClick={() => clickable && onCardClick?.(tab)}
            onKeyDown={compact ? event => {
              if (clickable && onCardClick && (event.key === 'Enter' || event.key === ' ')) {
                event.preventDefault();
                onCardClick(tab);
              }
            } : undefined}
            sx={{
              p: compact ? { xs: 1.25, sm: 1.5 } : 2,
              minHeight: compact ? { xs: 112, md: 126 } : 178,
              minWidth: compact ? 0 : undefined,
              borderRadius: '2px',
              cursor: onCardClick && clickable ? 'pointer' : 'default',
              position: 'relative',
              overflow: 'hidden',
              background: compact ? '#fff' : 'linear-gradient(145deg, #ffffff, #f5f5f5)',
              border: activeTab === tab ? `2px solid ${color}` : '1px solid rgba(0,0,0,0.06)',
              borderTop: `3px solid ${color}`,
              boxShadow: compact ? 'none' : activeTab === tab ? `0 0 0 2px ${color}18, 0 8px 24px ${color}20` : '0 4px 20px rgba(0,0,0,0.06)',
              opacity: clickable ? 1 : 0.55,
              transition: compact ? 'border-color 0.15s' : 'all 0.3s cubic-bezier(0.4,0,0.2,1)',
              '&:hover': clickable ? compact ? { borderColor: color, bgcolor: '#fbfcfe' } : { transform: 'translateY(-4px)', boxShadow: `0 12px 30px ${color}25` } : {},
              '&:focus-visible': compact ? { outline: `2px solid ${color}`, outlineOffset: 2 } : {},
            }}
          >
            {!compact && <Box sx={{ position: 'absolute', top: -8, right: -8, width: 40, height: 40, borderRadius: '50%', background: gradient, opacity: 0.1 }} />}
            <Box sx={{ display: 'flex', alignItems: 'center', gap: compact ? 0.75 : 1.25, mb: compact ? 0.5 : 1 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: compact ? 0 : undefined, width: compact ? 20 : 36, height: compact ? 20 : 36, borderRadius: '2px', background: compact ? 'transparent' : `${color}16`, color, ...(compact ? { '& svg': { fontSize: 17 } } : {}) }}>{icon}</Box>
              <Typography variant="body2" color="text.secondary" fontWeight={500} sx={compact ? { fontSize: { xs: 11, sm: 12 }, overflowWrap: 'anywhere', lineHeight: 1.4 } : undefined}>{label}</Typography>
            </Box>
            <Typography variant="h4" fontWeight={compact ? 600 : 800} sx={{ background: gradient, WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text', ...(compact ? { fontSize: { xs: 25, sm: 27 }, lineHeight: 1.2, overflowWrap: 'anywhere' } : {}) }}>{String(value)}</Typography>
            {subtitle && <Typography variant="caption" color="text.secondary" title={subtitle} sx={{ mt: 0.25, display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{subtitle}</Typography>}
            <Box sx={compact ? { mt: 0.75, minWidth: 0 } : { position: 'absolute', left: 14, right: 14, bottom: 7 }}><StatsMiniChart type={chartType} data={chartData} color={color} palette={miniChartPalettes[variant]} height={compact ? 32 : 40} /></Box>
          </Paper>
        );
      })}
    </Box>
  );
};

export default StatsCards;
