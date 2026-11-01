import React from 'react';
import { Box, Chip } from '@mui/material';
import type { Division } from '../types';

const R = '2px';

interface DivisionChipsProps {
  divisions: Division[];
  // 各事业部在当前搜索条件下的实验室数量（键为 division id）
  counts: Record<number, number>;
  // 「全部」Chip 显示的数量 = 当前搜索条件下实验室总数
  totalCount: number;
  // 当前选中的事业部 id；0 表示全部
  selected: number;
  onSelect: (id: number) => void;
  // 主题色（工作量录入用 #1976d2，研发送样用 #e65100）
  themeColor: string;
}

/**
 * 事业部筛选 Chip 栏（v0.4.24）
 * 复用 EntryPage 方法类型筛选的 Chip 样式（filled/outlined + borderRadius + fontWeight 高亮）。
 * 「全部 (n)」的 n 跟随搜索框过滤变化（= 搜索条件下实验室总数）；
 * 各事业部 Chip 显示该事业部在搜索条件下的实验室数量。
 */
const DivisionChips: React.FC<DivisionChipsProps> = ({ divisions, counts, totalCount, selected, onSelect, themeColor }) => {
  if (!divisions.length) return null;
  return (
    <Box sx={{ display: 'flex', gap: 1, mb: 3, flexWrap: 'wrap', alignItems: 'center' }}>
      <Chip
        label={`全部 (${totalCount})`} size="medium"
        color={selected === 0 ? 'primary' : 'default'}
        variant={selected === 0 ? 'filled' : 'outlined'}
        onClick={() => onSelect(0)}
        sx={{ borderRadius: R, cursor: 'pointer', fontWeight: selected === 0 ? 700 : 400 }}
      />
      {divisions.map(d => {
        const cnt = counts[d.id] ?? 0;
        const active = selected === d.id;
        return (
          <Chip
            key={d.id}
            label={`${d.name} (${cnt})`} size="medium"
            color={active ? 'primary' : 'default'}
            variant={active ? 'filled' : 'outlined'}
            onClick={() => onSelect(d.id)}
            sx={{ borderRadius: R, cursor: 'pointer', fontWeight: active ? 700 : 400 }}
          />
        );
      })}
    </Box>
  );
};

export default DivisionChips;
