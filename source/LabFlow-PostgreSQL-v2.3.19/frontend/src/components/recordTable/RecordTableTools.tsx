import React, { useMemo, useState } from 'react';
import {
  Box, Button, Checkbox, Chip, FormControlLabel, IconButton, ListItemText, Menu, MenuItem,
  Stack, TextField, Tooltip, Typography,
} from '@mui/material';
import ViewColumnIcon from '@mui/icons-material/ViewColumn';
import FilterListIcon from '@mui/icons-material/FilterList';
import CloseIcon from '@mui/icons-material/Close';

/**
 * v2.3.19 记录表增强工具。
 * 依据 docs/表格.md 补齐记录表的通用交互：固定表头、自定义列（记住选择）、
 * 表头筛选（多选 + 搜索）、已选条件与结果数量。
 *
 * 说明：列表接口按页返回数据，列筛选的作用范围明确限定为「当前页已加载的记录」，
 * 界面上会标注范围，避免让人误以为筛选作用于全量数据。
 */

const R = '2px';

export const stickyHeaderSx = {
  '& .MuiTableCell-stickyHeader': {
    bgcolor: '#f5f7f5',
    zIndex: 3,
    borderColor: '#e0e0e0',
  },
} as const;

export type ColumnFilters = Record<string, string[]>;

const STORAGE_PREFIX = 'labflow.record-columns:';

/** 自定义列：勾选决定显示哪些字段，并按用户 + 表名缓存在本机。 */
export function useColumnVisibility(storageKey: string, allKeys: string[]) {
  const key = `${STORAGE_PREFIX}${storageKey}`;
  const [hidden, setHidden] = useState<string[]>(() => {
    if (!storageKey) return [];
    try {
      const saved = JSON.parse(localStorage.getItem(key) || '[]');
      return Array.isArray(saved) ? saved.filter((item): item is string => typeof item === 'string') : [];
    } catch {
      return [];
    }
  });

  const persist = (next: string[]) => {
    setHidden(next);
    if (!storageKey) return;
    try {
      localStorage.setItem(key, JSON.stringify(next));
    } catch {
      // 本机存储不可用时只影响记忆，不影响本次会话的显示。
    }
  };

  const visibleCount = allKeys.filter(item => !hidden.includes(item)).length;
  return {
    hidden,
    visibleCount,
    toggle: (fieldKey: string) => {
      const isHidden = hidden.includes(fieldKey);
      // 至少保留一列，否则表格会变成空白让人误以为加载失败。
      if (!isHidden && visibleCount <= 1) return;
      persist(isHidden ? hidden.filter(item => item !== fieldKey) : [...hidden, fieldKey]);
    },
    reset: () => persist([]),
  };
}

export const ColumnSettingsButton: React.FC<{
  columns: { key: string; label: string }[];
  hidden: string[];
  onToggle: (key: string) => void;
  onReset: () => void;
}> = ({ columns, hidden, onToggle, onReset }) => {
  const [anchor, setAnchor] = useState<null | HTMLElement>(null);
  const visibleCount = columns.filter(column => !hidden.includes(column.key)).length;
  return (
    <>
      <Button
        size="small"
        variant="outlined"
        startIcon={<ViewColumnIcon />}
        onClick={event => setAnchor(event.currentTarget)}
        sx={{ borderRadius: R }}
      >
        列设置（{visibleCount}/{columns.length}）
      </Button>
      <Menu anchorEl={anchor} open={!!anchor} onClose={() => setAnchor(null)}>
        <Box sx={{ px: 1.5, py: 0.5, minWidth: 200 }}>
          <Stack direction="row" justifyContent="space-between" alignItems="center">
            <Typography variant="caption" color="text.secondary">勾选要显示的列</Typography>
            <Button size="small" onClick={onReset}>全部显示</Button>
          </Stack>
        </Box>
        {columns.map(column => (
          <MenuItem key={column.key} dense onClick={() => onToggle(column.key)}>
            <Checkbox size="small" checked={!hidden.includes(column.key)} sx={{ p: 0.4, mr: 0.5 }} />
            <ListItemText primary={column.label} />
          </MenuItem>
        ))}
      </Menu>
    </>
  );
};

export const ColumnFilterButton: React.FC<{
  label: string;
  options: string[];
  selected: string[];
  onChange: (next: string[]) => void;
  scopeHint?: string;
}> = ({ label, options, selected, onChange, scopeHint }) => {
  const [anchor, setAnchor] = useState<null | HTMLElement>(null);
  const [keyword, setKeyword] = useState('');
  const shown = useMemo(() => {
    const text = keyword.trim().toLowerCase();
    return text ? options.filter(option => option.toLowerCase().includes(text)) : options;
  }, [options, keyword]);

  const close = () => { setAnchor(null); setKeyword(''); };

  return (
    <>
      <Tooltip title={`按「${label}」筛选`}>
        <IconButton
          size="small"
          onClick={event => { event.stopPropagation(); setAnchor(event.currentTarget); }}
          sx={{ p: 0.2, ml: 0.25, color: selected.length ? 'primary.main' : 'text.disabled' }}
          aria-label={`筛选 ${label}`}
        >
          <FilterListIcon sx={{ fontSize: 15 }} />
        </IconButton>
      </Tooltip>
      <Menu anchorEl={anchor} open={!!anchor} onClose={close} onClick={event => event.stopPropagation()}>
        <Box sx={{ px: 1, pt: 0.5, pb: 0.25, minWidth: 210 }}>
          <TextField
            size="small"
            fullWidth
            autoFocus
            placeholder="搜索选项"
            value={keyword}
            onChange={event => setKeyword(event.target.value)}
          />
          {scopeHint && <Typography variant="caption" color="text.secondary">{scopeHint}</Typography>}
        </Box>
        <Box sx={{ maxHeight: 260, overflow: 'auto' }}>
          {shown.map(option => (
            <MenuItem key={option} dense onClick={() => onChange(
              selected.includes(option) ? selected.filter(item => item !== option) : [...selected, option],
            )}>
              <Checkbox size="small" checked={selected.includes(option)} sx={{ p: 0.4, mr: 0.5 }} />
              <ListItemText primary={option} />
            </MenuItem>
          ))}
          {shown.length === 0 && <Box sx={{ px: 1.5, py: 1, color: 'text.secondary', fontSize: '0.8rem' }}>无可选值</Box>}
        </Box>
        <Box sx={{ px: 1, py: 0.5, display: 'flex', justifyContent: 'space-between' }}>
          <Button size="small" onClick={() => { onChange([]); close(); }}>清除本列</Button>
          <Button size="small" onClick={close}>完成</Button>
        </Box>
      </Menu>
    </>
  );
};

export interface ActiveFilterItem {
  /** 用于移除该条件 */
  key: string;
  label: string;
}

export const ActiveFilterChips: React.FC<{
  items: ActiveFilterItem[];
  onRemove: (key: string) => void;
  onClearAll: () => void;
  matched: number;
  total: number;
  extraActions?: React.ReactNode;
}> = ({ items, onRemove, onClearAll, matched, total, extraActions }) => (
  <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 0.75, mb: 1 }}>
    <Typography variant="caption" color="text.secondary">
      {items.length > 0 ? `已选条件（${items.length}）` : '当前无筛选条件'} · 匹配 {matched} / 本页 {total} 条
    </Typography>
    {items.map(item => (
      <Chip
        key={item.key}
        label={item.label}
        size="small"
        onDelete={() => onRemove(item.key)}
        deleteIcon={<CloseIcon />}
        sx={{ borderRadius: R, bgcolor: '#eef3fb' }}
      />
    ))}
    {items.length > 0 && (
      <Button size="small" onClick={onClearAll} sx={{ borderRadius: R, minWidth: 0 }}>一键清空</Button>
    )}
    {extraActions}
  </Box>
);

/** 列筛选匹配：值为空的列不参与过滤，多选之间是「或」关系。 */
export function matchesColumnFilters(rowValues: Record<string, string>, filters: ColumnFilters): boolean {
  return Object.entries(filters).every(([fieldKey, selected]) => {
    if (!selected || selected.length === 0) return true;
    return selected.includes(rowValues[fieldKey] ?? '');
  });
}

/** 从当前页数据里收集某列的候选值，用于表头筛选菜单。 */
export function collectColumnOptions(rows: Record<string, string>[], fieldKey: string): string[] {
  const values = new Set<string>();
  rows.forEach(row => {
    const value = row[fieldKey];
    if (value && value !== '-') values.add(value);
  });
  return Array.from(values).sort((a, b) => a.localeCompare(b, 'zh-Hans-CN'));
}

export const InlineEmptyState: React.FC<{ text: string; hint?: string }> = ({ text, hint }) => (
  <Stack alignItems="center" spacing={0.5} sx={{ py: 4 }}>
    <Typography color="text.secondary">{text}</Typography>
    {hint && <Typography variant="caption" color="text.disabled">{hint}</Typography>}
  </Stack>
);

export const BulkActionsBar: React.FC<{
  count: number;
  onClear: () => void;
  children?: React.ReactNode;
}> = ({ count, onClear, children }) => {
  if (count === 0) return null;
  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 1, px: 1.25, py: 0.75, mb: 1, borderRadius: R, border: '1px solid #c7d8f0', bgcolor: '#f2f7ff' }}>
      <Typography variant="body2" sx={{ fontWeight: 600 }}>已选中 {count} 条</Typography>
      {children}
      <Button size="small" onClick={onClear} sx={{ borderRadius: R, ml: 'auto' }}>取消选择</Button>
    </Box>
  );
};

export const SelectAllCheckbox: React.FC<{
  checked: boolean;
  indeterminate: boolean;
  onChange: (checked: boolean) => void;
}> = ({ checked, indeterminate, onChange }) => (
  <FormControlLabel
    sx={{ m: 0 }}
    control={<Checkbox size="small" checked={checked} indeterminate={indeterminate} onChange={event => onChange(event.target.checked)} />}
    label=""
  />
);
