export const APP_NAME = 'labflow';
export const APP_PUBLISHER = 'HotLL';

export function brandName(value?: unknown): string {
  return typeof value !== 'string' || !value || value === '知微' || value === '样品管理系统' ? APP_NAME : value;
}
