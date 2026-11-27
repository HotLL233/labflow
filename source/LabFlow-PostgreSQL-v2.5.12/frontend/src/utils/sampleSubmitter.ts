type SubmitterAccount = {
  username?: string;
  is_public_account?: boolean;
  is_rd_public_account?: boolean;
  is_analysis_public_account?: boolean;
  require_actual_submitter?: boolean;
};

export const requiresActualSubmitter = (user?: SubmitterAccount | null) => Boolean(
  user?.is_public_account || user?.is_rd_public_account || user?.is_analysis_public_account || user?.require_actual_submitter,
);

export const defaultSubmitterName = (user?: SubmitterAccount | null) => requiresActualSubmitter(user) ? '' : user?.username || '';

export const isSubmitterPlaceholder = (value: unknown) => ['', '未知', '请填写实际送样人', '请选择送样人'].includes(String(value ?? '').trim());

export const submitterNeedsConfirmation = (row: { user_name?: unknown; _submitterConfirmed?: boolean }, user?: SubmitterAccount | null) => (
  requiresActualSubmitter(user) && row._submitterConfirmed !== true
  && (isSubmitterPlaceholder(row.user_name) || String(row.user_name ?? '').trim() === user?.username)
);
