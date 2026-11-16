/** A row keeps this identity until its submission is confirmed. Works on LAN HTTP too. */
export function createSubmissionId(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') return globalThis.crypto.randomUUID();
  const bytes = globalThis.crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const value = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
  return `${value.slice(0, 8)}-${value.slice(8, 12)}-${value.slice(12, 16)}-${value.slice(16, 20)}-${value.slice(20)}`;
}

export function submissionId(row: { _submissionId?: string }): string {
  return row._submissionId || createSubmissionId();
}

/** A malformed success or an unclassified transport failure may already have committed. */
export function submissionResultUncertain(error: any): boolean {
  // Database/internal errors can include a lost COMMIT response. Only an explicit
  // validation or permission refusal proves this submission was not committed.
  if (error?.name === 'ApiBusinessError') return error.code !== 1001 && error.code !== 1003;
  return error?.resultUncertain !== false;
}

export const SUBMISSION_STORAGE_PREFIX = 'labflow:sample-submission:';

export function readPendingSubmissionRows(key: string): any[] | null {
  try {
    const saved = sessionStorage.getItem(key);
    if (saved === null) return null;
    const rows = JSON.parse(saved);
    if (Array.isArray(rows) && rows.length > 0 && rows.every(row =>
      row && typeof row === 'object' && typeof row._submissionId === 'string')) return rows;
    throw new Error('invalid pending submission');
  } catch { throw new Error('待核对草稿读取失败。为避免重复送样已暂停提交，请先核对记录，再重置本页。'); }
}

export function storePendingSubmissionRows(key: string, rows: any[]): void {
  if (rows.length > 0) sessionStorage.setItem(key, JSON.stringify(rows));
  else sessionStorage.removeItem(key);
}
