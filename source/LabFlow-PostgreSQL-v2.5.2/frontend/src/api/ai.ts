import type { ApiResponse } from '../types';
import { client, requireApiSuccess } from './http';
import { createAiEventParser } from '../utils/aiStream';

export type AiModel = { id: string; name: string };
export type AiProtocol = 'openai' | 'anthropic' | 'ollama';
export type AiTestKind = 'connectivity' | 'hi' | 'stream' | 'tools';
export type AiAbility = { help: boolean; query: boolean; draft: boolean; report: boolean; voice: boolean; notification_read: boolean; submit: boolean };
export type AiProvider = { id: string; name: string; protocol: AiProtocol; url: string; has_secret: boolean; no_key: boolean; models: AiModel[]; default_model: string; secret?: string; clear_secret?: boolean };
export type AiSpeech = { enabled: boolean; url: string; model: string; has_secret: boolean; no_key: boolean; secret?: string; clear_secret?: boolean };
export type AiTest = { provider_id: string; model: string; kind: AiTestKind; passed: boolean; reachable: boolean; usable: boolean; message: string; latency_ms: number; checked_at: string };
export type AiConfig = { revision: number; enabled: boolean; name: string; default_provider: string; capabilities: AiAbility; providers: AiProvider[]; speech: AiSpeech; tests: AiTest[]; timeout_seconds: number; max_requests_per_minute: number };
export type AiCapabilities = { enabled: boolean; can_use: boolean; can_manage: boolean; name: string; provider_label?: string; capabilities: AiAbility; speech_enabled: boolean; speech_destination?: string; can_sample?: boolean; can_work?: boolean; can_aux?: boolean; can_report?: boolean; models: Array<AiModel & { provider_id: string; provider_name: string; chat_ready: boolean; tools_ready: boolean }>; default_provider: string; default_model: string };
export type AiChatMessage = { role: 'user' | 'assistant'; content: string };
export type AiChatContext = { include_business: boolean; allow_notification_read: boolean; allow_submit: boolean; date_from?: string; date_to?: string };
export type AiEvent = { text?: string; message?: string; name?: string; status?: string; type?: string; data?: unknown; changed?: number };
export type AiPublicAction = {
  id: string; kind: 'notification_read' | 'submit'; status: 'pending' | 'processing' | 'completed' | 'unknown';
  expires_at: string; summary: string; expired: boolean; can_confirm: boolean;
  items: Array<{ index: number; target: string; title: string; preview: Array<{ label: string; value: string }>; issues: string[] }>;
  results: Array<{ index: number; status: 'succeeded' | 'failed' | 'unknown'; record_id?: number; route?: string; message: string }>;
};

async function unwrap<T>(request: Promise<{ data: ApiResponse<T> }>): Promise<T> {
  const result = requireApiSuccess((await request).data, 'AI 服务请求失败');
  if (result == null) throw new Error('服务器未返回有效数据。');
  return result;
}
export const getAiConfig = () => unwrap<AiConfig>(client.get('/ai/config'));
export const saveAiConfig = (config: AiConfig) => {
  const { tests: _, providers, speech, ...rest } = config;
  const clean = (item: AiProvider | AiSpeech) => {
    const { has_secret: _secret, ...value } = item;
    if (!value.secret) delete value.secret;
    return value;
  };
  return unwrap<AiConfig>(client.put('/ai/config', { ...rest, providers: providers.map(clean), speech: clean(speech) }));
};
export const getAiCapabilities = () => unwrap<AiCapabilities>(client.get('/ai/capabilities'));
export const getAiAction = (id: string, signal?: AbortSignal) => unwrap<AiPublicAction>(client.get(`/ai/actions/${encodeURIComponent(id)}`, { signal }));
export const confirmAiAction = (id: string, signal?: AbortSignal) => unwrap<AiPublicAction>(client.post(`/ai/actions/${encodeURIComponent(id)}/confirm`, undefined, { signal }));
export const discoverAiModels = (provider_id: string, signal?: AbortSignal) => unwrap<{ models: AiModel[]; truncated: boolean }>(client.post('/ai/models', { provider_id }, { signal }));
export const testAiModel = (provider_id: string, model: string, kind: AiTestKind, signal?: AbortSignal) => unwrap<AiTest>(client.post('/ai/test', { provider_id, model, kind }, { signal }));
export const transcribeAiAudio = (file: File, signal?: AbortSignal) => {
  const form = new FormData(); form.append('file', file);
  return unwrap<{ text: string }>(client.post('/ai/transcribe', form, { signal }));
};
export async function streamAiChat(options: { token: string; messages: AiChatMessage[]; provider_id?: string; model?: string; context?: AiChatContext; signal: AbortSignal; onEvent: (event: string, data: AiEvent) => void }) {
  const { token, signal, onEvent, ...body } = options;
  const response = await fetch('/api/ai/chat', { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }, body: JSON.stringify(body), signal });
  if (!response.ok || !response.headers.get('content-type')?.includes('text/event-stream')) {
    const value = await response.json().catch(() => null);
    if (response.status === 401) await client.get('/users/me');
    throw new Error(value?.message || `助手暂时无法响应（HTTP ${response.status}）。`);
  }
  if (!response.body) throw new Error('当前浏览器不支持流式响应。');
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let completed = false;
  const parse = createAiEventParser((event, value) => {
    const data = value as AiEvent;
    if (event === 'error') throw new Error(data.message || '生成失败，请重试。');
    if (event === 'done') completed = true;
    onEvent(event, data);
  });
  try {
    while (true) {
      const { value, done } = await reader.read();
      parse(decoder.decode(value, { stream: !done }), done);
      if (done) break;
    }
    if (!completed) throw new Error('连接提前中断，当前回答尚未完成。');
  } finally { await reader.cancel().catch(() => undefined); reader.releaseLock(); }
}
