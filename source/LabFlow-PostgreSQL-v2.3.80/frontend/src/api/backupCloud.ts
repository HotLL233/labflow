import { client, requireApiSuccess } from './http';
import type { ApiResponse } from '../types';

export interface BackupCloudSettings {
  enabled: boolean;
  auto_upload: boolean;
  endpoint: string;
  username: string;
  has_password: boolean;
}
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
export async function saveBackupCloud(data: Omit<BackupCloudSettings, 'has_password'> & { password?: string }): Promise<void> {
  requireApiSuccess((await client.put('/backup/cloud', data)).data);
}
export async function testBackupCloud(): Promise<string> {
  const response = (await client.post<ApiResponse<string>>('/backup/cloud/test')).data;
  requireApiSuccess(response);
  return response.message;
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
