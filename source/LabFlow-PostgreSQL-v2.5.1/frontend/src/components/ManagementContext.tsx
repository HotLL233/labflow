import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { Alert, Button, Dialog, DialogActions, DialogContent, DialogTitle, Typography } from '@mui/material';
import { useLocation, useNavigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { hasAnyPrefix } from '../constants/permissions';
import { getSetting } from '../api/client';
import type { ManageNavKey } from './ManageNav';

type PendingConfig = { dirty: boolean; label: string; save?: () => Promise<boolean>; discard?: () => void };
interface ManagementState {
  enabledKeys?: ReadonlySet<ManageNavKey>;
  navigationError: string;
  reloadNavigation: () => void;
  guard: (action: () => void) => void;
  register: (key: string, read: () => PendingConfig) => () => void;
  readDraft: <T,>(key: string) => T | undefined;
  writeDraft: (key: string, value: unknown) => void;
  clearDraft: (key: string) => void;
}
const Context = createContext<ManagementState | null>(null);
const requiredKeys: ManageNavKey[] = ['master-import', 'auxiliary', 'instruments', 'sessions', 'stats', 'data-governance', 'log-maintenance', 'notifications', 'announcements', 'ai'];

export function ManagementProvider({ children }: { children: React.ReactNode }) {
  const { user, isLoggedIn } = useUser();
  const location = useLocation();
  const owner = JSON.stringify([user?.id, user?.is_admin, [...(user?.permissions || [])].sort(),
    user?.primary_division_id, user?.division_id, user?.group_id,
    [...(user?.group_ids || [])].sort(), [...(user?.division_ids || [])].sort(),
    [...(user?.business_division_ids || [])].sort(), [...(user?.role_ids || [])].sort(),
    [...(user?.role_identity_keys || [])].sort(), user?.is_public_account, user?.is_rd_public_account, user?.is_analysis_public_account]);
  const canManage = isLoggedIn && Boolean(user?.is_admin || hasAnyPrefix(user?.permissions || [], 'manage:'));
  const managementRoute = /^\/(manage|admin)(\/|$)/.test(location.pathname);
  const [navigation, setNavigation] = useState<{ owner: string; keys?: ReadonlySet<ManageNavKey>; error: string }>({ owner: '', error: '' });
  const [revision, setRevision] = useState(0);
  const [pending, setPending] = useState<{ action: () => void; labels: string[] } | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState('');
  const configs = useRef(new Map<string, () => PendingConfig>());
  const drafts = useRef(new Map<string, unknown>());
  const currentOwner = useRef(owner);
  if (currentOwner.current !== owner) {
    currentOwner.current = owner;
    configs.current.clear();
    drafts.current.clear();
  }
  useEffect(() => { setPending(null); setSaveError(''); setSaving(false); }, [owner]);
  useEffect(() => {
    if (!canManage || !managementRoute || (navigation.owner === owner && revision === 0)) return;
    let active = true;
    void getSetting('manage-tabs').then(response => {
      if (response.code !== 0) throw new Error(response.message || '导航配置读取失败');
      let keys: Set<ManageNavKey> | undefined;
      if (response.data?.value) {
        const tabs = JSON.parse(response.data.value) as { key: ManageNavKey; enabled?: boolean }[];
        if (!Array.isArray(tabs)) throw new Error('导航配置格式不正确');
        keys = new Set([...tabs.filter(item => item.enabled !== false).map(item => item.key), ...requiredKeys]);
      }
      if (active) { setNavigation({ owner, keys, error: '' }); setRevision(0); }
    }).catch(error => {
      if (active) { setNavigation({ owner, error: error?.message || '导航配置读取失败' }); setRevision(0); }
    });
    return () => { active = false; };
  }, [canManage, managementRoute, owner, navigation.owner, revision]);
  const readPending = useCallback(() => [...configs.current.entries()].map(([key, read]) => ({ key, ...read() })).filter(item => item.dirty), []);
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (readPending().length) { event.preventDefault(); event.returnValue = ''; }
    };
    window.addEventListener('beforeunload', beforeUnload);
    return () => window.removeEventListener('beforeunload', beforeUnload);
  }, [readPending]);
  const register = useCallback((key: string, read: () => PendingConfig) => {
    configs.current.set(key, read);
    return () => { if (configs.current.get(key) === read) configs.current.delete(key); };
  }, []);
  const guard = useCallback((action: () => void) => {
    const dirty = readPending();
    if (!dirty.length) { action(); return; }
    setSaveError('');
    setPending({ action, labels: dirty.map(item => item.label) });
  }, [readPending]);
  const readDraft = useCallback(<T,>(key: string) => currentOwner.current === owner ? drafts.current.get(key) as T | undefined : undefined, [owner]);
  const writeDraft = useCallback((key: string, value: unknown) => { if (currentOwner.current === owner) drafts.current.set(key, value); }, [owner]);
  const clearDraft = useCallback((key: string) => { if (currentOwner.current === owner) drafts.current.delete(key); }, [owner]);
  const reloadNavigation = useCallback(() => setRevision(value => value + 1), []);
  const saveAndLeave = async () => {
    if (!pending || saving) return;
    const requestOwner = currentOwner.current;
    setSaving(true); setSaveError('');
    try {
      for (const config of readPending()) {
        if (!config.save) return;
        const saved = await config.save();
        if (currentOwner.current !== requestOwner) return;
        if (!saved) { setSaveError('配置尚未保存成功，请继续编辑并查看错误提示。'); return; }
        drafts.current.delete(config.key);
      }
      const action = pending.action; setPending(null); action();
    } catch (error: any) { if (currentOwner.current === requestOwner) setSaveError(error?.message || '保存失败，已留在当前页面。'); }
    finally { if (currentOwner.current === requestOwner) setSaving(false); }
  };
  const discardAndLeave = () => {
    if (!pending || saving) return;
    for (const config of readPending()) { drafts.current.delete(config.key); config.discard?.(); }
    const action = pending.action; setPending(null); action();
  };
  const value = useMemo<ManagementState>(() => ({
    enabledKeys: navigation.owner === owner ? navigation.keys : undefined,
    navigationError: navigation.owner === owner ? navigation.error : '',
    reloadNavigation, guard, register, readDraft, writeDraft, clearDraft,
  }), [navigation, owner, reloadNavigation, guard, register, readDraft, writeDraft, clearDraft]);
  return <Context.Provider value={value}>
    <React.Fragment key={owner}>{children}</React.Fragment>
    <Dialog open={Boolean(pending)} onClose={saving ? undefined : () => setPending(null)} fullWidth maxWidth="sm" aria-labelledby="management-unsaved-title">
      <DialogTitle id="management-unsaved-title">有未保存的配置</DialogTitle>
      <DialogContent>
        <Typography>{pending?.labels.join('、')}</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>保存成功后再离开，或明确放弃当前修改。此处不会执行历史修正、导入或恢复。</Typography>
        {saveError && <Alert severity="error" sx={{ mt: 1 }}>{saveError}</Alert>}
      </DialogContent>
      <DialogActions sx={{ flexWrap: 'wrap', gap: 1 }}>
        <Button disabled={saving} onClick={() => setPending(null)}>继续编辑</Button>
        <Button color="warning" disabled={saving} onClick={discardAndLeave}>放弃修改并离开</Button>
        <Button variant="contained" disabled={saving || readPending().some(item => !item.save)} onClick={() => void saveAndLeave()}>{saving ? '保存中…' : '保存并离开'}</Button>
      </DialogActions>
    </Dialog>
  </Context.Provider>;
}

export const useManagement = () => useContext(Context);

export function useManagedNavigate() {
  const navigate = useNavigate();
  const management = useManagement();
  return useCallback((path: string) => {
    const go = () => navigate(path);
    if (management) management.guard(go); else go();
  }, [navigate, management?.guard]);
}

export function useUnsavedManagement(key: string, config: PendingConfig) {
  const management = useManagement();
  const latest = useRef(config);
  latest.current = config;
  useEffect(() => management?.register(key, () => latest.current), [management?.register, key]);
}
