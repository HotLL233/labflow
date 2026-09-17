import React, { type ReactNode } from 'react';
import { Navigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { hasAnyPrefix, hasPermission } from '../constants/permissions';

interface ProtectedRouteProps {
  children: ReactNode;
  requireAdmin?: boolean;
  requireManage?: boolean;
  requirePermission?: string | string[];
}

const ProtectedRoute: React.FC<ProtectedRouteProps> = ({
  children,
  requireAdmin,
  requireManage,
  requirePermission,
}) => {
  const { isLoggedIn, user } = useUser();
  if (!isLoggedIn) return <Navigate to="/login" replace />;
  if (requireAdmin && !user?.is_admin) return <Navigate to="/" replace />;
  if (requireManage && !(user?.is_admin || hasAnyPrefix(user?.permissions || [], 'manage:'))) {
    return <Navigate to="/" replace />;
  }
  if (requirePermission) {
    const required = Array.isArray(requirePermission) ? requirePermission : [requirePermission];
    const allowed = user?.is_admin || required.some((perm) => hasPermission(user?.permissions || [], perm));
    if (!allowed) return <Navigate to="/" replace />;
  }
  return <>{children}</>;
};

export default ProtectedRoute;
