import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Chip, CircularProgress, Collapse, Divider, Drawer, IconButton, MenuItem, Paper, Stack, TextField, Tooltip, Typography, useMediaQuery, useTheme, FormControlLabel, Checkbox } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import OpenInFullIcon from '@mui/icons-material/OpenInFull';
import CloseFullscreenIcon from '@mui/icons-material/CloseFullscreen';
import TuneIcon from '@mui/icons-material/Tune';
import SendOutlinedIcon from '@mui/icons-material/SendOutlined';
import StopCircleOutlinedIcon from '@mui/icons-material/StopCircleOutlined';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import { useLocation, useNavigate } from 'react-router-dom';
import { getAiCapabilities, streamAiChat, type AiCapabilities, type AiChatContext, type AiChatMessage, type AiEvent } from '../api/ai';
import { useUser } from '../UserContext';
import { useManagedNavigate, useManagement } from './ManagementContext';
import AIVoiceInput from './AIVoiceInput';
import { queueAiDraft, clearAiDraft, aiDraftFieldLabels, type AiEntryDraft } from '../utils/aiDraftBridge';
import { formatBeijingDateTimeInput } from '../utils/sampleWorkloadDateTime';

type Message = AiChatMessage & { id: string; state?: 'loading' | 'done' | 'error' | 'cancelled'; error?: string; drafts?: AiEntryDraft[] };
type ChatRequest = { messages: AiChatMessage[]; provider_id: string; model: string; context: AiChatContext; assistantId: string };
const makeId = () => `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
const toolNames: Record<string, string> = { query_records: '查询授权记录', workload_summary: '汇总工作量', entry_metadata: '核对填报可用信息', prepare_entry_draft: '整理原表单草稿', program_help: '查找程序说明' };
const draftFieldNames: Record<string, string> = { ...aiDraftFieldLabels, method_name: '检测方法', project_name: '研发项目', group_name: '实验室', lab_id: '实验室', actual_submitter: '实际送样人', amount_multiplier: '金额倍率', area_type_id: '区域', name: '样品名称', sample_name: '样品名称', batch_number: '批号', test_items: '检测项目', sample_count: '样品数量', remaining_required: '待补充' };
const asDraft = (data: unknown): AiEntryDraft | null => {
  if (!data || typeof data !== 'object') return null;
  const value = data as Partial<AiEntryDraft>;
  if (typeof value.id !== 'string' || typeof value.account_id !== 'number' || typeof value.route !== 'string' || !value.fields || !value.labels || !Array.isArray(value.issues)) return null;
  if (!['sample_info', 'rd_sample', 'workload', 'auxiliary'].includes(value.target || '')) return null;
  return value as AiEntryDraft;
};

export default function AIAssistant({ compact = false }: { compact?: boolean }) {
  const { user, token, hasPermission } = useUser();
  const location = useLocation();
  const navigate = useManagedNavigate();
  const rawNavigate = useNavigate();
  const management = useManagement();
  const theme = useTheme(); const mobile = useMediaQuery(theme.breakpoints.down('sm'));
  const [capabilities, setCapabilities] = useState<AiCapabilities | null>(null);
  const [capLoading, setCapLoading] = useState(false);
  const [open, setOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [settings, setSettings] = useState(false);
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [error, setError] = useState('');
  const [sending, setSending] = useState(false);
  const [tool, setTool] = useState('');
  const [selectedModel, setSelectedModel] = useState('');
  const [business, setBusiness] = useState(false);
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [failed, setFailed] = useState<ChatRequest | null>(null);
  const [transferred, setTransferred] = useState<string[]>([]);
  const controller = useRef<AbortController>();
  const generation = useRef(0);
  const capabilityGeneration = useRef(0);
  const currentModel = useRef('');
  const previousIdentity = useRef<{ authorization: string; token: string | null; routePerson: string }>({ authorization: '', token: null, routePerson: '' });
  const transferring = useRef<{ route: string; accountId: number } | null>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const isPublic = Boolean(user?.is_public_account || user?.is_rd_public_account || user?.is_analysis_public_account);
  const query = new URLSearchParams(location.search);
  const routePerson = isPublic ? ['subject_user_id', 'sender_user_id', 'detector_id', 'actual_submitter_id'].map(key => query.get(key) || '').join(':') : '';
  const authorization = JSON.stringify(user && [user.id, user.is_admin, user.is_active, [...(user.permissions || [])].sort(), [...(user.role_ids || [])].sort(), [...(user.role_identity_keys || [])].sort(), user.role_id, user.division_id, user.primary_division_id, user.group_id, [...(user.division_ids || [])].sort(), [...(user.group_ids || [])].sort(), [...(user.business_division_ids || [])].sort(), user.is_public_account, user.is_rd_public_account, user.is_analysis_public_account]);
  const owner = JSON.stringify([authorization, routePerson]);
  const canUse = Boolean(capabilities?.enabled && capabilities?.can_use);
  const selectedModelView = capabilities?.models.find(item => `${item.provider_id}\n${item.id}` === selectedModel);
  const canChat = canUse && Boolean(selectedModelView?.chat_ready);
  const canTools = canChat && Boolean(selectedModelView?.tools_ready);
  const canQuery = Boolean(capabilities?.capabilities.query || capabilities?.capabilities.report);
  const hasBusiness = canQuery || Boolean(capabilities?.capabilities.draft);
  const validDates = Boolean(dateFrom && dateTo && dateFrom <= dateTo);
  const businessStatus = capLoading ? '读取业务辅助状态…' : !canUse ? '业务辅助尚未启用' : !hasBusiness ? '当前账号未开放业务辅助' : !canChat ? '当前模型待 hi 测试' : !canTools ? '当前模型待工具测试' : !business ? '尚未允许业务辅助' : canQuery && !validDates ? '业务辅助已允许 · 查询待选日期' : canQuery ? '业务辅助已允许 · 查询范围已确认' : '已允许填报辅助';
  const availableAbilities = [capabilities?.capabilities.help && canChat && '程序操作说明', capabilities?.capabilities.query && canTools && '授权记录查询', capabilities?.capabilities.report && canTools && '工作量总结', capabilities?.capabilities.draft && canTools && '填报草稿与信息核对'].filter(Boolean);
  const introduction = !canChat ? '请先选择已通过 hi 测试的模型。' : !canTools ? '当前模型可普通问答，业务查询和填报辅助需管理员完成工具测试。' : availableAbilities.length ? `当前可协助：${availableAbilities.join('、')}。${hasBusiness ? '使用业务辅助前需你明确允许。' : ''}` : '当前可普通问答，业务能力由管理员按账号权限开放。';
  const stop = () => { generation.current += 1; controller.current?.abort(); setSending(false); setTool(''); setMessages(value => value.map(item => item.state === 'loading' ? { ...item, state: 'cancelled' } : item)); };
  const clearConversation = () => { stop(); setMessages([]); setError(''); setFailed(null); setTransferred([]); if (user) clearAiDraft(user.id); };
  const clear = () => { clearConversation(); setInput(''); setBusiness(false); setDateFrom(''); setDateTo(''); };
  const changeModel = (value: string) => { clear(); currentModel.current = value; setSelectedModel(value); };
  const changeBusiness = (allowed: boolean) => { clearConversation(); setBusiness(allowed); };
  const changeDateRange = (from: string, to: string) => { clearConversation(); setDateFrom(from); setDateTo(to); };
  const selectRecentDays = (days: 7 | 30) => {
    const to = formatBeijingDateTimeInput().slice(0, 10);
    const from = new Date(`${to}T00:00:00Z`);
    from.setUTCDate(from.getUTCDate() - days + 1);
    changeDateRange(from.toISOString().slice(0, 10), to);
  };
  const showContext = () => { setSettings(true); content.current?.scrollTo({ top: 0 }); };
  const refresh = async () => {
    const requestGeneration = ++capabilityGeneration.current; setCapLoading(true);
    try {
      const value = await getAiCapabilities();
      if (requestGeneration !== capabilityGeneration.current) return;
      const previousModel = capabilities?.models.find(item => `${item.provider_id}\n${item.id}` === currentModel.current);
      const refreshedModel = value.models.find(item => `${item.provider_id}\n${item.id}` === currentModel.current);
      const downgraded = (['help', 'query', 'draft', 'report', 'voice'] as const).some(key => capabilities?.capabilities[key] && !value.capabilities[key])
        || (['speech_enabled', 'can_sample', 'can_work', 'can_aux', 'can_report'] as const).some(key => capabilities?.[key] && !value[key])
        || (previousModel?.chat_ready && !refreshedModel?.chat_ready) || (previousModel?.tools_ready && !refreshedModel?.tools_ready);
      setCapabilities(value); setError('');
      const nextModel = value.models.some(item => `${item.provider_id}\n${item.id}` === currentModel.current) ? currentModel.current : `${value.default_provider}\n${value.default_model}`;
      if (currentModel.current !== nextModel) changeModel(nextModel);
      else if (!value.can_use || !value.enabled || downgraded) clear();
    } catch (reason) { if (requestGeneration === capabilityGeneration.current) { clear(); setCapabilities(null); setError(reason instanceof Error ? reason.message : '助手配置读取失败。'); } }
    finally { if (requestGeneration === capabilityGeneration.current) setCapLoading(false); }
  };
  useLayoutEffect(() => {
    const previous = previousIdentity.current;
    const ownTransfer = transferring.current?.accountId === user?.id && transferring.current?.route === `${location.pathname}${location.search}`;
    if (previous.authorization !== authorization || previous.token !== token || (previous.routePerson !== routePerson && !ownTransfer)) clearAiDraft();
    previousIdentity.current = { authorization, token, routePerson };
    generation.current += 1; capabilityGeneration.current += 1; controller.current?.abort();
    setCapabilities(null); setMessages([]); setInput(''); setError(''); setSending(false); setBusiness(false); setDateFrom(''); setDateTo(''); setFailed(null); setTransferred([]); setOpen(false); setTool('');
    if (user && (user.is_admin || hasPermission('ai:use'))) void refresh();
    return () => { generation.current += 1; capabilityGeneration.current += 1; controller.current?.abort(); };
  }, [owner, token]);
  useEffect(() => { transferring.current = null; }, [location.pathname, location.search]);
  useEffect(() => () => clearAiDraft(), []);
  useEffect(() => {
    const changed = () => { clear(); setCapabilities(null); void refresh(); };
    window.addEventListener('labflow-ai-config-changed', changed);
    return () => window.removeEventListener('labflow-ai-config-changed', changed);
  }, [owner]);
  useEffect(() => {
    const personChanged = () => { clear(); };
    window.addEventListener('labflow-ai-person-changed', personChanged);
    return () => window.removeEventListener('labflow-ai-person-changed', personChanged);
  }, [owner]);
  useEffect(() => { if (open && !settings) bottom.current?.scrollIntoView({ block: 'nearest' }); }, [messages, tool, open, settings]);
  const close = () => { stop(); setOpen(false); };
  const run = async (request: ChatRequest) => {
    if (!token || !canChat || sending) return;
    controller.current?.abort(); const abort = new AbortController(); controller.current = abort;
    const current = ++generation.current;
    setSending(true); setError(''); setTool(''); setFailed(null);
    setMessages(value => value.map(item => item.id === request.assistantId ? { ...item, content: '', state: 'loading', error: '', drafts: [] } : item));
    try {
      await streamAiChat({ token, messages: request.messages, provider_id: request.provider_id, model: request.model, context: request.context, signal: abort.signal, onEvent: (event: string, data: AiEvent) => {
        if (current !== generation.current) return;
        if (event === 'delta') setMessages(value => value.map(item => item.id === request.assistantId ? { ...item, content: item.content + (data.text || '') } : item));
        if (event === 'tool') setTool(data.status === 'done' ? '' : `${toolNames[data.name || ''] || '正在处理授权信息'}…`);
        if (event === 'draft') {
          const draft = asDraft(data.data) || asDraft(data);
          if (draft && draft.account_id === user?.id) setMessages(value => value.map(item => item.id === request.assistantId ? { ...item, drafts: [...(item.drafts || []).filter(existing => existing.id !== draft.id), draft] } : item));
        }
        if (event === 'done') setMessages(value => value.map(item => item.id === request.assistantId ? { ...item, state: 'done', content: item.content || data.message || '已完成。' } : item));
      } });
    } catch (reason) {
      if (current !== generation.current || abort.signal.aborted) return;
      const message = reason instanceof Error ? reason.message : '生成失败，请重试。';
      setMessages(value => value.map(item => item.id === request.assistantId ? { ...item, state: 'error', error: message } : item)); setFailed(request);
    } finally { if (current === generation.current) { setSending(false); setTool(''); } }
  };
  const send = () => {
    const text = input.trim();
    if (!text || sending || !canChat || capLoading) return;
    if (business && canQuery && ((Boolean(dateFrom) !== Boolean(dateTo)) || dateFrom > dateTo)) { showContext(); setError('查询日期请完整填写起止日期，并确保开始日期不晚于结束日期。填报草稿及主数据核对无需日期，可清空起止日期后继续。'); return; }
    const model = capabilities?.models.find(item => `${item.provider_id}\n${item.id}` === selectedModel);
    if (!model) { setSettings(true); setError('请先选择管理员已配置的可用模型。'); return; }
    const assistantId = makeId();
    const history = [...messages.filter(item => item.state !== 'error' && item.state !== 'cancelled' && item.content.trim()).map(({ role, content }) => ({ role, content })), { role: 'user' as const, content: text }].slice(-20);
    setMessages(value => [...value, { id: makeId(), role: 'user', content: text }, { id: assistantId, role: 'assistant', content: '', state: 'loading', drafts: [] }]); setInput(''); setSettings(false);
    void run({ messages: history, provider_id: model.provider_id, model: model.id, context: { include_business: business && canTools, ...(business && canTools && dateFrom && dateTo ? { date_from: dateFrom, date_to: dateTo } : {}) }, assistantId });
  };
  const transfer = (draft: AiEntryDraft) => {
    if (!user || sending || draft.account_id !== user.id) return;
    const apply = () => {
      try { queueAiDraft(draft, user.id); transferring.current = { route: draft.route, accountId: user.id }; setTransferred(value => [...value, draft.id]); close(); rawNavigate(draft.route); }
      catch (reason) { setError(reason instanceof Error ? reason.message : '草稿无法带入，请重新生成。'); }
    };
    if (management) management.guard(apply); else apply();
  };
  if (!user || (!user.is_admin && !hasPermission('ai:use')) || (capabilities && !capabilities.can_use && !capabilities.can_manage)) return null;
  const modelLabel = capabilities?.models.find(item => `${item.provider_id}\n${item.id}` === selectedModel);
  return <>
    <Tooltip title={capabilities?.name || 'AI 助手'}><Button color="inherit" size="small" aria-label="打开 AI 助手" onClick={() => { setOpen(true); void refresh(); }} sx={{ minWidth: compact ? 40 : 'auto', px: compact ? 0.5 : 1, gap: 0.5, whiteSpace: 'nowrap' }}><Box component="img" src="/assets/assistant.svg" alt="" sx={{ width: 28, height: 28 }} />{!compact && <span>{capabilities?.name || 'AI 助手'}</span>}</Button></Tooltip>
    <Drawer anchor="right" open={open} onClose={close} sx={{ zIndex: theme.zIndex.drawer + 2 }} ModalProps={{ keepMounted: false }} PaperProps={{ sx: { width: mobile ? '100%' : expanded ? 'min(820px, 88vw)' : 440, maxWidth: '100%', display: 'flex', flexDirection: 'column', overflow: 'hidden' }, role: 'dialog', 'aria-modal': true, 'aria-label': capabilities?.name || 'AI 助手' }}>
      <Box sx={{ px: 2, py: 1.25, display: 'flex', alignItems: 'center', gap: 1, flexShrink: 0, borderBottom: 1, borderColor: 'divider' }}><Box component="img" src="/assets/assistant.svg" alt="" sx={{ width: 36, height: 36 }} /><Box sx={{ flex: 1, minWidth: 0 }}><Typography variant="subtitle1" fontWeight={700}>{capabilities?.name || 'AI 助手'}</Typography><Typography variant="caption" color="text.secondary">{canUse ? modelLabel?.provider_name || '按账号权限提供帮助' : '由管理员配置后使用'}</Typography></Box>{!mobile && <Tooltip title={expanded ? '恢复宽度' : '放大'}><IconButton size="small" aria-label={expanded ? '恢复助手宽度' : '放大助手'} onClick={() => setExpanded(value => !value)}>{expanded ? <CloseFullscreenIcon /> : <OpenInFullIcon />}</IconButton></Tooltip>}<IconButton aria-label="关闭 AI 助手" onClick={close}><CloseIcon /></IconButton></Box>
      <Box sx={{ px: 2, py: 0.75, display: 'flex', gap: 0.5, alignItems: 'center', flexWrap: 'wrap', flexShrink: 0, borderBottom: 1, borderColor: 'divider' }}><Button size="small" startIcon={<TuneIcon />} aria-expanded={settings} onClick={() => settings ? setSettings(false) : showContext()}>本次上下文</Button><Button size="small" startIcon={<DeleteOutlineIcon />} disabled={!messages.length && !input && !business && !dateFrom && !dateTo} onClick={clear}>新建会话</Button>{capabilities?.can_manage && <Button size="small" sx={{ ml: 'auto' }} onClick={() => { close(); navigate('/manage/ai'); }}>配置</Button>}</Box>
      <Box sx={{ px: 2, py: 0.75, display: 'flex', gap: 1, alignItems: 'center', flexShrink: 0, borderBottom: 1, borderColor: 'divider' }}>
        <Box sx={{ flex: 1, minWidth: 0 }}><Typography variant="caption" role="status" display="block">{businessStatus}</Typography>{business && canTools && canQuery && validDates && <Typography variant="caption" color="text.secondary" display="block">{dateFrom} 至 {dateTo} · 按账号权限</Typography>}</Box>
        {hasBusiness && <Button size="small" disabled={!canUse || capLoading} onClick={showContext} sx={{ flexShrink: 0 }}>业务授权与范围</Button>}
      </Box>
      <Box ref={content} sx={{ flex: 1, minHeight: 0, overflowY: 'auto', px: 2, py: 1.5 }}>
        <Collapse in={settings}><Stack spacing={1.25} sx={{ pb: 1.5, mb: 1.5, borderBottom: 1, borderColor: 'divider' }}>
          <TextField select size="small" label="本次模型" value={capabilities?.models.some(item => `${item.provider_id}\n${item.id}` === selectedModel) ? selectedModel : ''} disabled={sending} onChange={event => changeModel(event.target.value)}><MenuItem value="">请选择模型</MenuItem>{capabilities?.models.map(item => <MenuItem key={`${item.provider_id}:${item.id}`} value={`${item.provider_id}\n${item.id}`}>{item.provider_name} · {item.name || item.id}{!item.chat_ready ? '（待 hi 测试）' : !item.tools_ready ? '（仅问答）' : ''}</MenuItem>)}</TextField>
          {hasBusiness && <FormControlLabel sx={{ alignItems: 'flex-start', m: 0 }} control={<Checkbox size="small" checked={business && canTools} disabled={sending || !canTools} onChange={event => changeBusiness(event.target.checked)} />} label={<Typography variant="body2" sx={{ pt: 0.5 }}>允许本次会话使用已开放的业务辅助（记录查询、填报信息核对或草稿整理），并将必要信息发送到所选模型服务</Typography>} />}
          {canQuery && <><Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}><Button size="small" variant="outlined" disabled={sending} onClick={() => selectRecentDays(7)}>近7天（含今天）</Button><Button size="small" variant="outlined" disabled={sending} onClick={() => selectRecentDays(30)}>近30天</Button></Box><Box sx={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 1 }}><TextField size="small" type="date" label="开始日期" InputLabelProps={{ shrink: true }} value={dateFrom} disabled={sending} onChange={event => changeDateRange(event.target.value, dateTo)} inputProps={{ max: dateTo || undefined }} sx={{ minWidth: 0 }} /><TextField size="small" type="date" label="结束日期" InputLabelProps={{ shrink: true }} value={dateTo} disabled={sending} onChange={event => changeDateRange(dateFrom, event.target.value)} inputProps={{ min: dateFrom || undefined }} sx={{ minWidth: 0 }} /></Box><Typography variant="caption" color="text.secondary">日期按北京时间，选择日期不会自动授权。修改授权或日期会清除旧回答和草稿，保留未发送的提问。</Typography></>}
          <Typography variant="caption" color="text.secondary">默认不附带当前页面或数据库内容。{canQuery && '查询记录和总结工作量需确认日期。'}{capabilities?.capabilities.draft && '填报草稿及所需主数据核对无需日期；请先勾选本次业务授权，草稿带入原表单后复核提交。'}</Typography>
        </Stack></Collapse>
        {capLoading && <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}><CircularProgress size={18} /><Typography variant="body2">读取助手权限与配置…</Typography></Box>}
        {error && <Alert severity="error" onClose={() => setError('')} sx={{ mb: 1 }}>{error}</Alert>}
        {!capLoading && !canUse && <Alert severity="info">{capabilities?.enabled ? '当前账号尚未获得 AI 助手使用资格，请联系管理员。' : 'AI 助手尚未启用，请由管理员在系统配置中启用服务并分配使用权限。'}</Alert>}
        {!capLoading && canUse && !canChat && <Alert severity="warning" sx={{ mb: 1 }}>所选模型尚未通过当前配置的 hi 测试，请由管理员测试通过后使用，或选择已通过测试的模型。</Alert>}
        {!capLoading && canChat && !canTools && <Alert severity="info" sx={{ mb: 1 }}>此模型可普通问答。查询业务和生成草稿需管理员先完成此模型的工具测试。</Alert>}
        {canUse && !messages.length && <Stack spacing={1.5} sx={{ py: 2 }}><Typography variant="subtitle1" fontWeight={700}>需要我帮你做什么？</Typography><Typography variant="body2" color="text.secondary">{introduction}</Typography><Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>{[['如何操作', '请介绍我的账号可用的程序功能和常用操作。', capabilities?.capabilities.help && canChat], ['辅助填报送样信息', '请帮我整理送样信息，先询问需要填写的内容，生成草稿后由我带入原表单确认。', capabilities?.capabilities.draft && capabilities.can_sample && canTools], ['查询我的记录', '请按我确认的日期范围查询我有权查看的业务记录。', capabilities?.capabilities.query && canTools], ['总结我的工作量', '请按我确认的日期范围总结工作量，将检测与辅助工作分别列出。', capabilities?.capabilities.report && canTools]].filter(item => item[2]).map(([label, prompt]) => <Button size="small" variant="outlined" key={String(label)} onClick={() => { setInput(String(prompt)); if (String(label) !== '如何操作') showContext(); }}>{String(label)}</Button>)}</Box></Stack>}
        <Stack spacing={1.5} aria-live="polite" aria-relevant="additions text">{messages.map((message, index) => <Box key={message.id} sx={{ alignSelf: message.role === 'user' ? 'flex-end' : 'stretch', maxWidth: '100%', minWidth: 0 }}>
          <Typography variant="caption" color="text.secondary">{message.role === 'user' ? '我' : capabilities?.name || 'AI 助手'}</Typography>
          <Paper variant="outlined" sx={{ mt: 0.5, p: 1.25, bgcolor: message.role === 'user' ? 'action.hover' : 'background.paper', overflowWrap: 'anywhere' }}><Typography variant="body2" sx={{ whiteSpace: 'pre-wrap' }}>{message.content || (message.state === 'loading' ? '正在思考…' : message.state === 'cancelled' ? '已停止生成。' : '')}</Typography>
            {message.state === 'cancelled' && message.content && <Typography variant="caption" color="text.secondary">已停止，以上回答尚未完成。</Typography>}
            {message.error && <Alert severity="error" sx={{ mt: 1 }}>{message.error}</Alert>}
            {message.state === 'error' && index === messages.length - 1 && failed && <Button size="small" sx={{ mt: 0.5 }} disabled={sending || !canChat} onClick={() => void run(failed)}>重试本次回答</Button>}
            {message.drafts?.map(draft => <Paper variant="outlined" key={draft.id} sx={{ mt: 1.25, p: 1.25, bgcolor: 'action.hover' }}><Typography variant="subtitle2">{draft.title || '待复核草稿'}</Typography><Stack spacing={0.5} sx={{ my: 1 }}>{Object.entries(draft.labels).map(([key, value]) => <Typography variant="body2" key={key} sx={{ overflowWrap: 'anywhere' }}>{draftFieldNames[key] || key}：{String(value)}</Typography>)}</Stack>{draft.issues.length > 0 && <Alert severity="warning" sx={{ mb: 1 }}>{draft.issues.join('；')}</Alert>}<Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 1 }}>带入后核对原表单，仍需按原提交按钮完成录入。</Typography><Button size="small" variant="contained" disabled={sending || transferred.includes(draft.id)} onClick={() => transfer(draft)}>{transferred.includes(draft.id) ? '已带入原表单' : '复核并带入原表单'}</Button></Paper>)}
          </Paper>
        </Box>)}</Stack>
        {tool && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>{tool}</Typography>}<div ref={bottom} />
      </Box>
      <Divider />
      <Stack spacing={1} sx={{ p: 1.5, flexShrink: 0 }}>
        <TextField size="small" multiline minRows={2} maxRows={mobile ? 3 : 6} placeholder="输入问题或需要整理的信息…" value={input} disabled={!canChat || capLoading} inputProps={{ maxLength: 6000, 'aria-label': '给 AI 助手的消息' }} onChange={event => setInput(event.target.value)} onKeyDown={event => { if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); send(); } }} />
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
          <AIVoiceInput disabled={!open || !canUse || !capabilities?.speech_enabled || sending || capLoading} destination={capabilities?.speech_destination || '管理员配置的语音服务'} onApply={text => setInput(value => `${value}${value ? '\n' : ''}${text}`.slice(0, 6000))} />
          <Typography variant="caption" color="text.secondary" sx={{ flex: 1 }}>Ctrl + Enter 发送</Typography>
          {sending ? <Button variant="outlined" color="error" startIcon={<StopCircleOutlinedIcon />} onClick={stop}>停止</Button> : <Button variant="contained" startIcon={<SendOutlinedIcon />} disabled={!canChat || !input.trim() || capLoading} onClick={send}>发送</Button>}
        </Box>
      </Stack>
    </Drawer>
  </>;
}
