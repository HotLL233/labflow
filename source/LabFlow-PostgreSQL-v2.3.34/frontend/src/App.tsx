import React, { Component, type ErrorInfo, type ReactNode } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { Box } from '@mui/material';
import { UserProvider, useUser } from './UserContext';
import { UiDisplayProvider } from './UiDisplayContext';
import Layout from './components/Layout';
import HomePage from './pages/HomePage';
import SamplePortal from './pages/SamplePortal';
import WorkloadPortal from './pages/WorkloadPortal';
import EntryPage from './pages/EntryPage';
import SampleEntryPage from './pages/SampleEntryPage';
import SampleStatsPage from './pages/SampleStatsPage';
import RdRecordsPage from './pages/RdRecordsPage';
import StatsPage from './pages/StatsPage';
import ManagePage from './pages/ManagePage';
import AdminRolesPage from './pages/AdminRolesPage';
import HelpPage from './pages/HelpPage';
import SampleInfoHome from './pages/SampleInfoHome';
import SampleInfoEntry from './pages/SampleInfoEntry';
import SampleInfoStatsPage from './pages/SampleInfoStatsPage';
import LoginPage from './pages/LoginPage';
import ProtectedRoute from './components/ProtectedRoute';
import DataGovernancePage from './pages/DataGovernancePage';
import NotificationPage from './pages/NotificationPage';
import AnnouncementAdminPage from './pages/AnnouncementAdminPage';
import MyNotificationsPage from './pages/MyNotificationsPage';
import PersonnelChangePage from './pages/PersonnelChangePage';

interface ErrorBoundaryState { hasError: boolean; error: Error | null; }
class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, error: null };
  static getDerivedStateFromError(error: Error): ErrorBoundaryState { return { hasError: true, error }; }
  componentDidCatch(error: Error, info: ErrorInfo) { console.error('ErrorBoundary caught error:', error, info); }
  render() { if (this.state.hasError) return <div style={{ padding: '2rem', textAlign: 'center', marginTop: '20vh' }}><h1>页面出错了</h1><p style={{ color: '#666', margin: '1rem 0' }}>{this.state.error?.message}</p><button onClick={() => this.setState({ hasError: false, error: null })} style={{ padding: '0.5rem 1.5rem', fontSize: '1rem', cursor: 'pointer', borderRadius: '2px', border: '1px solid #1976d2', background: '#1976d2', color: '#fff' }}>重试</button></div>; return this.props.children; }
}

const NotFoundPage: React.FC = () => <div style={{ padding: '2rem', textAlign: 'center', marginTop: '10vh' }}><h1 style={{ fontSize: '4rem', color: '#ccc', margin: 0 }}>404</h1><p style={{ color: '#666', marginTop: '1rem' }}>页面未找到</p></div>;

const RequireAuth: React.FC<{ children: ReactNode }> = ({ children }) => {
  const { isLoggedIn } = useUser();
  if (!isLoggedIn) return <Navigate to="/login" replace />;
  return <>{children}</>;
};

const App: React.FC = () => (<ErrorBoundary><UserProvider><UiDisplayProvider><Routes><Route element={<Layout />}>
  <Route path="/login" element={<Box sx={{ position: 'fixed', inset: 0, zIndex: 9999 }}><LoginPage /></Box>} />
  <Route path="/" element={<RequireAuth><HomePage /></RequireAuth>} />

  <Route path="/sample" element={<ProtectedRoute requirePermission="entry:sample"><SamplePortal /></ProtectedRoute>} />
  <Route path="/sample/:groupId" element={<ProtectedRoute requirePermission="entry:sample"><SampleEntryPage /></ProtectedRoute>} />
  <Route path="/sample/stats" element={<ProtectedRoute requirePermission="stats:portal:rd"><SampleStatsPage /></ProtectedRoute>} />
  <Route path="/sample-records" element={<ProtectedRoute requirePermission={["entry:sample", "records:rd:view-all", "records:rd:view-lab"]}><RdRecordsPage /></ProtectedRoute>} />

  <Route path="/sample-info" element={<ProtectedRoute><SampleInfoHome /></ProtectedRoute>} />
  <Route path="/sample-info/entry" element={<ProtectedRoute requirePermission="entry:sample-info"><SampleInfoEntry /></ProtectedRoute>} />
  <Route path="/sample-info/stats" element={<ProtectedRoute requirePermission="stats:portal:sample-info"><SampleInfoStatsPage /></ProtectedRoute>} />

  <Route path="/workload" element={<ProtectedRoute requirePermission="entry:workload"><WorkloadPortal /></ProtectedRoute>} />
  <Route path="/entry/:groupId" element={<ProtectedRoute requirePermission="entry:workload"><EntryPage /></ProtectedRoute>} />
  <Route path="/stats" element={<ProtectedRoute requirePermission="stats:portal:workload"><StatsPage /></ProtectedRoute>} />
  <Route path="/help" element={<ProtectedRoute requirePermission="help:view"><HelpPage /></ProtectedRoute>} />
  <Route path="/personnel-change" element={<ProtectedRoute><PersonnelChangePage /></ProtectedRoute>} />
  <Route path="/notifications" element={<ProtectedRoute><MyNotificationsPage /></ProtectedRoute>} />

  <Route path="/manage" element={<ProtectedRoute requireManage><ManagePage /></ProtectedRoute>} />
  <Route path="/manage/data-governance" element={<ProtectedRoute requirePermission="manage:data-governance:view"><DataGovernancePage /></ProtectedRoute>} />
  <Route path="/manage/notifications" element={<ProtectedRoute requirePermission="manage:notifications"><NotificationPage /></ProtectedRoute>} />
  <Route path="/manage/announcements" element={<ProtectedRoute requirePermission="manage:settings"><AnnouncementAdminPage /></ProtectedRoute>} />
  <Route path="/manage/:section" element={<ProtectedRoute requireManage><ManagePage /></ProtectedRoute>} />
  <Route path="/manage/roles" element={<ProtectedRoute requirePermission="manage:roles"><AdminRolesPage /></ProtectedRoute>} />
  <Route path="/admin/roles" element={<ProtectedRoute requirePermission="manage:roles"><AdminRolesPage /></ProtectedRoute>} />

  <Route path="/404" element={<NotFoundPage />} />
  <Route path="*" element={<Navigate to="/404" replace />} />
</Route></Routes></UiDisplayProvider></UserProvider></ErrorBoundary>);
export default App;
