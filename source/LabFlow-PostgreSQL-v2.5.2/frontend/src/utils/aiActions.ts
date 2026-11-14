import type { AiPublicAction } from '../api/ai';

export function asAiAction(value: unknown): AiPublicAction | null {
  if (!value || typeof value !== 'object') return null;
  const action = value as AiPublicAction;
  if (typeof action.id !== 'string' || !action.id || action.id.length > 200
    || !['submit', 'notification_read'].includes(action.kind)
    || !['pending', 'processing', 'completed', 'unknown'].includes(action.status)
    || typeof action.summary !== 'string' || typeof action.expires_at !== 'string' || !Number.isFinite(Date.parse(action.expires_at))
    || typeof action.expired !== 'boolean' || typeof action.can_confirm !== 'boolean'
    || !Array.isArray(action.items) || !action.items.length || !Array.isArray(action.results)) return null;
  if (!action.items.every(item => item && typeof item === 'object' && Number.isInteger(item.index) && typeof item.target === 'string' && typeof item.title === 'string'
    && Array.isArray(item.preview) && item.preview.every(field => field && typeof field === 'object' && typeof field.label === 'string' && typeof field.value === 'string')
    && item.index >= 0 && Array.isArray(item.issues) && item.issues.every(issue => typeof issue === 'string'))) return null;
  const indexes = new Set(action.items.map(item => item.index));
  if (indexes.size !== action.items.length) return null;
  if (!action.results.every(result => result && typeof result === 'object' && Number.isInteger(result.index) && ['succeeded', 'failed', 'unknown'].includes(result.status)
    && indexes.has(result.index) && typeof result.message === 'string' && (result.route === undefined || typeof result.route === 'string'))) return null;
  if (new Set(action.results.map(result => result.index)).size !== action.results.length) return null;
  return action;
}

export const canConfirmAiAction = (action: AiPublicAction, allowed: boolean, now = Date.now()) => allowed
  && action.kind === 'submit' && action.status === 'pending' && action.can_confirm && !action.expired
  && Date.parse(action.expires_at) > now && action.items.length > 0 && action.items.length <= 20
  && action.items.every(item => item.preview.length > 0 && !item.issues.length) && !action.results.length;

export const aiActionNeedsRecovery = (action: AiPublicAction) => action.status === 'processing' || action.status === 'unknown'
  || action.results.some(item => item.status === 'unknown') || (action.status === 'completed' && action.results.length !== action.items.length);

export const aiActionStatusLabel = (action: AiPublicAction) => action.expired && action.status === 'pending' ? '预览已过期'
  : action.status === 'processing' ? '正在提交'
    : aiActionNeedsRecovery(action) ? '结果待核对'
      : action.status !== 'completed' ? '待确认'
        : action.results.every(item => item.status === 'succeeded') ? '已提交'
          : action.results.some(item => item.status === 'succeeded') ? '部分提交成功' : '提交未成功';

export const aiActionRecordRoute = (route?: string) => route?.startsWith('/') && !route.startsWith('//') && !/[\\\u0000-\u0020]/.test(route) ? route : null;
