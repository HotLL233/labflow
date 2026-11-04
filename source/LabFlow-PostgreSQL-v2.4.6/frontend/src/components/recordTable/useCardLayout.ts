import { useMemo } from 'react';
import { useMediaQuery } from '@mui/material';
import {
  VERY_NARROW_QUERY, detectMobileDevice, resolveCardLayout,
  type RecordTableStyleSpec,
} from '../../utils/recordTableStyle';

/**
 * v2.3.34 记录页布局判定。
 *
 * `auto` 模式下以**设备类型**为主，而不是视口宽度：
 * - 移动设备 → 卡片布局；
 * - 桌面设备 → 表格布局。手机浏览器开启「桌面网站」后上报的是桌面 UA，
 *   因此会按桌面处理，看到与 PC 一致的表格。
 *
 * 只有视口极窄（< 768px）时才兜底回落到卡片，避免桌面端把窗口拖得很窄时
 * 把十几列硬塞进不可读的宽度。
 */
export function useCardLayout(spec: RecordTableStyleSpec): boolean {
  const veryNarrow = useMediaQuery(VERY_NARROW_QUERY);
  const mobileDevice = useMemo(detectMobileDevice, []);
  return resolveCardLayout(spec.layout.mode, mobileDevice, veryNarrow);
}

export default useCardLayout;
