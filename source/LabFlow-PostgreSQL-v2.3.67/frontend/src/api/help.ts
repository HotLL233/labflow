import { client } from './http';
import type {
  ApiResponse,
  HelpDocument,
  HelpArticle,
  HelpAttachment,
} from '../types';

// ========== v0.4.11: 甯姪鏂囨。 API ==========

export const getHelpDocuments = (visibleOnly?: boolean): Promise<ApiResponse<HelpDocument[]>> =>
  client.get('/help-documents', { params: { visible_only: visibleOnly ?? true } }).then((r) => r.data);

export const uploadHelpDocument = (formData: FormData): Promise<ApiResponse<HelpDocument>> =>
  client.post('/help-documents', formData, { headers: { 'Content-Type': 'multipart/form-data' } }).then((r) => r.data);

export const updateHelpDocument = (id: number, data: { title?: string; is_visible?: boolean; sort_order?: number }): Promise<ApiResponse<HelpDocument>> =>
  client.put(`/help-documents/${id}`, data).then((r) => r.data);

export const deleteHelpDocument = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/help-documents/${id}`, { params: { reason } }).then((r) => r.data);

export const getHelpDocumentFileUrl = (id: number): string =>
  `/api/help-documents/${id}/file`;

export const getHelpDocumentPageUrl = (id: number, page: number): string =>
  `/api/help-documents/${id}/pages/${page}`;

export const getHelpDocumentFileBlob = (id: number): Promise<Blob> =>
  client.get(`/help-documents/${id}/file`, { responseType: 'blob' }).then(r => r.data);

export const getHelpDocumentPageBlob = (id: number, page: number): Promise<Blob> =>
  client.get(`/help-documents/${id}/pages/${page}`, { responseType: 'blob' }).then(r => r.data);

export const getHelpArticleImageBlob = (url: string): Promise<Blob> =>
  client.get(url.replace(/^\/api/, ''), { responseType: 'blob' }).then(r => r.data);

export const getHelpAttachments = (visibleOnly?: boolean): Promise<ApiResponse<HelpAttachment[]>> =>
  client.get('/help-attachments', { params: { visible_only: visibleOnly ?? true } }).then(r => r.data);
export const uploadHelpAttachment = (formData: FormData): Promise<ApiResponse<HelpAttachment>> =>
  client.post('/help-attachments', formData, { headers: { 'Content-Type': 'multipart/form-data' } }).then(r => r.data);
export const getHelpAttachmentFileUrl = (id: number): string => `/api/help-attachments/${id}/file`;
export const updateHelpAttachment = (id: number, data: { title?: string; is_visible?: boolean; sort_order?: number }): Promise<ApiResponse<HelpAttachment>> =>
  client.put(`/help-attachments/${id}`, data).then(r => r.data);
export const deleteHelpAttachment = (id: number): Promise<ApiResponse<null>> =>
  client.delete(`/help-attachments/${id}`).then(r => r.data);

// v0.4.19: 缁撴瀯鍖栨枃绔?
export const getHelpArticles = (visibleOnly?: boolean): Promise<ApiResponse<HelpArticle[]>> =>
  client.get('/help-articles', { params: { visible_only: visibleOnly ?? true } }).then(r => r.data);

export const getHelpArticle = (id: number): Promise<ApiResponse<HelpArticle>> =>
  client.get(`/help-articles/${id}`).then(r => r.data);

export const deleteHelpArticle = (id: number, reason?: string): Promise<ApiResponse<null>> =>
  client.delete(`/help-articles/${id}`, { params: { reason } }).then(r => r.data);

export const updateHelpArticle = (id: number, data: { title?: string; content_html?: string; is_visible?: boolean; sort_order?: number }): Promise<ApiResponse<HelpArticle>> =>
  client.put(`/help-articles/${id}`, data).then(r => r.data);

export const reorderHelpDocuments = (ids: { id: number; sort_order: number }[]): Promise<ApiResponse<null>> =>
  client.put('/help-documents/sort', { ids }).then(r => r.data);

export const reorderHelpArticles = (ids: { id: number; sort_order: number }[]): Promise<ApiResponse<null>> =>
  client.put('/help-articles/sort', { ids }).then(r => r.data);

export interface PersonnelFeedback {
  id: number; notice_no: string; lab_id: number; lab_name: string;
  responsible_user_id?: number | null; responsible_name: string;
  change_type: string; person_name: string; effective_at: string; notes: string; status: string;
  project_ids: number[]; project_names: string[]; created_by_user_id: number;
  created_by_username: string; created_at: string; updated_at: string;
}

export const getPersonnelFeedback = (): Promise<ApiResponse<PersonnelFeedback[]>> =>
  client.get('/personnel-feedback').then(r => r.data);
export const createPersonnelFeedback = (data: { lab_id: number; responsible_user_id?: number | null; change_type: string; person_name: string; effective_at: string; notes?: string; project_ids: number[] }): Promise<ApiResponse<PersonnelFeedback>> =>
  client.post('/personnel-feedback', data).then(r => r.data);
export const updatePersonnelFeedback = (id: number, data: Partial<{ responsible_user_id: number | null; change_type: string; person_name: string; effective_at: string; notes: string; project_ids: number[] }>): Promise<ApiResponse<PersonnelFeedback>> =>
  client.put(`/personnel-feedback/${id}`, data).then(r => r.data);
export const withdrawPersonnelFeedback = (id: number): Promise<ApiResponse<PersonnelFeedback>> =>
  client.post(`/personnel-feedback/${id}/withdraw`).then(r => r.data);
