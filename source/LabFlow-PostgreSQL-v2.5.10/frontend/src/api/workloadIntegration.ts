import { client, requireApiSuccess } from './http';
import type { ApiResponse } from '../types';

export interface WorkloadApiConfig {
  name: string;
  kind: 'detection' | 'auxiliary' | 'both';
  departments: { mode: 'all' } | { mode: 'selected'; ids: number[] };
  from_month: string;
  through_month: string | null;
  expires_on: string | null;
  enabled: boolean;
}

export interface WorkloadApiLink {
  id: number;
  revision: number;
  expired: boolean;
  config: WorkloadApiConfig;
}

export interface IssuedWorkloadApiLink {
  link: WorkloadApiLink;
  token: string;
}

const path = '/admin/workload-api-links';
const payload = <T,>(response: { data: ApiResponse<T> }): T => {
  const data = requireApiSuccess(response.data);
  if (data == null) throw new Error('服务器未返回接入配置，请刷新核对。');
  return data;
};

export const getWorkloadApiLinks = () =>
  client.get<ApiResponse<{ endpoint: string; links: WorkloadApiLink[] }>>(path).then(payload);

export const createWorkloadApiLink = (config: WorkloadApiConfig) =>
  client.post<ApiResponse<IssuedWorkloadApiLink>>(path, config).then(payload);

export const updateWorkloadApiLink = (link: WorkloadApiLink, config: WorkloadApiConfig) =>
  client.put<ApiResponse<WorkloadApiLink>>(`${path}/${link.id}`, { revision: link.revision, config }).then(payload);

export const rotateWorkloadApiToken = (link: WorkloadApiLink) =>
  client.post<ApiResponse<IssuedWorkloadApiLink>>(`${path}/${link.id}/rotate`, { revision: link.revision }).then(payload);
