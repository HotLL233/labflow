import type { ReactNode } from 'react';
import AssessmentIcon from '@mui/icons-material/Assessment';
import BiotechIcon from '@mui/icons-material/Biotech';
import BusinessIcon from '@mui/icons-material/Business';
import CalendarMonthIcon from '@mui/icons-material/CalendarMonth';
import FolderIcon from '@mui/icons-material/Folder';
import HistoryIcon from '@mui/icons-material/History';
import MemoryIcon from '@mui/icons-material/Memory';
import PeopleIcon from '@mui/icons-material/People';
import PrecisionManufacturingIcon from '@mui/icons-material/PrecisionManufacturing';
import ScienceIcon from '@mui/icons-material/Science';
import ViewWeekIcon from '@mui/icons-material/ViewWeek';
import WaterDropIcon from '@mui/icons-material/WaterDrop';

export type TabValue =
  | 'week'
  | 'month'
  | 'user-log'
  | 'division'
  | 'sheet1'
  | 'sheet2'
  | 'sheet3'
  | 'sheet4'
  | 'sheet5'
  | 'sheet6'
  | 'sheet7'
  | 'sheet8'
  | 'sheet9'
  | 'sheet10'
  | 'sheet11'
  | 'sheet13';

type StatCardDef = {
  key: TabValue;
  label: string;
  icon: ReactNode;
  color: string;
  desc: string;
};

export const STAT_CARDS: StatCardDef[] = [
  { key: 'week', label: '按周统计', icon: <ViewWeekIcon />, color: '#1976d2', desc: '每月第几周汇总' },
  { key: 'month', label: '按月统计', icon: <CalendarMonthIcon />, color: '#0891b2', desc: '每月汇总数据' },
  { key: 'user-log', label: '检测人记录', icon: <HistoryIcon />, color: '#4338ca', desc: '逐条记录明细' },
  { key: 'division', label: '检测部门-送样部门统计', icon: <BusinessIcon />, color: '#0369a1', desc: '按检测部门和送样部门汇总统计' },
  { key: 'sheet1', label: '实验室-项目-方法', icon: <BiotechIcon />, color: '#0e7490', desc: 'Sheet 1 各实验室项目方法对应表' },
  { key: 'sheet2', label: '仪器-汇总', icon: <PrecisionManufacturingIcon />, color: '#2563eb', desc: 'Sheet 2 仪器每日汇总' },
  { key: 'sheet3', label: '项目-汇总（含金额）', icon: <FolderIcon />, color: '#0f766e', desc: 'Sheet 3 项目金额汇总' },
  { key: 'sheet4', label: '实验室-汇总（含金额）', icon: <BusinessIcon />, color: '#4f46e5', desc: 'Sheet 4 实验室金额汇总' },
  { key: 'sheet5', label: '检测人-汇总（原始记录）', icon: <HistoryIcon />, color: '#0284c7', desc: 'Sheet 5 检测人原始记录' },
  { key: 'sheet6', label: '检测人汇总表（含系数）', icon: <PeopleIcon />, color: '#06b6d4', desc: 'Sheet 6 检测人系数汇总' },
  { key: 'sheet7', label: '实验室总表', icon: <ScienceIcon />, color: '#0d9488', desc: 'Sheet 7 实验室分类汇总' },
  { key: 'sheet8', label: '项目总表', icon: <AssessmentIcon />, color: '#1e40af', desc: 'Sheet 8 项目分类汇总' },
  { key: 'sheet9', label: '仪器类型汇总', icon: <MemoryIcon />, color: '#9E9E9E', desc: 'Sheet 9 仪器类型汇总' },
  { key: 'sheet10', label: '理化汇总', icon: <WaterDropIcon />, color: '#334155', desc: 'Sheet 10 理化方法汇总' },
  { key: 'sheet13', label: '人员工作量汇总', icon: <PeopleIcon />, color: '#3f51b5', desc: '按检测类型、系数和倍率逐行展示' },
];

export const STAT_CARD_PERMISSIONS: Record<TabValue, string> = {
  week: 'stats:workload:week',
  month: 'stats:workload:month',
  'user-log': 'stats:workload:user-log',
  division: 'stats:workload:division',
  sheet1: 'stats:workload:sheet1',
  sheet2: 'stats:workload:sheet2',
  sheet3: 'stats:workload:sheet3',
  sheet4: 'stats:workload:sheet4',
  sheet5: 'stats:workload:sheet5',
  sheet6: 'stats:workload:sheet6',
  sheet7: 'stats:workload:sheet7',
  sheet8: 'stats:workload:sheet8',
  sheet9: 'stats:workload:sheet9',
  sheet10: 'stats:workload:sheet10',
  sheet11: 'stats:workload:division',
  sheet13: 'stats:workload:sheet6',
};
