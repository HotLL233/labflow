import React, { createContext, useContext, useState, useCallback, useEffect, useRef, type ReactNode } from 'react';
import type { User } from './types';
import { hasPermission as checkPermission } from './constants/permissions';

interface UserContextType {
  user: User | null;
  token: string | null;
  isLoggedIn: boolean;
  userName: string;
  setUserName: (name: string) => void;
  login: (user: User, token: string, keepSignedIn?: boolean) => void;
  logout: () => void;
  /** 便捷权限判断：管理员恒真，否则按 permissions 命中 key */
  hasPermission: (key: string) => boolean;
}

const UserContext = createContext<UserContextType>({
  user: null, token: null, isLoggedIn: false,
  userName: '', setUserName: () => {},
  login: () => {}, logout: () => {},
  hasPermission: () => false,
});

const TK = 'workload_token';
const UK = 'workload_user';
const SK = 'workload_keep_signed_in';
const LEGACY_SK = 'workload_remember';

const hasPersistentSession = () => {
  const current = localStorage.getItem(SK) === 'true';
  const legacy = localStorage.getItem(LEGACY_SK) === 'true';
  if (!current && legacy && localStorage.getItem(TK)) {
    localStorage.setItem(SK, 'true');
  }
  return current || legacy;
};

const loadStored = (): { token: string | null; user: User | null } => {
  try {
    const storage = hasPersistentSession() ? localStorage : sessionStorage;
    const token = storage.getItem(TK);
    const userRaw = storage.getItem(UK);
    const user = userRaw ? JSON.parse(userRaw) : null;
    return { token, user };
  } catch {
    return { token: null, user: null };
  }
};

export const UserProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const stored = loadStored();
  const [token, setToken] = useState<string | null>(stored.token);
  const [user, setUser] = useState<User | null>(stored.user);
  const [userName, setUserNameState] = useState(stored.user?.username || '');
  const lastActivityRef = useRef(Date.now());
  const lastHeartbeatRef = useRef(0);

  const isLoggedIn = !!token && !!user;

  useEffect(() => {
    if (!token) return;
    const updateActivity = () => {
      lastActivityRef.current = Date.now();
      // Keep an actively used long form alive without converting an idle
      // browser tab into a permanent login session.
      if (Date.now() - lastHeartbeatRef.current > 5 * 60_000) {
        void refreshSession();
      }
    };
    const refreshSession = async () => {
      if (!token) return;
      lastHeartbeatRef.current = Date.now();
      try {
        const response = await fetch('/api/users/me', {
          headers: { Authorization: `Bearer ${token}` },
        });
        if (!response.ok) return;
        const payload = await response.json();
        if (payload?.code !== 0 || !payload?.data) return;
        const nextUser = payload.data as User;
        const storage = hasPersistentSession() ? localStorage : sessionStorage;
        storage.setItem(UK, JSON.stringify(nextUser));
        setUser(nextUser);
        setUserNameState(nextUser.username || '');
      } catch {
        // The next business request keeps the existing centralized login-error
        // handling. A transient heartbeat failure must not log the user out.
      }
    };
    const interval = window.setInterval(() => {
      if (Date.now() - lastActivityRef.current <= 6 * 60_000) {
        void refreshSession();
      }
    }, 60_000);
    window.addEventListener('pointerdown', updateActivity);
    window.addEventListener('keydown', updateActivity);
    window.addEventListener('input', updateActivity);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener('pointerdown', updateActivity);
      window.removeEventListener('keydown', updateActivity);
      window.removeEventListener('input', updateActivity);
    };
  }, [token]);

  const login = useCallback((u: User, t: string, keepSignedIn?: boolean) => {
    // Never leave a stale token in the other storage area when the option is changed.
    localStorage.removeItem(TK);
    localStorage.removeItem(UK);
    sessionStorage.removeItem(TK);
    sessionStorage.removeItem(UK);
    const storage = keepSignedIn ? localStorage : sessionStorage;
    storage.setItem(TK, t);
    storage.setItem(UK, JSON.stringify(u));
    if (keepSignedIn) {
      localStorage.setItem(SK, 'true');
    } else {
      localStorage.removeItem(SK);
      localStorage.removeItem(LEGACY_SK);
    }
    setToken(t);
    setUser(u);
    setUserNameState(u.username || '');
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem(TK);
    localStorage.removeItem(UK);
    localStorage.removeItem(SK);
    localStorage.removeItem(LEGACY_SK);
    sessionStorage.removeItem(TK);
    sessionStorage.removeItem(UK);
    setToken(null);
    setUser(null);
    setUserNameState('');
  }, []);

  const setUserName = useCallback((name: string) => {
    sessionStorage.setItem('workload_user_name', name);
    setUserNameState(name);
  }, []);

  const hasPermission = useCallback((key: string) => {
    const u = user;
    if (!u) return false;
    if (u.is_admin) return true;
    return checkPermission(u.permissions || [], key);
  }, [user]);

  return (
    <UserContext.Provider value={{ user, token, isLoggedIn, userName, setUserName, login, logout, hasPermission }}>
      {children}
    </UserContext.Provider>
  );
};

export const useUser = () => useContext(UserContext);
