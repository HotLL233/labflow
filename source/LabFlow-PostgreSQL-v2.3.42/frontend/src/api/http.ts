import axios from 'axios';
import type { ApiResponse } from '../types';

export const client = axios.create({ baseURL: '/api' });

/**
 * The backend keeps a uniform response envelope for both successful requests
 * and business failures. HTTP 200 alone must never be treated as a write
 * success by entry pages.
 */
export class ApiBusinessError extends Error {
  constructor(public readonly code: number, message: string) {
    super(message);
    this.name = 'ApiBusinessError';
  }
}

export const requireApiSuccess = <T>(response: ApiResponse<T>, fallbackMessage = '操作失败'): T | null => {
  if (response.code !== 0) {
    throw new ApiBusinessError(response.code, response.message || fallbackMessage);
  }
  return response.data;
};

const clearStoredSession = () => {
  localStorage.removeItem('workload_token');
  localStorage.removeItem('workload_user');
  localStorage.removeItem('workload_keep_signed_in');
  localStorage.removeItem('workload_remember');
  sessionStorage.removeItem('workload_token');
  sessionStorage.removeItem('workload_user');
};

const clearInvalidSession = (message: string): boolean => {
  const sessionInvalid = ['其他设备退出', '会话已撤销', '会话已过期', '登录已过期', '无效的登录凭证', '用户不存在，请重新登录']
    .some(text => message.includes(text));
  if (!sessionInvalid) return false;
  redirectToLogin(message);
  return true;
};

/** v2.3.19: 统一会话失效处理。清本地会话 → 写入提示 → 回登录页，避免停留在假登录态。 */
const redirectToLogin = (message: string) => {
  clearStoredSession();
  sessionStorage.setItem('workload_session_notice', message);
  if (window.location.pathname !== '/login') {
    window.location.replace('/login');
  }
};

const getDownloadFilename = (disposition: string, fallback: string): string => {
  const encodedName = disposition.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  if (encodedName) {
    try { return decodeURIComponent(encodedName); } catch {}
  }
  return disposition.match(/filename="?([^";]+)"?/i)?.[1] || fallback;
};

client.interceptors.response.use(
  (res) => {
    const message = typeof res.data?.message === 'string' ? res.data.message : '';
    if (clearInvalidSession(message)) {
      return Promise.reject(new Error(message));
    }
    return res;
  },
  (err) => {
    // v0.4.27-A: 401 时清除登录态
    // v2.3.19: 补上跳转与提示；此前只清存储，界面会停留在未跳转的假登录态。
    if (err.response?.status === 401) {
      redirectToLogin(err.response?.data?.message || '登录已过期，请重新登录');
    }
    const msg = err.response?.data?.message || '网络错误';
    return Promise.reject(new Error(msg));
  }
);

// v0.4.27-A: 璇锋眰鎷︽埅鍣?鈥?鑷姩闄勫姞 JWT token
client.interceptors.request.use((config) => {
  const token =
    localStorage.getItem('workload_token') ||
    sessionStorage.getItem('workload_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

export async function downloadFile(url: string, params: Record<string, any>, filename: string): Promise<void> {
  const qs = new URLSearchParams();
  Object.entries(params).forEach(([k, v]) => { if (v !== undefined && v !== null) qs.set(k, String(v)); });
  const token = localStorage.getItem('workload_token') || sessionStorage.getItem('workload_token') || '';
  const res = await fetch(`${url}?${qs.toString()}`, {
    credentials: 'include',
    headers: token ? { 'Authorization': `Bearer ${token}` } : {},
  });
  if (!res.ok) {
    const txt = await res.text().catch(() => '');
    let msg = `导出失败 (HTTP ${res.status})`;
    let code = res.status;
    try { const j = JSON.parse(txt); if (j.message) msg = j.message; if (typeof j.code === 'number') code = j.code; } catch {}
    clearInvalidSession(msg);
    throw new ApiBusinessError(code, msg);
  }
  const contentType = (res.headers.get('content-type') || '').toLowerCase();
  if (contentType.includes('application/json') || contentType.startsWith('text/')) {
    const txt = await res.text().catch(() => '');
    let msg = '导出失败：服务器未返回文件';
    let code = 2001;
    try {
      const body = JSON.parse(txt);
      if (typeof body.message === 'string' && body.message.trim()) msg = body.message;
      if (typeof body.code === 'number') code = body.code;
    } catch {}
    clearInvalidSession(msg);
    throw new ApiBusinessError(code, msg);
  }
  const blob = await res.blob();
  if (blob.size === 0) throw new Error('导出文件为空');
  const signature = new Uint8Array(await blob.slice(0, 4).arrayBuffer());
  const isZip = signature.length === 4 && signature[0] === 0x50 && signature[1] === 0x4b
    && ((signature[2] === 0x03 && signature[3] === 0x04) || (signature[2] === 0x05 && signature[3] === 0x06));
  if (!isZip) {
    throw new Error('导出失败：服务器未返回有效的 Excel/ZIP 文件，请重新登录后再试');
  }
  const u = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = u;
  a.download = getDownloadFilename(res.headers.get('content-disposition') || '', filename);
  document.body.appendChild(a); a.click();
  document.body.removeChild(a);
  setTimeout(() => URL.revokeObjectURL(u), 1000);
}
