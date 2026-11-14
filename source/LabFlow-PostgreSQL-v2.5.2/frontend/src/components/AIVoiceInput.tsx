import { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, Stack, TextField, Typography } from '@mui/material';
import MicOutlinedIcon from '@mui/icons-material/MicOutlined';
import { transcribeAiAudio } from '../api/ai';
import { AI_AUDIO_LIMIT, validateAiAudio } from '../utils/aiStream';

function checkUploadedDuration(file: File, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file); const audio = document.createElement('audio');
    const finish = (error?: Error) => { clearTimeout(timeout); audio.onloadedmetadata = null; audio.onerror = null; signal.removeEventListener('abort', aborted); audio.removeAttribute('src'); audio.load(); URL.revokeObjectURL(url); error ? reject(error) : resolve(); };
    const aborted = () => finish(new DOMException('已取消', 'AbortError'));
    const timeout = setTimeout(() => finish(new Error('无法读取音频时长，请使用含时长信息的短音频，或直接录音。')), 6000);
    audio.preload = 'metadata';
    audio.onloadedmetadata = () => {
      if (!Number.isFinite(audio.duration) || audio.duration <= 0) { finish(new Error('无法确认音频时长，请使用含时长信息的短音频，或直接录音。')); return; }
      finish(audio.duration > 60.5 ? new Error('每段音频最多 60 秒，请截短后再选择。') : undefined);
    };
    audio.onerror = () => finish(new Error('当前浏览器无法读取此音频，请转换为 MP3、M4A 或 WAV 后重试。'));
    signal.addEventListener('abort', aborted, { once: true });
    if (signal.aborted) aborted(); else audio.src = url;
  });
}

export default function AIVoiceInput({ disabled = false, onApply, destination = '管理员配置的语音服务', label = '语音输入' }: { disabled?: boolean; onApply: (text: string) => void; destination?: string; label?: string }) {
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<'idle' | 'requesting' | 'recording' | 'transcribing'>('idle');
  const [seconds, setSeconds] = useState(0);
  const [text, setText] = useState('');
  const [error, setError] = useState('');
  const stream = useRef<MediaStream | null>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const request = useRef<AbortController | null>(null);
  const generation = useRef(0);
  const timer = useRef<ReturnType<typeof setInterval>>();
  const input = useRef<HTMLInputElement>(null);
  const canRecord = typeof MediaRecorder !== 'undefined' && window.isSecureContext && Boolean(navigator.mediaDevices?.getUserMedia);
  const release = () => { clearInterval(timer.current); stream.current?.getTracks().forEach(track => track.stop()); stream.current = null; };
  const cancel = () => {
    generation.current += 1; request.current?.abort();
    if (recorder.current?.state === 'recording') recorder.current.stop();
    release(); setPhase('idle');
  };
  const close = () => { cancel(); setOpen(false); setText(''); setError(''); };
  useEffect(() => () => { generation.current += 1; request.current?.abort(); if (recorder.current?.state === 'recording') recorder.current.stop(); release(); }, []);
  useEffect(() => { if (disabled) close(); }, [disabled]);
  const transcribe = async (file: File, epoch = generation.current, uploaded = false) => {
    const invalid = validateAiAudio(file);
    if (invalid) { setError(invalid); setPhase('idle'); return; }
    setPhase('transcribing'); setError('');
    const controller = new AbortController(); request.current = controller;
    try {
      if (uploaded) await checkUploadedDuration(file, controller.signal);
      if (generation.current !== epoch || controller.signal.aborted) return;
      const response = await transcribeAiAudio(file, controller.signal);
      if (generation.current === epoch) { setText(response.text); if (!response.text.trim()) setError('未识别到文字，请重新录音或选择音频文件。'); }
    } catch (reason) { if (generation.current === epoch && !controller.signal.aborted) setError(reason instanceof Error ? reason.message : '转写失败，请重试。'); }
    finally { if (generation.current === epoch) setPhase('idle'); }
  };
  const record = async () => {
    if (!canRecord || phase !== 'idle') return;
    cancel(); const epoch = generation.current;
    setPhase('requesting'); setError(''); setText(''); setSeconds(0);
    try {
      const audio = await navigator.mediaDevices.getUserMedia({ audio: true });
      if (generation.current !== epoch) { audio.getTracks().forEach(track => track.stop()); return; }
      stream.current = audio;
      const mime = ['audio/webm;codecs=opus', 'audio/webm', 'audio/mp4', 'audio/ogg;codecs=opus'].find(type => MediaRecorder.isTypeSupported(type));
      const next = new MediaRecorder(audio, mime ? { mimeType: mime } : undefined);
      recorder.current = next;
      const chunks: Blob[] = []; let size = 0; let exceeded = false;
      next.ondataavailable = event => {
        if (generation.current !== epoch) return;
        size += event.data.size;
        if (size > AI_AUDIO_LIMIT) { exceeded = true; if (next.state === 'recording') next.stop(); release(); return; }
        chunks.push(event.data);
      };
      next.onerror = () => { if (generation.current === epoch) { cancel(); setError('录音失败，请检查麦克风后重试，或选择音频文件。'); } };
      next.onstop = () => {
        release();
        if (generation.current !== epoch) return;
        if (exceeded) { setPhase('idle'); setError('录音超过 10MB，请缩短后重试。'); return; }
        const type = next.mimeType || mime || 'audio/webm';
        const extension = type.includes('mp4') ? 'm4a' : type.includes('ogg') ? 'ogg' : 'webm';
        void transcribe(new File(chunks, `voice.${extension}`, { type }), epoch);
      };
      next.start(1000); setPhase('recording');
      const start = Date.now(); timer.current = setInterval(() => { const elapsed = Math.floor((Date.now() - start) / 1000); setSeconds(Math.min(elapsed, 60)); if (elapsed >= 60 && next.state === 'recording') { next.stop(); release(); } }, 250);
    } catch (reason) {
      release();
      if (generation.current === epoch) { setPhase('idle'); setError(reason instanceof DOMException && reason.name === 'NotAllowedError' ? '麦克风权限被拒绝，可在浏览器允许后重试，或选择音频文件。' : '无法使用麦克风，请检查设备，或选择音频文件。'); }
    }
  };
  return <>
    <Button size="small" variant="outlined" startIcon={<MicOutlinedIcon />} disabled={disabled} onClick={() => setOpen(true)}>{label}</Button>
    <Dialog open={open} onClose={close} fullWidth maxWidth="sm" aria-labelledby="ai-voice-title">
      <DialogTitle id="ai-voice-title">{label}</DialogTitle>
      <DialogContent><Stack spacing={1.5}>
        <Typography variant="body2" color="text.secondary">音频将发送到{destination}，识别后可修改文字再使用。每段最多 60 秒、10MB。</Typography>
        {!canRecord && <Alert severity="info">当前地址或浏览器无法录音。局域网访问请配置受信任的 HTTPS，仍可选择本地短音频。</Alert>}
        {error && <Alert severity="error" role="alert">{error}</Alert>}
        <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center' }}>
          {phase === 'recording' ? <Button color="error" variant="contained" onClick={() => { if (recorder.current?.state === 'recording') recorder.current.stop(); release(); }}>停止并转写（{seconds}/60 秒）</Button> : <Button variant="contained" disabled={!canRecord || phase !== 'idle'} onClick={() => void record()}>开始录音</Button>}
          <Button variant="outlined" disabled={phase !== 'idle'} onClick={() => input.current?.click()}>选择音频</Button>
          {(phase === 'requesting' || phase === 'transcribing') && <><CircularProgress size={18} /><Typography variant="body2">{phase === 'requesting' ? '等待麦克风授权…' : '正在转写…'}</Typography></>}
          {phase !== 'idle' && <Button onClick={cancel}>取消处理</Button>}
          <input ref={input} hidden type="file" accept="audio/*,.webm,.mp4,.m4a,.flac" onChange={event => { const file = event.target.files?.[0]; event.target.value = ''; if (file) { setText(''); void transcribe(file, generation.current, true); } }} />
        </Box>
        <TextField label="识别文字（可编辑）" multiline minRows={4} maxRows={10} value={text} onChange={event => setText(event.target.value)} disabled={phase !== 'idle'} fullWidth helperText="请复核姓名、批号、数量和方法名称，使用后仍需确认原业务表单。" />
      </Stack></DialogContent>
      <DialogActions><Button onClick={close}>关闭</Button><Button variant="contained" disabled={phase !== 'idle' || !text.trim()} onClick={() => { onApply(text.trim()); close(); }}>使用这段文字</Button></DialogActions>
    </Dialog>
  </>;
}
