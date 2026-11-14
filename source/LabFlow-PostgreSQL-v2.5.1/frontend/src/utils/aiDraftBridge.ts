import { useEffect, useSyncExternalStore } from 'react';

export type AiEntryTarget = 'sample_info' | 'rd_sample' | 'workload' | 'auxiliary';
export interface AiEntryDraft {
  id: string;
  account_id: number;
  target: AiEntryTarget;
  title: string;
  route: string;
  fields: Record<string, unknown>;
  labels: Record<string, string>;
  issues: string[];
  created_at: string;
}

// Business drafts live only in this tab's memory, never browser storage.
let pending: AiEntryDraft | null = null;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach(listener => listener());
const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
const snapshot = () => pending;
const serverSnapshot = () => null;

export function queueAiDraft(draft: AiEntryDraft, accountId: number): void {
  if (!accountId || draft.account_id !== accountId || !draft.id || !draft.fields || !draft.labels || !Array.isArray(draft.issues)) {
    throw new Error('草稿不属于当前账号，请重新生成');
  }
  const groupId = Number(draft.fields.group_id);
  const expected = draft.target === 'sample_info' ? `/sample-info/entry?type=${encodeURIComponent(String(draft.fields.type_key || ''))}`
    : draft.target === 'rd_sample' ? `/sample/${groupId}`
    : draft.target === 'workload' ? `/entry/${groupId}?subject_user_id=${Number(draft.fields.user_id)}`
    : draft.target === 'auxiliary' ? '/auxiliary' : '';
  if (!expected || draft.route !== expected || (['rd_sample', 'workload'].includes(draft.target) && (!Number.isSafeInteger(groupId) || groupId <= 0))) {
    throw new Error('草稿表单地址无效，请重新生成');
  }
  pending = JSON.parse(JSON.stringify(draft)) as AiEntryDraft;
  emit();
}

export function clearAiDraft(accountId?: number): void {
  if (pending && (accountId === undefined || pending.account_id === accountId)) { pending = null; emit(); }
}

export function useAiDraft(accountId: number | undefined, target: AiEntryTarget, actualPersonId?: number) {
  const draft = useSyncExternalStore(subscribe, snapshot, serverSnapshot);
  useEffect(() => {
    if (pending && (pending.account_id !== accountId || (pending.target === target && actualPersonId !== undefined && Number(pending.fields.user_id) !== actualPersonId))) clearAiDraft();
  }, [accountId, target, actualPersonId]);
  const current = draft && draft.account_id === accountId && draft.target === target && (actualPersonId === undefined || Number(draft.fields.user_id) === actualPersonId) ? draft : null;
  return {
    aiDraft: current,
    dismissAiDraft: () => { if (pending?.id === current?.id) clearAiDraft(accountId); },
  };
}

export const aiDraftFieldLabels: Record<string, string> = {
  group_id: '实验室', project_id: '项目', method_id: '实际方法', original_method_id: '原关联方法',
  sender_user_id: '实际送样人', user_id: '实际人员', auxiliary_work_item_id: '辅助事项',
  division_id: '部门', type_key: '样品类型', user_name: '实际送样人', quantity: '数量',
  batch_no: '批号', main_components: '主要成分', notes: '注意事项 / 备注', recorded_at: '工作量时间',
  multiplier: '单价倍率', coefficient: '工作量系数', amount: '方法单价',
  detection_type: '检测类型',
};
