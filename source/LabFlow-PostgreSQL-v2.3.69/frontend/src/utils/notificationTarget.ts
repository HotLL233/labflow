import type { InAppNotification } from '../types';

export function notificationTarget(item: Pick<InAppNotification, 'target_url' | 'analysis_group_id'>, hasPermission: (key: string) => boolean): string | null {
  const target = item.target_url;
  if (!target || !target.startsWith('/') || target.startsWith('//') || /[\\\u0000-\u0020]/.test(target)) return null;
  const url = new URL(target, 'https://labflow.invalid');
  if (url.origin !== 'https://labflow.invalid') return null;
  if (url.pathname === '/sample-records' && !hasPermission('records:rd:view-all')) {
    const groupId = item.analysis_group_id;
    if (hasPermission('records:rd:view-work-lab') && Number.isSafeInteger(groupId) && groupId && groupId > 0) {
      url.searchParams.set('tab', 'records');
      return `/entry/${groupId}${url.search}`;
    }
    if (!['entry:sample', 'records:rd:view-lab'].some(hasPermission)) return null;
  }
  return `${url.pathname}${url.search}${url.hash}`;
}
