import React, { useCallback, useEffect, useState } from 'react';
import { Box, Divider, ListItemIcon, ListItemText, Menu, MenuItem, Typography } from '@mui/material';

/**
 * v2.3.20 记录表列宽交互。
 *
 * 列宽来源按优先级排列：
 *   ④ 本机覆盖（localStorage，用户拖拽产生）
 *   ③ 管理员配置（数据表 width + width_mode）
 *   ② 内容测量
 *   ① 系统默认上下限
 * 本文件只负责 ④ 与本机交互，②③ 由 utils/recordTableLayout.ts 计算。
 */

/**
 * 容器宽度测量：列宽引擎与操作列档位都依赖它。
 *
 * v2.3.20 修复：必须使用 callback ref。页面在加载态会提前 return（表格尚未渲染），
 * 若在 `useEffect(..., [])` 中读取 `ref.current`，首次必然为 null 且之后不会再绑定，
 * 导致宽度恒为 0 —— 引擎会退回「按自然宽比例」分配，把短列压到 20~30px、表头逐字竖排。
 */
export function useContainerWidth<T extends HTMLElement>() {
  const [element, setElement] = useState<T | null>(null);
  const [width, setWidth] = useState(0);

  const ref = useCallback((node: T | null) => {
    setElement(node);
  }, []);

  useEffect(() => {
    if (!element) return undefined;
    const update = () => setWidth(element.clientWidth || 0);
    update();
    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', update);
      return () => window.removeEventListener('resize', update);
    }
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [element]);

  return { ref, width };
}

/** 本机列宽覆盖。只存本机，换设备或清缓存即失效。 */
export function useColumnWidthOverrides(storageKey: string) {
  const key = `labflow.record-col-widths:${storageKey}`;
  const [overrides, setOverrides] = useState<Record<string, number>>(() => {
    if (!storageKey) return {};
    try {
      const raw = JSON.parse(localStorage.getItem(key) || '{}');
      if (!raw || typeof raw !== 'object') return {};
      return Object.fromEntries(
        Object.entries(raw as Record<string, unknown>)
          .filter(([, value]) => typeof value === 'number' && Number(value) > 0)
          .map(([name, value]) => [name, Number(value)]),
      );
    } catch {
      return {};
    }
  });

  const persist = useCallback(
    (next: Record<string, number>) => {
      setOverrides(next);
      try {
        if (Object.keys(next).length === 0) localStorage.removeItem(key);
        else localStorage.setItem(key, JSON.stringify(next));
      } catch {
        // 本机存储不可用时只影响记忆，不影响本次会话。
      }
    },
    [key],
  );

  const overrideCount = Object.keys(overrides).length;

  return {
    overrides,
    overrideCount,
    setWidth: (columnKey: string, width: number) =>
      persist({ ...overrides, [columnKey]: Math.max(32, Math.round(width)) }),
    resetColumn: (columnKey: string) => {
      const next = { ...overrides };
      delete next[columnKey];
      persist(next);
    },
    resetAll: () => persist({}),
  };
}

/**
 * 列宽拖拽手柄：放在表头单元格右边缘。
 * 双击恢复该列默认宽度（清掉本机覆盖）。
 */
export const ColumnResizeHandle: React.FC<{
  currentWidth: number;
  onResize: (width: number) => void;
  onReset: () => void;
}> = ({ currentWidth, onResize, onReset }) => {
  const [dragging, setDragging] = useState(false);

  const handlePointerDown = (event: React.PointerEvent<HTMLSpanElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startWidth = currentWidth;
    setDragging(true);

    const handleMove = (moveEvent: PointerEvent) => {
      const delta = moveEvent.clientX - startX;
      onResize(Math.max(32, startWidth + delta));
    };
    const handleUp = () => {
      setDragging(false);
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('pointerup', handleUp);
    };
    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleUp);
  };

  return (
    <Box
      component="span"
      role="separator"
      aria-label="拖动调整列宽，双击恢复默认"
      onPointerDown={handlePointerDown}
      onDoubleClick={event => {
        event.stopPropagation();
        onReset();
      }}
      sx={{
        position: 'absolute',
        top: 0,
        right: -4,
        bottom: 0,
        width: 8,
        cursor: 'col-resize',
        userSelect: 'none',
        zIndex: 4,
        bgcolor: dragging ? 'primary.main' : 'transparent',
        '&:hover': { bgcolor: 'primary.light' },
      }}
    />
  );
};

export interface RowActionItem {
  key: string;
  label: string;
  icon?: React.ReactNode;
  onClick: () => void;
  /** 不满足条件时置灰并给出原因，避免按钮直接消失让人困惑 */
  disabledReason?: string;
}

/**
 * 操作列：主操作 + 「更多 ▾」收纳。
 * 权限与状态判断完全由调用方决定，本组件只负责展示与收纳。
 */
export const RowActionsMenu: React.FC<{
  items: RowActionItem[];
  /** 始终显示的主操作（如「取样」「完成检测」「已录入」） */
  primary?: React.ReactNode;
  /** 是否收纳：true 时其余动作全部进入菜单 */
  collapsed?: boolean;
  /** 未收纳时最多内联显示的动作数量 */
  maxInline?: number;
}> = ({ items, primary, collapsed = true, maxInline = 2 }) => {
  const [anchor, setAnchor] = useState<null | HTMLElement>(null);
  const available = items.filter(item => !item.disabledReason);
  const inline = collapsed ? [] : available.slice(0, maxInline);
  const inlineKeys = new Set(inline.map(item => item.key));
  const menuItems = items.filter(item => !inlineKeys.has(item.key));

  if (!primary && items.length === 0) return <Typography variant="caption" color="text.disabled">-</Typography>;

  return (
    <Box sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.5, flexWrap: 'wrap', justifyContent: 'center' }}>
      {primary}
      {inline.map(item => (
        <Box
          key={item.key}
          component="span"
          onClick={event => {
            event.stopPropagation();
            item.onClick();
          }}
          sx={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 0.4,
            px: 0.75,
            py: 0.25,
            fontSize: '0.72rem',
            border: '1px solid #b8c2cc',
            borderRadius: '2px',
            cursor: 'pointer',
            bgcolor: '#fff',
            whiteSpace: 'nowrap',
            '&:hover': { bgcolor: 'action.hover' },
          }}
        >
          {item.icon}
          {item.label}
        </Box>
      ))}
      {menuItems.length > 0 && (
        <>
          <Box
            component="span"
            onClick={event => {
              event.stopPropagation();
              setAnchor(event.currentTarget as HTMLElement);
            }}
            sx={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 0.4,
              px: 0.75,
              py: 0.25,
              fontSize: '0.72rem',
              border: '1px solid #b8c2cc',
              borderRadius: '2px',
              cursor: 'pointer',
              bgcolor: '#fff',
              whiteSpace: 'nowrap',
              '&:hover': { bgcolor: 'action.hover' },
            }}
          >
            更多 ▾
          </Box>
          <Menu anchorEl={anchor} open={Boolean(anchor)} onClose={() => setAnchor(null)} onClick={event => event.stopPropagation()}>
            {menuItems.map((item, index) => (
              <MenuItem
                key={item.key}
                dense
                disabled={Boolean(item.disabledReason)}
                onClick={() => {
                  setAnchor(null);
                  item.onClick();
                }}
              >
                {item.icon && <ListItemIcon sx={{ minWidth: 26 }}>{item.icon}</ListItemIcon>}
                <ListItemText primary={item.label} secondary={item.disabledReason || undefined} />
                {index === 0 && menuItems.length > 3 && <Divider />}
              </MenuItem>
            ))}
          </Menu>
        </>
      )}
    </Box>
  );
};
