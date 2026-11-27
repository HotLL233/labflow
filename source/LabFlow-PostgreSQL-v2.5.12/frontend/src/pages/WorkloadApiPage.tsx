import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import {
  Alert, Autocomplete, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle,
  FormControlLabel, InputAdornment, LinearProgress, MenuItem, Paper, Snackbar, Stack, Switch, TextField, Typography,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import ConfirmDialog from '../components/ConfirmDialog';
import ManageWorkspace, { ManagePageHeader } from '../components/ManageWorkspace';
import { useUnsavedManagement } from '../components/ManagementContext';
import { getDivisions, requireApiSuccess } from '../api/client';
import {
  createWorkloadApiLink, deleteWorkloadApiLink, getWorkloadApiLinks, rotateWorkloadApiToken, updateWorkloadApiLink,
  type IssuedWorkloadApiLink, type WorkloadApiConfig, type WorkloadApiLink,
} from '../api/workloadIntegration';
import { formatBeijingDateTimeInput } from '../utils/sampleWorkloadDateTime';

type Department = { id: number; name: string };
type Editor = { link?: WorkloadApiLink; config: WorkloadApiConfig; initial: WorkloadApiConfig };
type Confirmation = { action: 'rotate' | 'disable' | 'delete'; link: WorkloadApiLink };

const kinds = { detection: '检测工作', auxiliary: '辅助工作', both: '检测与辅助工作' };
const departmentLabel = (id: number) => `未在当前列表中的部门（${id}）`;
const errorText = (error: unknown, fallback: string) => error instanceof Error ? error.message : fallback;

function CopyField({ label, value, onMessage }: { label: string; value: string; onMessage: (message: string) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const copy = async () => {
    try {
      if (!navigator.clipboard) throw new Error();
      await navigator.clipboard.writeText(value);
      onMessage(`已复制${label}`);
    } catch {
      input.current?.focus();
      input.current?.select();
      onMessage('无法自动复制，已选中文字，请手动复制。');
    }
  };
  return <TextField
    fullWidth size="small" label={label} value={value} inputRef={input} autoComplete="off"
    InputProps={{ readOnly: true, endAdornment: <InputAdornment position="end">
      <Button size="small" aria-label={`复制${label}`} onClick={() => void copy()}><ContentCopyIcon fontSize="small" /></Button>
    </InputAdornment> }}
    inputProps={{ style: { fontFamily: 'monospace' }, spellCheck: false }}
  />;
}

export default function WorkloadApiPage() {
  const [links, setLinks] = useState<WorkloadApiLink[]>([]);
  const [endpoint, setEndpoint] = useState('');
  const [departments, setDepartments] = useState<Department[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState('');
  const [editor, setEditor] = useState<Editor | null>(null);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [issued, setIssued] = useState<IssuedWorkloadApiLink | null>(null);
  const [busy, setBusy] = useState(false);
  const [operationError, setOperationError] = useState('');
  const [message, setMessage] = useState('');
  const inFlight = useRef(false);
  const url = endpoint ? window.location.origin + endpoint : '';

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError('');
    try {
      const [data, response] = await Promise.all([getWorkloadApiLinks(), getDivisions()]);
      const divisions = requireApiSuccess(response, '检测部门读取失败');
      if (!Array.isArray(divisions)) throw new Error('检测部门列表无效');
      setLinks(data.links);
      setEndpoint(data.endpoint);
      setDepartments(divisions.map(({ id, name }) => ({ id, name })));
    } catch (error) {
      setLoadError(errorText(error, '接入配置读取失败'));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => { void load(); }, [load]);

  const openEditor = (link?: WorkloadApiLink) => {
    const config: WorkloadApiConfig = link?.config ?? {
      name: '公司运营', kind: 'detection', departments: { mode: 'selected', ids: [] },
      from_month: formatBeijingDateTimeInput().slice(0, 7), through_month: null, expires_on: null, enabled: true,
    };
    setOperationError('');
    setEditor({ link, config, initial: config });
  };
  const closeEditor = () => { setEditor(null); setOperationError(''); };
  useUnsavedManagement('workload-api', {
    label: '工作量 API 接入配置',
    dirty: Boolean(editor && editor.config !== editor.initial),
    discard: closeEditor,
  });
  const patchConfig = (patch: Partial<WorkloadApiConfig>) =>
    setEditor(current => current && { ...current, config: { ...current.config, ...patch } });
  const upsert = (link: WorkloadApiLink) =>
    setLinks(current => [link, ...current.filter(item => item.id !== link.id)].sort((first, second) => second.id - first.id));

  const mutate = async (action: () => Promise<void>) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setOperationError('');
    try {
      await action();
    } catch (error) {
      setOperationError(errorText(error, '未能确认操作结果，请刷新配置后核对。'));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  };
  const save = (event: FormEvent) => {
    event.preventDefault();
    if (!editor) return;
    const { link, config } = editor;
    if (!config.name.trim() || !config.from_month || (config.departments.mode === 'selected' && !config.departments.ids.length)) {
      setOperationError('请填写名称、起始月份，并明确选择检测部门范围。');
      return;
    }
    void mutate(async () => {
      if (link) {
        upsert(await updateWorkloadApiLink(link, config));
        setMessage('接入范围已保存，旧分页快照已失效。');
      } else {
        const result = await createWorkloadApiLink(config);
        upsert(result.link);
        setIssued(result);
      }
      closeEditor();
    });
  };
  const confirm = ({ action, link }: Confirmation) => void mutate(async () => {
    if (action === 'delete') {
      await deleteWorkloadApiLink(link);
      setLinks(current => current.filter(item => item.id !== link.id));
      setMessage('接入已删除，原令牌已失效。');
    } else if (action === 'rotate') {
      const result = await rotateWorkloadApiToken(link);
      upsert(result.link);
      setIssued(result);
    } else {
      upsert(await updateWorkloadApiLink(link, { ...link.config, enabled: false }));
      setMessage('此接口已停用。');
    }
    setConfirmation(null);
  });
  const confirmationCopy = confirmation?.action === 'delete' ? {
    title: '永久删除接入', confirmText: '永久删除',
    message: `确定删除「${confirmation.link.config.name}」？原令牌将立即失效，此操作不可撤销。不会删除 LabFlow 的工作量记录或公司运营已同步的数据。`,
  } : confirmation?.action === 'rotate' ? {
    title: '轮换接入令牌', message: '旧令牌将立即失效，需将新令牌交给公司运营。',
  } : {
    title: '停用此接入', message: '公司运营将暂时无法读取此接入的数据，不会删除已同步的数据。',
  };
  const config = editor?.config;
  const selected = config?.departments.mode === 'selected'
    ? config.departments.ids.map(id => departments.find(item => item.id === id) ?? { id, name: departmentLabel(id) }) : [];
  const options = [...departments, ...selected.filter(item => !departments.some(department => department.id === item.id))];

  return <ManageWorkspace activeKey="workload-api">
    <ManagePageHeader title="工作量 API 接入" description="为公司运营生成专用令牌。接入方只能读取授权月份、部门和工作类别的工作量。"
      actions={<Button variant="contained" startIcon={<AddIcon />} disabled={loading || Boolean(loadError) || busy} onClick={() => openEditor()}>生成接入</Button>} />
    <Stack spacing={2} sx={{ maxWidth: 1000 }}>
      <Alert severity="info">接口不允许写入，不返回应收金额、样品内容或附件。向公司运营分别提供接口地址和令牌；跨电脑访问请使用 HTTPS。</Alert>
      {loading && <LinearProgress aria-label="正在读取接入配置" />}
      {loadError && <Alert severity="error" action={<Button onClick={() => void load()}>重试</Button>}>{loadError}</Alert>}
      {!loading && !loadError && <>
        <Paper variant="outlined" sx={{ p: 2 }}>
          <CopyField label="工作量接口地址" value={url} onMessage={setMessage} />
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>不同接入方共用地址，使用各自的令牌。若地址为 localhost，请换成公司运营可访问的部署地址。</Typography>
        </Paper>
        {links.length === 0 && <Paper variant="outlined" sx={{ p: 3 }}>
          <Typography variant="h6">还没有接入配置</Typography>
          <Typography variant="body2" color="text.secondary">点击「生成接入」，先选定允许读取的数据范围。</Typography>
        </Paper>}
        {links.map(link => <Paper key={link.id} variant="outlined" sx={{ p: 2 }}>
          <Stack direction={{ xs: 'column', md: 'row' }} spacing={2} justifyContent="space-between" alignItems={{ md: 'flex-start' }}>
            <Stack spacing={0.5} sx={{ minWidth: 0 }}>
              <Stack direction="row" spacing={1} alignItems="center" sx={{ flexWrap: 'wrap', rowGap: 0.5 }}>
                <Typography component="h2" variant="h6" sx={{ overflowWrap: 'anywhere' }}>{link.config.name}</Typography>
                <Chip size="small" label={!link.config.enabled ? '已停用' : link.expired ? '已到期' : '已启用'}
                  color={link.config.enabled ? link.expired ? 'warning' : 'success' : 'default'} />
              </Stack>
              <Typography variant="body2">{kinds[link.config.kind]} · {link.config.from_month} 起{link.config.through_month ? `，至 ${link.config.through_month}` : '，无结束月份'}</Typography>
              <Typography variant="body2" color="text.secondary" sx={{ overflowWrap: 'anywhere' }}>
                {link.config.departments.mode === 'all' ? '全部检测部门（含未归属历史）' : link.config.departments.ids.map(id => departments.find(item => item.id === id)?.name ?? departmentLabel(id)).join('、')}
              </Typography>
              <Typography variant="caption" color="text.secondary">{link.config.expires_on ? `令牌有效至 ${link.config.expires_on}（含当天）` : '令牌未设置到期日'}</Typography>
            </Stack>
            <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
              <Button disabled={busy} onClick={() => openEditor(link)}>编辑范围</Button>
              <Button disabled={busy} onClick={() => { setOperationError(''); setConfirmation({ action: 'rotate', link }); }}>轮换令牌</Button>
              {link.config.enabled && <Button color="error" disabled={busy} onClick={() => { setOperationError(''); setConfirmation({ action: 'disable', link }); }}>停用</Button>}
              <Button color="error" disabled={busy} onClick={() => { setOperationError(''); setConfirmation({ action: 'delete', link }); }}>删除</Button>
            </Box>
          </Stack>
        </Paper>)}
      </>}
    </Stack>
    {config && <Dialog open onClose={busy ? undefined : closeEditor} fullWidth maxWidth="sm" aria-labelledby="workload-api-form-title">
      <Box component="form" onSubmit={save}>
        <DialogTitle id="workload-api-form-title">{editor.link ? '编辑接入范围' : '生成工作量 API 接入'}</DialogTitle>
        <DialogContent dividers>
          <Stack spacing={2}>
            {operationError && <Alert severity="error">{operationError}</Alert>}
            <TextField required autoFocus label="接入名称" value={config.name} inputProps={{ maxLength: 80 }} disabled={busy}
              onChange={event => patchConfig({ name: event.target.value })} />
            <TextField select label="工作类别" value={config.kind} disabled={busy}
              onChange={event => patchConfig({ kind: event.target.value as WorkloadApiConfig['kind'] })}>
              <MenuItem value="detection">检测工作</MenuItem><MenuItem value="auxiliary">辅助工作</MenuItem><MenuItem value="both">检测与辅助工作</MenuItem>
            </TextField>
            <TextField select label="部门范围" value={config.departments.mode} disabled={busy}
              onChange={event => patchConfig({ departments: event.target.value === 'all' ? { mode: 'all' } : { mode: 'selected', ids: [] } })}>
              <MenuItem value="selected">仅指定检测部门</MenuItem><MenuItem value="all">全部检测部门（含未归属历史）</MenuItem>
            </TextField>
            {config.departments.mode === 'selected' && <Autocomplete multiple disableCloseOnSelect options={options} value={selected} disabled={busy}
              isOptionEqualToValue={(option, value) => option.id === value.id} getOptionLabel={option => option.name}
              onChange={(_, items) => patchConfig({ departments: { mode: 'selected', ids: items.map(item => item.id) } })}
              renderInput={params => <TextField {...params} label="允许读取的检测部门" helperText="按记录的检测部门快照筛选，不包含未归属部门的历史记录。" />} />}
            {config.departments.mode === 'all' && <Alert severity="warning">此范围会授权读取全部检测部门，包括以后新增的部门。</Alert>}
            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
              <TextField fullWidth required type="month" label="起始月份" InputLabelProps={{ shrink: true }} value={config.from_month} disabled={busy}
                onChange={event => patchConfig({ from_month: event.target.value })} />
              <TextField fullWidth type="month" label="结束月份（可选）" InputLabelProps={{ shrink: true }} value={config.through_month ?? ''} disabled={busy}
                inputProps={{ min: config.from_month }} onChange={event => patchConfig({ through_month: event.target.value || null })} />
            </Stack>
            <TextField type="date" label="令牌到期日（可选）" InputLabelProps={{ shrink: true }} value={config.expires_on ?? ''} disabled={busy}
              onChange={event => patchConfig({ expires_on: event.target.value || null })} helperText="留空则不自动到期；随时可停用或轮换。" />
            <FormControlLabel control={<Switch checked={config.enabled} disabled={busy} onChange={event => patchConfig({ enabled: event.target.checked })} />} label="启用此接入" />
          </Stack>
        </DialogContent>
        <DialogActions><Button disabled={busy} onClick={closeEditor}>取消</Button>
          <Button type="submit" variant="contained" disabled={busy}>{busy ? '保存中…' : editor.link ? '保存范围' : '生成链接和令牌'}</Button></DialogActions>
      </Box>
    </Dialog>}
    {confirmation && <ConfirmDialog open loading={busy} {...confirmationCopy}
      dependencySummary={operationError} onCancel={() => { setConfirmation(null); setOperationError(''); }} onConfirm={() => confirm(confirmation)} />}
    {issued && <Dialog open maxWidth="sm" fullWidth aria-labelledby="workload-api-issued-title">
      <DialogTitle id="workload-api-issued-title">接入凭证已生成</DialogTitle>
      <DialogContent dividers><Stack spacing={2}>
        <Alert severity="warning">令牌只显示这一次。请妥善保存，不要拼接到网址或发到公开群聊。遗失后请轮换令牌。</Alert>
        <CopyField label="工作量接口地址" value={url} onMessage={setMessage} />
        <CopyField label="接入令牌" value={issued.token} onMessage={setMessage} />
      </Stack></DialogContent>
      <DialogActions><Button variant="contained" onClick={() => setIssued(null)}>已保存，关闭</Button></DialogActions>
    </Dialog>}
    <Snackbar open={Boolean(message)} autoHideDuration={4000} message={message} onClose={() => setMessage('')} />
  </ManageWorkspace>;
}
