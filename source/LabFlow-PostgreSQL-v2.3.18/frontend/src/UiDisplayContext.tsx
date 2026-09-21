import React, { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import { getSetting, updateSetting } from './api/client';
import { useUser } from './UserContext';

/** 全局界面显示设置（system_settings 表，无需鉴权即可读取）。 */
export const UI_DISPLAY_SETTING_KEY = 'ui-display';
/** 本地缓存键：沿用 labflow.* 命名，保证接口异常时仍有稳定的显示口径。 */
const STORAGE_KEY = 'labflow.ui.show-business-no';

export interface UiDisplaySettings {
  /** 业务编号是否在业务列表中显示。 */
  show_business_no: boolean;
}

interface UiDisplayContextValue {
  showBusinessNo: boolean;
  /** 立即在本机生效（页面无需刷新）；persist=true 时同时写入全局设置。 */
  setShowBusinessNo: (value: boolean, persist?: boolean) => Promise<void>;
  /** 重新从服务端读取全局设置（保存后或登录后调用）。 */
  reloadUiDisplay: () => Promise<void>;
}

const UiDisplayContext = createContext<UiDisplayContextValue>({
  showBusinessNo: true,
  setShowBusinessNo: async () => {},
  reloadUiDisplay: async () => {},
});

const readCachedShowBusinessNo = (): boolean => {
  try {
    return localStorage.getItem(STORAGE_KEY) !== '0';
  } catch {
    return true;
  }
};

const writeCachedShowBusinessNo = (value: boolean) => {
  try {
    localStorage.setItem(STORAGE_KEY, value ? '1' : '0');
  } catch {
    // 隐私模式下 localStorage 不可写时忽略：本次会话内仍然按内存状态生效。
  }
};

export const UiDisplayProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const { token, isLoggedIn } = useUser();
  const [showBusinessNo, setShowBusinessNoState] = useState<boolean>(readCachedShowBusinessNo);

  const reloadUiDisplay = useCallback(async () => {
    if (!isLoggedIn) return;
    try {
      const response = await getSetting(UI_DISPLAY_SETTING_KEY);
      const raw = response.code === 0 ? response.data?.value : '';
      if (!raw) return;
      const parsed = JSON.parse(raw) as Partial<UiDisplaySettings>;
      if (typeof parsed?.show_business_no === 'boolean') {
        setShowBusinessNoState(parsed.show_business_no);
        writeCachedShowBusinessNo(parsed.show_business_no);
      }
    } catch {
      // 尚未配置（404）或网络异常时沿用本地缓存，避免界面闪烁。
    }
  }, [isLoggedIn]);

  useEffect(() => {
    void reloadUiDisplay();
  }, [reloadUiDisplay, token]);

  const setShowBusinessNo = useCallback(async (value: boolean, persist = false) => {
    setShowBusinessNoState(value);
    writeCachedShowBusinessNo(value);
    if (!persist) return;
    const response = await updateSetting(UI_DISPLAY_SETTING_KEY, { show_business_no: value } satisfies UiDisplaySettings);
    if (response.code !== 0) throw new Error(response.message || '业务编号显示设置保存失败');
  }, []);

  return (
    <UiDisplayContext.Provider value={{ showBusinessNo, setShowBusinessNo, reloadUiDisplay }}>
      {children}
    </UiDisplayContext.Provider>
  );
};

export const useUiDisplay = () => useContext(UiDisplayContext);
