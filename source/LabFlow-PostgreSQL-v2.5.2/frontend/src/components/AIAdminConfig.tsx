import { useEffect, useRef, useState } from 'react';
import { Accordion, AccordionDetails, AccordionSummary, Alert, Autocomplete, Box, Button, Checkbox, Chip, CircularProgress, Divider, FormControlLabel, List, ListItemButton, ListItemText, MenuItem, Paper, Stack, Switch, Tab, Tabs, TextField, Typography } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import { discoverAiModels, getAiConfig, saveAiConfig, testAiModel, type AiConfig, type AiModel, type AiProvider, type AiTest, type AiTestKind } from '../api/ai';
import { useUser } from '../UserContext';
import { useManagement, useManagedNavigate, useUnsavedManagement } from './ManagementContext';
import AIVoiceInput from './AIVoiceInput';

const DRAFT_KEY = 'ai-service-config';
const testNames: Record<AiTestKind, string> = { connectivity: '检查地址', hi: '发送 hi', stream: '流式测试', tools: '工具测试' };
const abilityNames = { help: '程序操作问答', query: '授权业务查询', draft: '原表单草稿辅助', report: '工作量总结', voice: '语音转文字', notification_read: '本人通知已读', submit: '预览后确认提交' };
const withoutSecrets = (config: AiConfig): AiConfig => ({ ...config, providers: config.providers.map(({ secret: _, ...item }) => item), speech: (({ secret: _, ...item }) => item)(config.speech) });
const errorText = (reason: unknown) => reason instanceof Error ? reason.message : '请求失败，请重试。';

export default function AIAdminConfig() {
  const { user } = useUser();
  const management = useManagement();
  const navigate = useManagedNavigate();
  const [config, setConfig] = useState<AiConfig | null>(null);
  const [saved, setSaved] = useState<AiConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [dismissedTests, setDismissedTests] = useState<AiTest[]>([]);
  const [tab, setTab] = useState(0);
  const [selected, setSelected] = useState('');
  const [testModel, setTestModel] = useState('');
  const [busy, setBusy] = useState('');
  const [discovered, setDiscovered] = useState<AiModel[]>([]);
  const [chosen, setChosen] = useState<AiModel[]>([]);
  const [discoveryNote, setDiscoveryNote] = useState('');
  const [modelId, setModelId] = useState('');
  const [modelName, setModelName] = useState('');
  const [speechText, setSpeechText] = useState('');
  const controller = useRef<AbortController>();
  const epoch = useRef(0);
  const provider = config?.providers.find(item => item.id === selected);
  const stopProbes = () => { epoch.current += 1; controller.current?.abort(); setBusy(''); };

  useEffect(() => {
    if (!user?.is_admin) { setLoading(false); return; }
    let active = true;
    void getAiConfig().then(value => {
      if (!active) return;
      const draft = management?.readDraft<AiConfig>(DRAFT_KEY);
      setSaved(value); setConfig(draft || value); setDirty(Boolean(draft));
      const first = (draft || value).providers[0]; setSelected(first?.id || ''); setTestModel(first?.default_model || '');
      if (draft) setNotice('已恢复未保存配置。密钥输入不会保留，请重新填写尚未保存的密钥。');
    }).catch(reason => { if (active) setError(errorText(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; controller.current?.abort(); epoch.current += 1; };
  }, [user?.id, user?.is_admin]);
  useEffect(() => { if (dirty && config) management?.writeDraft(DRAFT_KEY, withoutSecrets(config)); else management?.clearDraft(DRAFT_KEY); }, [dirty, config, management?.writeDraft, management?.clearDraft]);
  const change = (next: AiConfig) => { if (saving) return; stopProbes(); setConfig({ ...next, tests: [] }); setDirty(true); setNotice(''); };
  const editProvider = (patch: Partial<AiProvider>) => { if (config && provider) change({ ...config, providers: config.providers.map(item => item.id === provider.id ? { ...item, ...patch } : item) }); };
  const selectProvider = (id: string) => {
    stopProbes(); setSelected(id); setTestModel(config?.providers.find(item => item.id === id)?.default_model || '');
    setDiscovered([]); setChosen([]); setDiscoveryNote(''); setModelId(''); setModelName(''); setError('');
  };
  const save = async () => {
    if (!config || saving) return false;
    setError(''); setNotice('');
    if (!config.name.trim()) { setError('请填写助手名称。'); return false; }
    if (config.enabled && !config.providers.some(item => item.id === config.default_provider && item.models.some(model => model.id === item.default_model))) { setError('启用前请选择默认服务及其默认模型。'); return false; }
    for (const item of config.providers) {
      if (!item.name.trim() || !item.url.trim()) { setError('请完整填写每个模型服务的名称和接口地址。'); return false; }
      const old = saved?.providers.find(row => row.id === item.id);
      if (old?.has_secret && (old.url !== item.url || old.protocol !== item.protocol) && !item.secret?.trim() && !item.no_key) { setError(`“${item.name}”已更改接口地址或协议，请重新填写密钥。`); return false; }
    }
    if (config.speech.enabled && (!config.speech.url.trim() || !config.speech.model.trim())) { setError('启用语音前请填写独立语音服务地址及识别模型。'); return false; }
    if (saved?.speech.has_secret && saved.speech.url !== config.speech.url && !config.speech.secret?.trim() && !config.speech.no_key) { setError('语音服务地址已更改，请重新填写语音密钥。'); return false; }
    stopProbes(); setSaving(true);
    try {
      const value = await saveAiConfig(config); setSaved(value); setConfig(value); setDirty(false); management?.clearDraft(DRAFT_KEY);
      setNotice('配置已保存。服务或模型变更后请重新测试。'); window.dispatchEvent(new Event('labflow-ai-config-changed')); return true;
    } catch (reason) { setError(`${errorText(reason)} 编辑内容仍保留；若配置已由其他管理员更新，请先记录当前修改再重新进入。`); return false; }
    finally { setSaving(false); }
  };
  useUnsavedManagement(DRAFT_KEY, { dirty, label: 'AI 助手配置', save, discard: () => { stopProbes(); setConfig(saved); setDirty(false); } });
  const probe = async (kind: AiTestKind | 'models') => {
    if (!provider || !config || dirty || busy) return;
    stopProbes(); const current = epoch.current; const abort = new AbortController(); controller.current = abort;
    setBusy(kind); setError(''); setNotice('');
    try {
      if (kind === 'models') {
        const result = await discoverAiModels(provider.id, abort.signal);
        if (current !== epoch.current) return;
        const unique = [...new Map(result.models.map(item => [item.id, item])).values()];
        setDiscovered(unique); setChosen([]);
        setDiscoveryNote(`${provider.name} · ${new Date().toLocaleString()} · 获取 ${unique.length} 个模型${result.truncated ? '（列表未取完，可手工添加其他模型）' : ''}。已配置模型保持不变。`);
      } else {
        const result = await testAiModel(provider.id, testModel, kind, abort.signal);
        if (current !== epoch.current) return;
        setConfig(value => value ? { ...value, tests: [...value.tests.filter(item => !(item.provider_id === result.provider_id && item.model === result.model && item.kind === result.kind)), result] } : value);
      }
    } catch (reason) { if (current === epoch.current && !abort.signal.aborted) setError(errorText(reason)); }
    finally { if (current === epoch.current) setBusy(''); }
  };
  const addModels = (models: AiModel[]) => {
    if (!provider) return;
    const merged = [...provider.models, ...models.filter(item => !provider.models.some(existing => existing.id === item.id))];
    editProvider({ models: merged, default_model: provider.default_model || merged[0]?.id || '' }); setChosen([]); setModelId(''); setModelName('');
  };
  if (!user?.is_admin) return <Alert severity="warning">仅系统管理员可配置 AI 模型服务。</Alert>;
  if (loading) return <Box sx={{ p: 3 }}><CircularProgress size={24} aria-label="读取 AI 配置" /></Box>;
  if (!config) return <Alert severity="error">{error || 'AI 配置读取失败，请重新进入。'}</Alert>;
  const results = config.tests.filter(item => item.provider_id === selected && (item.kind === 'connectivity' || item.model === testModel));
  const visibleResults = results.filter(result => !dismissedTests.includes(result));
  const renderResult = (result: AiTest) => <Alert key={`${result.kind}:${result.model}`} severity={result.passed ? 'success' : result.reachable ? 'warning' : 'error'} onClose={() => setDismissedTests(previous => [...previous, result])} closeText="关闭测试提示" sx={{ overflowWrap: 'anywhere' }}><strong>{testNames[result.kind]}：{result.passed ? '通过' : result.reachable ? '地址可达，功能未通过' : '未通过'}</strong><br />{result.message}<Typography component="div" variant="caption">{result.model || '地址检查'} · {result.latency_ms} ms · {result.checked_at}</Typography></Alert>;
  return <Stack spacing={1.5} sx={{ minWidth: 0 }}>
    <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}><Box><Typography variant="subtitle1" fontWeight={700}>AI 助手</Typography><Typography variant="body2" color="text.secondary">统一管理模型与语音服务，使用范围沿用账号实际权限。</Typography></Box><Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}><Button variant="outlined" disabled={saving} onClick={() => navigate('/manage/roles?permission=ai%3Ause')}>分配使用权限</Button><Button variant="contained" disabled={!dirty || saving} onClick={() => void save()}>{saving ? '保存中…' : '保存配置'}</Button></Box></Box>
    {error && <Alert severity="error" onClose={() => setError('')}>{error}</Alert>}
    {notice && <Alert severity="success" onClose={() => setNotice('')}>{notice}</Alert>}
    {dirty && <Alert severity="info">有未保存的修改。模型发现和连通性测试使用已保存配置，请先保存。</Alert>}
    <Paper variant="outlined" sx={{ minWidth: 0 }}>
      <Tabs value={tab} onChange={(_, value: number) => { stopProbes(); setTab(value); }} variant="scrollable" scrollButtons="auto" aria-label="AI 配置分类"><Tab label="模型服务" /><Tab label="助手与权限" /><Tab label="语音输入" /></Tabs>
      <Box component="fieldset" disabled={saving} sx={{ p: { xs: 1.5, sm: 2 }, border: 0, m: 0, minWidth: 0 }}>
        {tab === 0 && <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: '220px minmax(0,1fr)' }, gap: 2 }}>
          <Box sx={{ minWidth: 0 }}><List dense disablePadding aria-label="模型服务列表">{config.providers.map(item => <ListItemButton key={item.id} selected={item.id === selected} onClick={() => selectProvider(item.id)} sx={{ borderRadius: 1, mb: 0.5 }}><ListItemText primary={item.name || '未命名服务'} secondary={`${item.protocol} · ${item.default_model || '未选默认模型'}`} primaryTypographyProps={{ noWrap: true }} secondaryTypographyProps={{ sx: { overflowWrap: 'anywhere' } }} />{item.id === config.default_provider && <Chip label="默认" size="small" />}</ListItemButton>)}</List><Button fullWidth variant="outlined" startIcon={<AddIcon />} sx={{ mt: 1 }} onClick={() => {
            const id = `provider-${globalThis.crypto?.randomUUID?.() || `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`}`; change({ ...config, providers: [...config.providers, { id, name: '新模型服务', protocol: 'openai', url: '', has_secret: false, no_key: false, models: [], default_model: '' }], default_provider: config.default_provider || id }); selectProvider(id);
          }}>添加服务</Button></Box>
          {!provider ? <Alert severity="info">添加模型服务后，配置接口地址并获取或手工添加模型。</Alert> : <Stack spacing={1.5} sx={{ minWidth: 0 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}><Typography variant="subtitle2" sx={{ flex: 1 }}>{provider.name}</Typography><Button size="small" variant="outlined" disabled={config.default_provider === provider.id} onClick={() => change({ ...config, default_provider: provider.id })}>设为默认服务</Button><Button size="small" color="error" startIcon={<DeleteOutlineIcon />} onClick={() => {
              const rest = config.providers.filter(item => item.id !== provider.id); change({ ...config, providers: rest, default_provider: config.default_provider === provider.id ? rest[0]?.id || '' : config.default_provider }); selectProvider(rest[0]?.id || '');
            }}>移除</Button></Box>
            <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '1fr 180px' }, gap: 1.5 }}><TextField size="small" label="服务名称" value={provider.name} onChange={event => editProvider({ name: event.target.value })} /><TextField select size="small" label="协议" value={provider.protocol} onChange={event => editProvider({ protocol: event.target.value as AiProvider['protocol'] })}><MenuItem value="openai">OpenAI 兼容</MenuItem><MenuItem value="anthropic">Anthropic</MenuItem><MenuItem value="ollama">Ollama 原生</MenuItem></TextField></Box>
            <TextField size="small" label="接口基础地址" placeholder={provider.protocol === 'ollama' ? 'http://主机:11434' : 'https://服务地址/v1'} value={provider.url} onChange={event => editProvider({ url: event.target.value })} helperText={provider.protocol === 'ollama' ? '服务端调用 /api/tags 和 /api/chat，地址只由管理员配置。' : `服务端调用 ${provider.protocol === 'anthropic' ? 'models、messages' : 'models、chat/completions'}；末尾可包含 /v1，勿重复填写。`} />
            <TextField size="small" label={`密钥${provider.has_secret ? '（已配置，留空保留）' : '（未配置）'}`} type="password" autoComplete="new-password" value={provider.secret || ''} onChange={event => editProvider({ secret: event.target.value, clear_secret: false })} disabled={provider.no_key} helperText="密钥只写入服务端；更改地址或协议需重新填写，不会显示已保存的密钥。" />
            <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap' }}><FormControlLabel control={<Checkbox size="small" checked={provider.no_key} onChange={event => editProvider({ no_key: event.target.checked, ...(event.target.checked ? { secret: '', clear_secret: true } : {}) })} />} label="此服务无需密钥" /><FormControlLabel control={<Checkbox size="small" checked={Boolean(provider.clear_secret) && !provider.no_key} disabled={provider.no_key} onChange={event => editProvider({ clear_secret: event.target.checked, secret: '' })} />} label="清除已保存密钥" /></Box>
            <Divider />
            <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap' }}><Typography variant="subtitle2" sx={{ flex: 1 }}>可用模型（{provider.models.length}）</Typography><Button size="small" variant="outlined" disabled={dirty || Boolean(busy) || saving} onClick={() => void probe('models')}>{busy === 'models' ? '获取中…' : '获取上游模型'}</Button></Box>
            {discoveryNote && <Alert severity="info">{discoveryNote}</Alert>}
            {discovered.length > 0 && <><Autocomplete multiple size="small" options={discovered} value={chosen} onChange={(_, value) => setChosen(value)} getOptionLabel={item => item.name === item.id ? item.id : `${item.name} · ${item.id}`} isOptionEqualToValue={(a, b) => a.id === b.id} renderInput={params => <TextField {...params} label="搜索并选择上游模型" />} /><Button size="small" variant="outlined" disabled={!chosen.length} sx={{ alignSelf: 'flex-start' }} onClick={() => addModels(chosen)}>添加选中的 {chosen.length} 个模型</Button></>}
            <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}><TextField size="small" label="手工模型 ID" value={modelId} onChange={event => setModelId(event.target.value)} sx={{ flex: '1 1 150px' }} /><TextField size="small" label="显示名称（可选）" value={modelName} onChange={event => setModelName(event.target.value)} sx={{ flex: '1 1 150px' }} /><Button variant="outlined" disabled={!modelId.trim() || provider.models.some(item => item.id === modelId.trim())} onClick={() => addModels([{ id: modelId.trim(), name: modelName.trim() || modelId.trim() }])}>添加</Button></Box>
            {provider.models.map(item => <Box key={item.id} sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center' }}><TextField size="small" label={item.id} value={item.name} sx={{ flex: '1 1 200px' }} onChange={event => editProvider({ models: provider.models.map(model => model.id === item.id ? { ...model, name: event.target.value } : model) })} /><Button size="small" variant={provider.default_model === item.id ? 'contained' : 'outlined'} onClick={() => { editProvider({ default_model: item.id }); setTestModel(item.id); }}>{provider.default_model === item.id ? '默认模型' : '设为默认'}</Button><Button size="small" color="error" aria-label={`移除模型 ${item.id}`} onClick={() => { const rest = provider.models.filter(model => model.id !== item.id); editProvider({ models: rest, default_model: provider.default_model === item.id ? rest[0]?.id || '' : provider.default_model }); if (testModel === item.id) setTestModel(rest[0]?.id || ''); }}>移除</Button></Box>)}
            <Divider />
            <Typography variant="subtitle2">真实请求测试</Typography>
            <Typography variant="caption" color="text.secondary">检查地址仅验证可达性；hi、流式和工具测试可能产生上游费用。模型须通过 hi 才可对话，通过工具测试才可查询业务。结果只适用于当次已保存的服务与模型。</Typography>
            <TextField select size="small" label="本次测试模型" value={testModel} onChange={event => { stopProbes(); setTestModel(event.target.value); }}><MenuItem value="">请选择模型</MenuItem>{provider.models.map(item => <MenuItem key={item.id} value={item.id}>{item.name || item.id}</MenuItem>)}</TextField>
            <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>{(Object.keys(testNames) as AiTestKind[]).map(kind => <Button key={kind} size="small" variant="outlined" disabled={dirty || Boolean(busy) || saving || (kind !== 'connectivity' && !testModel)} onClick={() => void probe(kind)}>{busy === kind ? '测试中…' : testNames[kind]}</Button>)}{busy && <Button size="small" onClick={stopProbes}>取消测试</Button>}</Box>
            {visibleResults.map(renderResult)}
            {!results.length && <Typography variant="caption" color="text.secondary">{dirty ? '配置已变更，保存后重新测试。' : '此服务和模型暂无有效测试结果。'}</Typography>}
          </Stack>}
        </Box>}
        {tab === 1 && <Stack spacing={2}>
          <FormControlLabel control={<Switch checked={config.enabled} onChange={event => change({ ...config, enabled: event.target.checked })} />} label="启用 AI 助手" />
          <TextField size="small" label="助手名称" value={config.name} onChange={event => change({ ...config, name: event.target.value })} sx={{ maxWidth: 420 }} inputProps={{ maxLength: 40 }} />
          <Typography variant="subtitle2">开放能力</Typography>
          <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>{(Object.keys(abilityNames) as Array<keyof typeof abilityNames>).map(key => <FormControlLabel key={key} control={<Checkbox checked={Boolean(config.capabilities[key])} onChange={event => change({ ...config, capabilities: { ...config.capabilities, [key]: event.target.checked } })} />} label={abilityNames[key]} />)}</Box>
          <Typography variant="caption" color="text.secondary">通知已读与确认提交默认关闭，还需使用者在本次会话单独允许。通知已读只影响本人通知角标，不代表取样；提交必须先核对整批预览并点击确认。</Typography>
          <Alert severity="info">使用资格在“角色与权限 → 功能权限 → AI 助手”中分配，可点击上方“分配使用权限”进入。查询、统计和草稿再与账号现有业务权限取交集，每次调用由服务端复核。升级不会自动扩大旧角色权限。</Alert>
          <Typography variant="body2" color="text.secondary">程序知识可帮助寻找原功能。业务查询仅在使用者明确允许并确认日期后提供授权范围数据；草稿仍需带入原表单复核。模型不会直接提交、删除或更改业务记录。</Typography>
          <Accordion variant="outlined" disableGutters elevation={0}><AccordionSummary expandIcon={<ExpandMoreIcon />}><Typography variant="body2">高级调用限制</Typography></AccordionSummary><AccordionDetails><Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap' }}><TextField size="small" type="number" label="请求超时（秒）" value={config.timeout_seconds} inputProps={{ min: 10, max: 180 }} onChange={event => change({ ...config, timeout_seconds: Number(event.target.value) })} /><TextField size="small" type="number" label="每用户每分钟请求上限" value={config.max_requests_per_minute} inputProps={{ min: 1, max: 120 }} onChange={event => change({ ...config, max_requests_per_minute: Number(event.target.value) })} /></Box></AccordionDetails></Accordion>
        </Stack>}
        {tab === 2 && <Stack spacing={1.5}>
          <FormControlLabel control={<Switch checked={config.speech.enabled} onChange={event => change({ ...config, speech: { ...config.speech, enabled: event.target.checked } })} />} label="启用独立语音转文字服务" />
          <Typography variant="body2" color="text.secondary">使用提供兼容音频转写接口的独立服务。对话服务支持文字，不代表支持语音。</Typography>
          <TextField size="small" label="语音服务基础地址" value={config.speech.url} onChange={event => change({ ...config, speech: { ...config.speech, url: event.target.value } })} helperText="服务端调用 audio/transcriptions；地址末尾可包含 /v1。" />
          <TextField size="small" label="识别模型 ID" value={config.speech.model} onChange={event => change({ ...config, speech: { ...config.speech, model: event.target.value } })} />
          <TextField size="small" type="password" autoComplete="new-password" label={`语音密钥${config.speech.has_secret ? '（已配置，留空保留）' : '（未配置）'}`} value={config.speech.secret || ''} disabled={config.speech.no_key} onChange={event => change({ ...config, speech: { ...config.speech, secret: event.target.value, clear_secret: false } })} />
          <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap' }}><FormControlLabel control={<Checkbox checked={config.speech.no_key} onChange={event => change({ ...config, speech: { ...config.speech, no_key: event.target.checked, ...(event.target.checked ? { secret: '', clear_secret: true } : {}) } })} />} label="此语音服务无需密钥" /><FormControlLabel control={<Checkbox checked={Boolean(config.speech.clear_secret) && !config.speech.no_key} disabled={config.speech.no_key} onChange={event => change({ ...config, speech: { ...config.speech, clear_secret: event.target.checked, secret: '' } })} />} label="清除已保存语音密钥" /></Box>
          <Alert severity="info">录音需 HTTPS 或本机 localhost；远程局域网 HTTP 可选择本地音频文件。每段最多 60 秒、10MB，识别结果需人工复核。</Alert>
          <Box><AIVoiceInput disabled={dirty || !config.speech.enabled || saving} onApply={setSpeechText} label="测试语音" destination={saved?.speech.url || '语音服务'} /></Box>
          {speechText && <TextField label="语音测试文字" multiline minRows={3} value={speechText} onChange={event => setSpeechText(event.target.value)} />}
        </Stack>}
      </Box>
    </Paper>
  </Stack>;
}
