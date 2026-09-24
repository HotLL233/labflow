import { client, downloadFile } from './http';
import type { OwnershipQuery, SampleInfoStatsQuery } from './workload';
import type { SampleWorkloadPreview } from '../components/SampleWorkloadDialog';
import type {
  ApiResponse,
  PaginatedResponse,
  SampleInfoRecord,
  SampleInfoColumn,
  SampleInfoAttachment,
  SampleInfoType,
  ColumnVisibilityItem,
} from '../types';

// ========== v0.4.22: 鏍峰搧淇℃伅鐧昏 API ==========

export const getSampleInfoRecords = (params?: { detection_type?: string; type_key?: string; status?: string; user_name?: string; lab_name?: string; project_name?: string; start?: string; end?: string; page?: number; page_size?: number; include_deleted?: boolean; sort_by?: string; sort_dir?: 'asc' | 'desc' } & OwnershipQuery): Promise<ApiResponse<PaginatedResponse<SampleInfoRecord>>> =>
  client.get('/sample-info', { params }).then(r => r.data);

export const createSampleInfo = (data: { batch_no: string; user_name: string; lab_name: string; project_name: string; submitted_at?: string; detection_date?: string; main_components: string; detection_type: string; type_key: string; division_id?: number | null; quantity?: number; notes?: string; extra_fields?: Record<string, any> }): Promise<ApiResponse<SampleInfoRecord>> =>
  client.post('/sample-info', data).then(r => r.data);

export const createSampleInfoWithAttachments = (data: { batch_no: string; user_name: string; lab_name: string; project_name: string; submitted_at?: string; detection_date?: string; main_components: string; detection_type: string; type_key: string; division_id?: number | null; quantity?: number; notes?: string; extra_fields?: Record<string, any> }, files: File[]): Promise<ApiResponse<SampleInfoRecord>> => {
  const form = new FormData();
  form.append('payload', JSON.stringify(data));
  files.forEach(file => form.append('files', file));
  return client.post('/sample-info/with-attachments', form).then(r => r.data);
};

export const getSampleInfoDrafts = (): Promise<ApiResponse<import('../types').SampleInfoDraft[]>> => client.get('/sample-info/drafts').then(r => r.data);
export const createSampleInfoDraft = (data: { type_key: string; title?: string; payload: Record<string, any> }): Promise<ApiResponse<import('../types').SampleInfoDraft>> => client.post('/sample-info/drafts', data).then(r => r.data);
export const updateSampleInfoDraft = (id: number, data: { type_key: string; title?: string; payload: Record<string, any> }): Promise<ApiResponse<import('../types').SampleInfoDraft>> => client.put(`/sample-info/drafts/${id}`, data).then(r => r.data);
export const deleteSampleInfoDraft = (id: number): Promise<ApiResponse<null>> => client.delete(`/sample-info/drafts/${id}`).then(r => r.data);
export const getSampleInfoDraftAttachments = (draftId: number): Promise<ApiResponse<import('../types').SampleInfoDraftAttachment[]>> => client.get(`/sample-info/drafts/${draftId}/attachments`).then(r => r.data);
export const uploadSampleInfoDraftAttachment = (draftId: number, rowIndex: number, file: File): Promise<ApiResponse<import('../types').SampleInfoDraftAttachment>> => {
  const form = new FormData();
  form.append('row_index', String(rowIndex));
  form.append('file', file);
  return client.post(`/sample-info/drafts/${draftId}/attachments`, form).then(r => r.data);
};
export const downloadSampleInfoDraftAttachment = (attachmentId: number): Promise<Blob> =>
  client.get(`/sample-info/drafts/attachments/${attachmentId}/file`, { responseType: 'blob' }).then(r => r.data);
export const deleteSampleInfoDraftAttachment = (attachmentId: number): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/drafts/attachments/${attachmentId}`).then(r => r.data);

export const updateSampleInfo = (id: number, data: { status?: string; batch_no?: string; user_name?: string; lab_name?: string; project_name?: string; submitted_at?: string; detection_date?: string; main_components?: string; type_key?: string; division_id?: number | null; quantity?: number; notes?: string; extra_fields?: Record<string, any> }): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}`, data).then(r => r.data);

export const deleteSampleInfo = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/${id}`, { params: { reason } }).then(r => r.data);

export const restoreSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.post(`/sample-info/${id}/restore`).then(r => r.data);

export const updateSampleInfoStatus = (id: number, status: string): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/status`, { status }).then(r => r.data);

export const sampleSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/sample`, {}).then(r => r.data);

export const getSampleInfoWorkloadPreview = (id: number): Promise<ApiResponse<SampleWorkloadPreview>> =>
  client.get(`/sample-info/${id}/sample-workload`).then(r => r.data);

// v2.3.14: 优先提交方法库选中的 method_id；方法库没有候选时提交自定义的方法/仪器名称。
export const createSampleInfoWorkload = (id: number, data: { method_id?: number; custom_method_name?: string; custom_instrument_name?: string; quantity: number; multiplier: number; notes?: string }): Promise<ApiResponse<import('../types').WorkRecord>> =>
  client.post(`/sample-info/${id}/sample-workload`, data).then(r => r.data);

export const withdrawSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/withdraw`, {}).then(r => r.data);

export const completeSampleInfo = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.put(`/sample-info/${id}/complete`, {}).then(r => r.data);
export const returnSampleInfo = (id: number, reason: string): Promise<ApiResponse<SampleInfoRecord>> =>
  client.post(`/sample-info/${id}/return`, { reason }).then(r => r.data);
export const confirmSampleInfoReturn = (id: number): Promise<ApiResponse<SampleInfoRecord>> =>
  client.post(`/sample-info/${id}/confirm-return`).then(r => r.data);

// 鐙珛缁熻锛堜笉鎺ュ垎鏋愭娴?/stats锛?
export const getSampleInfoStats = (params?: SampleInfoStatsQuery): Promise<ApiResponse<any>> =>
  client.get('/sample-info/stats', { params }).then(r => r.data);

// ========== v0.4.23: 妫€娴嬬被鍨?CRUD ==========
export const getSampleInfoTypes = (): Promise<ApiResponse<SampleInfoType[]>> =>
  client.get('/sample-info-types').then(r => r.data);

export const getSampleInfoTypesAll = (): Promise<ApiResponse<SampleInfoType[]>> =>
  client.get('/sample-info-types/all').then(r => r.data);

export const createSampleInfoType = (data: { type_key: string; label: string; description?: string; color?: string; sort_order?: number }): Promise<ApiResponse<SampleInfoType>> =>
  client.post('/sample-info-types', data).then(r => r.data);

export const updateSampleInfoType = (id: number, data: { type_key?: string; label?: string; description?: string; color?: string; sort_order?: number; is_active?: number }): Promise<ApiResponse<SampleInfoType>> =>
  client.put(`/sample-info-types/${id}`, data).then(r => r.data);

export const deleteSampleInfoType = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info-types/${id}`, { params: { reason } }).then(r => r.data);

// ========== v0.4.23: 鏍峰搧淇℃伅鐧昏瀵煎嚭锛堢嫭绔嬫帴鍙ｏ級 ==========
export const exportSampleInfo = (params: SampleInfoStatsQuery): Promise<void> =>
  downloadFile('/api/sample-info/export', params, `样品信息登记_${params.start?.substring(0, 10) ?? ''}_${params.end?.substring(0, 10) ?? ''}.xlsx`);

// ========== v0.4.26: 鍒楄嚜瀹氫箟 API ==========
export const getSampleInfoColumns = (typeKey?: string): Promise<ApiResponse<SampleInfoColumn[]>> =>
  client.get('/sample-info/columns', { params: { type_key: typeKey || undefined } }).then(r => r.data);

export const getActiveSampleInfoColumns = (typeKey?: string): Promise<ApiResponse<SampleInfoColumn[]>> =>
  client.get('/sample-info/columns/active', { params: { type_key: typeKey || undefined } }).then(r => r.data);

// v0.4.27-A: 绠＄悊椤典笓鐢?鈥?鍒?+ 鍙鎬т俊鎭?
export const getSampleInfoColumnsManage = (typeKey: string): Promise<ApiResponse<Array<SampleInfoColumn & { is_visible_in_type: boolean }>>> =>
  client.get('/sample-info/columns/manage', { params: { type_key: typeKey } }).then(r => r.data);

// v0.4.27-A: 鎵归噺鏇存柊棰勭疆鍒楀彲瑙佹€?
export const updateSampleInfoColumnVisibility = (data: { type_key: string; items: ColumnVisibilityItem[] }): Promise<ApiResponse<null>> =>
  client.put('/sample-info/columns/visibility', data).then(r => r.data);

export const updateSampleInfoColumnTypes = (id: number, typeRules: { type_key: string; is_visible: boolean; is_required: boolean; show_in_form?: boolean; show_in_list?: boolean; show_in_export?: boolean; sort_order?: number }[]): Promise<ApiResponse<SampleInfoColumn>> =>
  client.put('/sample-info/columns/' + id + '/types', { type_rules: typeRules }).then(r => r.data);

export const createSampleInfoColumn = (data: {
  field_key: string;
  label: string;
  data_type: string;
  width?: number;
  /** v2.3.23：auto 按内容测量，custom 使用 width 作为固定宽度 */
  width_mode?: 'auto' | 'custom';
  min_width?: number;
  max_width?: number;
  sort_order?: number;
  options?: string;
  is_required?: boolean;
  show_in_list?: boolean;
  show_in_export?: boolean;
  show_in_form?: boolean;
}): Promise<ApiResponse<SampleInfoColumn>> =>
  client.post('/sample-info/columns', data).then(r => r.data);

export const updateSampleInfoColumn = (id: number, data: {
  label?: string;
  data_type?: string;
  is_active?: boolean;
  is_required?: boolean;
  width?: number;
  /** v2.3.23：auto 按内容测量，custom 使用 width 作为固定宽度 */
  width_mode?: 'auto' | 'custom';
  min_width?: number;
  max_width?: number;
  options?: string;
  show_in_list?: boolean;
  show_in_export?: boolean;
  show_in_form?: boolean;
}): Promise<ApiResponse<SampleInfoColumn>> =>
  client.put(`/sample-info/columns/${id}`, data).then(r => r.data);

export const deleteSampleInfoColumn = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/columns/${id}`, { params: { reason } }).then(r => r.data);

export const reorderSampleInfoColumns = (ids: { id: number; sort_order: number }[]): Promise<ApiResponse<SampleInfoColumn[]>> =>
  client.put('/sample-info/columns/sort', { ids }).then(r => r.data);

// ========== v0.4.27-A: 闄勪欢 API ==========
export const getSampleInfoAttachments = (recordId: number): Promise<ApiResponse<SampleInfoAttachment[]>> =>
  client.get(`/sample-info/${recordId}/attachments`).then(r => r.data);

export const uploadSampleInfoAttachment = (recordId: number, file: File): Promise<ApiResponse<SampleInfoAttachment>> => {
  const fd = new FormData();
  fd.append('file', file);
  return client.post(`/sample-info/${recordId}/attachments`, fd).then(r => r.data);
};

export const downloadSampleInfoAttachment = (attId: number): Promise<Blob> =>
  client.get(`/sample-info/attachments/${attId}/file`, { responseType: 'blob' }).then(r => r.data);

export const getSampleInfoAttachmentDocxPreview = (attId: number): Promise<ApiResponse<{ html: string }>> =>
  client.get(`/sample-info/attachments/${attId}/docx-preview`).then(r => r.data);

export type SampleInfoAttachmentPreviewStatus = {
  status: 'queued' | 'generating' | 'first_page_ready' | 'ready' | 'failed';
  page_count: number;
  first_page_ready: boolean;
  error?: string | null;
};

export const getSampleInfoAttachmentImagePreview = (attId: number): Promise<ApiResponse<SampleInfoAttachmentPreviewStatus>> =>
  client.get(`/sample-info/attachments/${attId}/image-preview`).then(r => r.data);

export const getSampleInfoAttachmentPreviewPage = (attId: number, page: number): Promise<Blob> =>
  client.get(`/sample-info/attachments/${attId}/image-preview/${page}`, { responseType: 'blob' }).then(r => r.data);

export const getSampleInfoAttachmentPreviewPdf = (attId: number): Promise<Blob> =>
  client.get(`/sample-info/attachments/${attId}/preview-pdf`, { responseType: 'blob' }).then(r => r.data);

export const openSampleInfoAttachment = async (attachment: SampleInfoAttachment): Promise<void> => {
  const blob = await downloadSampleInfoAttachment(attachment.id);
  const url = URL.createObjectURL(blob);
  const isPdf = attachment.file_type === 'application/pdf' || attachment.file_name.toLowerCase().endsWith('.pdf');
  if (isPdf) {
    window.open(url, '_blank', 'noopener,noreferrer');
    window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
    return;
  }
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = attachment.file_name;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
};

export const deleteSampleInfoAttachment = (attId: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/sample-info/attachments/${attId}`, { params: { reason } }).then(r => r.data);

// v0.4.28: 鎵归噺鑾峰彇闄勪欢
export const batchGetSampleInfoAttachments = (recordIds: number[]): Promise<ApiResponse<Record<number, SampleInfoAttachment[]>>> => {
  if (recordIds.length === 0) return Promise.resolve({ code: 0, message: 'ok', data: {} } as any);
  return client.get('/sample-info/attachments/batch', { params: { record_ids: recordIds.join(',') } }).then(r => r.data);
};
