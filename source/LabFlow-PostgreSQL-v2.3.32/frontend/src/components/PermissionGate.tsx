import React, { type ReactNode } from 'react';
import { useUser } from '../UserContext';
import { hasPermission } from '../constants/permissions';

interface PermissionGateProps {
  /** 需要的单个权限点（满足即显示，支持 `*` 通配） */
  permission?: string;
  /** 多权限点：任一满足即显示 */
  permissions?: string[];
  /** 仅管理员可见 */
  adminOnly?: boolean;
  children: ReactNode;
  /** 无权限时渲染的内容（默认不渲染） */
  fallback?: ReactNode;
}

/**
 * 基于当前用户权限渲染子内容。
 * - adminOnly：仅管理员可见
 * - permission / permissions：命中任一权限点（或 is_admin 通配）即显示
 * - 两者皆未指定：直接显示
 */
const PermissionGate: React.FC<PermissionGateProps> = ({
  permission,
  permissions,
  adminOnly,
  children,
  fallback = null,
}) => {
  const { user } = useUser();

  if (adminOnly) {
    return user?.is_admin ? <>{children}</> : <>{fallback}</>;
  }

  if (permission || (permissions && permissions.length > 0)) {
    const perms = user?.permissions || [];
    const ok = !!user?.is_admin || (permission ? hasPermission(perms, permission) : false)
      || (permissions ? permissions.some((p) => hasPermission(perms, p)) : false);
    return ok ? <>{children}</> : <>{fallback}</>;
  }

  return <>{children}</>;
};

export default PermissionGate;
