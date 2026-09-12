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

const variantCards: Record<'workload' | 'rd', Record<string, Omit<StatCard, 'key'>>> = {
  workload: {
    total: { label: '检测数量', color: '#1d4ed8', gradient: 'linear-gradient(135deg,#1d4ed8,#0891b2)' },
    records: { label: '检测记录', color: '#0f766e', gradient: 'linear-gradient(135deg,#0f766e,#14b8a6)' },
    users: { label: '检测人数', color: '#4338ca', gradient: 'linear-gradient(135deg,#4338ca,#6366f1)' },
    project_count: { label: '项目数', color: '#0369a1', gradient: 'linear-gradient(135deg,#0369a1,#38bdf8)' },
    lab_count: { label: '实验室数', color: '#0e7490', gradient: 'linear-gradient(135deg,#0e7490,#22d3ee)' },
    type_count: { label: '方法类型', color: '#4f46e5', gradient: 'linear-gradient(135deg,#4f46e5,#818cf8)' },
  },
  rd: {
    total: { label: '送样数量', color: '#ea580c', gradient: 'linear-gradient(135deg,#ea580c,#f59e0b)' },
    records: { label: '送样记录', color: '#16a34a', gradient: 'linear-gradient(135deg,#16a34a,#65a30d)' },
    users: { label: '送样人数', color: '#b45309', gradient: 'linear-gradient(135deg,#b45309,#fbbf24)' },
    project_count: { label: '项目数', color: '#d97706', gradient: 'linear-gradient(135deg,#d97706,#f97316)' },
    lab_count: { label: '承接实验室', color: '#059669', gradient: 'linear-gradient(135deg,#059669,#22c55e)' },
    type_count: { label: '方法类型', color: '#dc2626', gradient: 'linear-gradient(135deg,#dc2626,#f97316)' },
  },
};

const miniChartPalettes: Record<'workload' | 'rd', string[]> = {
  workload: ['#1d4ed8', '#0891b2', '#4338ca', '#0f766e', '#0369a1', '#4f46e5'],
  rd: ['#ea580c', '#16a34a', '#f59e0b', '#059669', '#b45309', '#dc2626'],
};

const defaultCardItems = (variant: 'workload' | 'rd'): StatCard[] =>
  ['total', 'records', 'users'].map(key => ({ key, ...variantCards[variant][key] }));

const applyVariantCards = (cards: StatCard[], variant: 'workload' | 'rd'): StatCard[] =>
  cards.map(card => {
    const themed = variantCards[variant][card.key];
    return themed ? { ...card, label: themed.label, color: themed.color, gradient: themed.gradient } : card;
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
    fetch('/api/settings/stats-cards')
      .then(r => r.json())
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
    <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2, 1fr)', sm: 'repeat(4, 1fr)', md: 'repeat(6, 1fr)' }, gap: 2, mb: 3, px: 1 }}>
      {cardItems.map(({ key, label, color, gradient, icon, tab, value, subtitle, chartType, chartData }) => {
        const clickable = canOpenTab?.(tab) ?? true;
        return (
          <Paper
            key={key}
            elevation={0}
            onClick={() => clickable && onCardClick?.(tab)}
            sx={{
              p: 2,
              minHeight: 178,
              borderRadius: '2px',
              cursor: onCardClick && clickable ? 'pointer' : 'default',
              position: 'relative',
              overflow: 'hidden',
              background: 'linear-gradient(145deg, #ffffff, #f5f5f5)',
              border: activeTab === tab ? `2px solid ${color}` : '1px solid rgba(0,0,0,0.06)',
              borderTop: `3px solid ${color}`,
              boxShadow: activeTab === tab ? `0 0 0 2px ${color}18, 0 8px 24px ${color}20` : '0 4px 20px rgba(0,0,0,0.06)',
              opacity: clickable ? 1 : 0.55,
              transition: 'all 0.3s cubic-bezier(0.4,0,0.2,1)',
              '&:hover': clickable ? { transform: 'translateY(-4px)', boxShadow: `0 12px 30px ${color}25` } : {},
            }}
          >
            <Box sx={{ position: 'absolute', top: -8, right: -8, width: 40, height: 40, borderRadius: '50%', background: gradient, opacity: 0.1 }} />
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.25, mb: 1 }}>
              <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', width: 36, height: 36, borderRadius: '2px', background: `${color}16`, color }}>{icon}</Box>
              <Typography variant="body2" color="text.secondary" fontWeight={500}>{label}</Typography>
            </Box>
            <Typography variant="h4" fontWeight={800} sx={{ background: gradient, WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>{String(value)}</Typography>
            {subtitle && <Typography variant="caption" color="text.secondary" sx={{ mt: 0.25, display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{subtitle}</Typography>}
            <Box sx={{ position: 'absolute', left: 14, right: 14, bottom: 7 }}><StatsMiniChart type={chartType} data={chartData} color={color} palette={miniChartPalettes[variant]} height={40} /></Box>
          </Paper>
        );
      })}
    </Box>
  );
};

export default StatsCards;
