import { client, requireApiSuccess } from './http';
import type { ApiResponse } from '../types';

export interface BackupCloudSettings {
  provider: string;
  enabled: boolean;
  auto_upload: boolean;
  endpoint: string;
  username: string;
  has_password: boolean;
  onedrive: { client_id: string; tenant: string; folder: string; connected: boolean; account: string };
}
export interface BackupCloudInput {
  provider?: string; enabled: boolean; auto_upload: boolean; endpoint: string; username: string; password?: string;
  onedrive?: { client_id: string; tenant: string; folder: string };
}
export interface OneDriveLogin { id: string; user_code: string; verification_uri: string; expires_at: number; interval: number }
export interface BackupTransfer {
  id: string;
  direction: 'upload' | 'download';
  name: string;
  size: number;
  status: 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';
  attempts: number;
  detail: string;
  created_at: number;
  updated_at: number;
}
export interface BackupCloudStatus {
  settings: BackupCloudSettings;
  transfers: BackupTransfer[];
}
export interface RemoteBackup { name: string; size: number; has_manifest: boolean }

export async function getBackupCloud(): Promise<BackupCloudStatus> {
  return requireApiSuccess((await client.get<ApiResponse<BackupCloudStatus>>('/backup/cloud')).data)!;
}
export async function saveBackupCloud(data: BackupCloudInput): Promise<void> {
  requireApiSuccess((await client.put('/backup/cloud', data)).data);
}
export async function testBackupCloud(input?: BackupCloudInput): Promise<string> {
  const response = (await client.post<ApiResponse<string>>('/backup/cloud/test', input)).data;
  requireApiSuccess(response);
  return response.message;
}
export async function startOneDriveLogin(): Promise<OneDriveLogin> {
  return requireApiSuccess((await client.post<ApiResponse<OneDriveLogin>>('/backup/cloud/onedrive/login')).data)!;
}
export async function pollOneDriveLogin(id: string): Promise<boolean> {
  return requireApiSuccess((await client.post<ApiResponse<boolean>>(`/backup/cloud/onedrive/login/${encodeURIComponent(id)}`)).data) === true;
}
export async function disconnectOneDrive(): Promise<void> {
  requireApiSuccess((await client.post('/backup/cloud/onedrive/disconnect')).data);
}
export async function getRemoteBackups(): Promise<RemoteBackup[]> {
  return requireApiSuccess((await client.get<ApiResponse<RemoteBackup[]>>('/backup/cloud/files')).data) || [];
}
export async function transferBackup(name: string, direction: 'upload' | 'download'): Promise<void> {
  requireApiSuccess((await client.post(`/backup/cloud/${direction}/${encodeURIComponent(name)}`)).data);
}
export async function changeBackupTransfer(id: string, action: 'retry' | 'cancel'): Promise<void> {
  requireApiSuccess((await client.post(`/backup/cloud/jobs/${encodeURIComponent(id)}/${action}`)).data);
}

export async function downloadBackup(name?: string): Promise<void> {
  const route = name ? `/backup/file/${encodeURIComponent(name)}/download-ticket` : '/backup/latest-download-ticket';
  const response = (await client.post<ApiResponse<{ url: string; filename: string }>>(route)).data;
  const link = requireApiSuccess(response);
  if (!link) throw new Error('未获得有效下载链接');
  // A scoped, short-lived grant lets the browser stream to disk without a Blob or a login token in the URL.
  const anchor = document.createElement('a');
  anchor.href = link.url;
  anchor.download = link.filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
}
