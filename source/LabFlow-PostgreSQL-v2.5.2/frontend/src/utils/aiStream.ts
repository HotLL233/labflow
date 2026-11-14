/** Parse SSE incrementally: upstream chunks may split UTF-8, CRLF or JSON. */
export function createAiEventParser(onEvent: (event: string, data: unknown) => void) {
  let pending = '';
  return (chunk: string, end = false) => {
    pending += chunk;
    const frames = pending.split(/\r?\n\r?\n/);
    pending = frames.pop() || '';
    if (end && pending.trim()) { frames.push(pending); pending = ''; }
    for (const frame of frames) {
      let event = 'message';
      const data: string[] = [];
      for (const line of frame.split(/\r?\n/)) {
        if (line.startsWith('event:')) event = line.slice(6).trim();
        if (line.startsWith('data:')) data.push(line.slice(5).replace(/^ /, ''));
      }
      if (!data.length) continue;
      const raw = data.join('\n');
      if (raw === '[DONE]') { onEvent('done', {}); continue; }
      try { onEvent(event, JSON.parse(raw)); }
      catch (error) { if (error instanceof SyntaxError) throw new Error('助手响应格式异常，请重试。'); throw error; }
    }
  };
}

export const AI_AUDIO_LIMIT = 10 * 1024 * 1024;
export function validateAiAudio(file: { size: number; type: string; name?: string }): string {
  if (!file.size) return '音频文件为空。';
  if (file.size > AI_AUDIO_LIMIT) return '每段音频不能超过 10MB。';
  const mime = file.type.toLowerCase();
  if (mime.startsWith('audio/') || mime === 'video/webm' || (!mime && /\.(webm|mp3|mp4|m4a|wav|ogg|flac)$/i.test(file.name || ''))) return '';
  return '请选择 WebM、MP3、MP4、M4A、WAV、OGG 或 FLAC 音频。';
}
