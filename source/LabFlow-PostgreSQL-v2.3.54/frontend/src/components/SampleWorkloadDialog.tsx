import { filterWorkloadInstruments, filterWorkloadOptions, findWorkloadTarget, nextWorkloadSearchQuery, parseSampleWorkloadDraft, sampleWorkloadDraftKey, workloadCoefficient, workloadInstrumentLabel, workloadInstrumentOptions, workloadMethodSearchText, workloadSearchPending, type SampleWorkloadDraft, type SampleWorkloadTarget } from '../utils/sampleWorkload';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Alert, Autocomplete, Box, Button, Collapse, Dialog, DialogActions, DialogContent, DialogTitle, MenuItem, TextField, Typography } from '@mui/material';
import { actionColorSx, type ActionColorPalette } from '../utils/recordTableStyle';
import { getServerTime } from '../api/system';
import { formatBeijingDateTimeInput, normalizeWorkloadDateTime } from '../utils/sampleWorkloadDateTime';

export type SampleWorkloadInstrument = {
  method_id: number; instrument_id: number; instrument: string; method_name?: string; multiplier: number; coefficient?: number;
  instrument_name?: string; instrument_code?: string; instrument_type?: string; method_types?: string[]; method_code?: string; method_short_name?: string;
  match_source?: 'mapping' | 'record' | 'type' | 'available';
};
export type SampleWorkloadPreview = {
  source_record_id: number; operator_username?: string; department: string; laboratory: string; project: string; quantity: number; recorded: boolean;
  recorded_quantity?: number; remaining_quantity?: number; coefficient?: number;
  method?: string; instrument?: string; multiplier?: number; detection_type?: string; instruments?: SampleWorkloadInstrument[];
  reason?: string | null; allow_custom?: boolean;
  source_summary?: { business_no: string; department: string; laboratory: string; project: string; detection_type: string; type_key: string; quantity: number; project_id: number | null; group_id: number | null };
  targets?: SampleWorkloadTarget[];
  selected_project_id?: number | null; selected_group_id?: number | null;
};
export type SampleWorkloadSubmit = {
  quantity: number; multiplier: number; recorded_at: string; method_id?: number;
  project_id?: number; group_id?: number;
  custom_method_name?: string; custom_instrument_name?: string; notes?: string;
};

const MATCH_LABEL = { mapping: '映射推荐', record: '登记信息匹配', type: '类型匹配', available: '可选方法' };
const GRID = { display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', sm: 'repeat(2,minmax(0,1fr))' }, gap: 1, mb: 1.5 };
const LISTBOX = { style: { maxHeight: 'min(224px, 40vh)', overflowX: 'hidden' as const } };
const EMPTY_SEARCH: Record<'department' | 'laboratory' | 'project' | 'instrument' | 'method', string | null> = { department: null, laboratory: null, project: null, instrument: null, method: null };
const methodLabel = (item: SampleWorkloadInstrument) => `${item.method_code ? `${item.method_code} · ` : ''}${item.method_name || item.method_short_name || '未命名方法'}${item.instrument_id === 0 ? '' : ` · ${workloadInstrumentLabel(item)}`}`;

export default function SampleWorkloadDialog({ preview, onClose, onConfirm, onReload, draftOwnerId, actionColors = {} }: {
  preview: SampleWorkloadPreview | null;
  onClose: () => void;
  onConfirm: (data: SampleWorkloadSubmit) => Promise<void>;
  onReload?: (target: { project_id: number; group_id: number }) => Promise<void>;
  draftOwnerId?: number;
  actionColors?: ActionColorPalette;
}) {
  const sourceId = preview?.source_record_id;
  const independent = Boolean(preview?.source_summary);
  const instruments = useMemo(() => preview?.instruments ?? [], [preview?.instruments]);
  const targets = useMemo(() => preview?.targets ?? [], [preview?.targets]);
  const [departmentId, setDepartmentId] = useState<number | ''>('');
  const [groupId, setGroupId] = useState<number | ''>('');
  const [projectId, setProjectId] = useState<number | ''>('');
  const [instrumentId, setInstrumentId] = useState<number | ''>('');
  const [methodId, setMethodId] = useState<number | ''>('');
  const [custom, setCustom] = useState(false);
  const [quantity, setQuantity] = useState(1);
  const [multiplier, setMultiplier] = useState(1);
  const [notes, setNotes] = useState('');
  const [customMethod, setCustomMethod] = useState('');
  const [customInstrument, setCustomInstrument] = useState('');
  const [recordedAt, setRecordedAt] = useState('');
  const dateEdited = useRef(false);
  const multiplierEdited = useRef(false);
  const pendingMethod = useRef<number | undefined>();
  const desiredBinding = useRef<{ project_id: number; group_id: number } | null>(null);
  const session = useRef(0);
  const [serverTimeFallback, setServerTimeFallback] = useState(false);
  const [saving, setSaving] = useState(false);
  const [bindingBusy, setBindingBusy] = useState(false);
  const submitting = useRef(false);
  const [error, setError] = useState('');
  const [draftNotice, setDraftNotice] = useState('');
  const [sourceExpanded, setSourceExpanded] = useState(false);
  const [searchQueries, setSearchQueries] = useState(EMPTY_SEARCH);
  const trackSearch = (field: keyof typeof EMPTY_SEARCH, value: string, reason: string) => {
    setSearchQueries(current => {
      const query = nextWorkloadSearchQuery(current[field], value, reason);
      return query === current[field] ? current : { ...current, [field]: query };
    });
  };
  const selectedTarget = findWorkloadTarget(targets, projectId || undefined, groupId || undefined);
  const bindingCurrent = !independent || Boolean(selectedTarget && selectedTarget.project_id === preview?.selected_project_id && selectedTarget.group_id === preview?.selected_group_id);
  const filteredMethods = filterWorkloadInstruments(instruments, instrumentId);
  const selected = instruments.find(item => item.method_id === methodId);
  const departments = [...new Map(targets.map(target => [target.division_id ?? 0, target])).values()];
  const laboratories = [...new Map(targets.filter(target => (target.division_id ?? 0) === departmentId).map(target => [target.group_id, target])).values()];
  const projects = targets.filter(target => target.group_id === groupId && (target.division_id ?? 0) === departmentId);
  const instrumentOptions = useMemo(() => workloadInstrumentOptions(instruments), [instruments]);
  const selectedDepartment = departments.find(target => (target.division_id ?? 0) === departmentId) ?? null;
  const selectedLaboratory = laboratories.find(target => target.group_id === groupId) ?? null;
  const selectedProject = projects.find(target => target.project_id === projectId) ?? null;
  const selectedInstrument = instrumentId === '' ? null : instrumentOptions.find(item => item.instrument_id === instrumentId) ?? null;
  const selectedMethod = bindingCurrent ? filteredMethods.find(item => item.method_id === methodId) ?? null : null;
  const departmentLabel = selectedDepartment ? selectedDepartment.department || '未指定部门' : '';
  const laboratoryLabel = selectedLaboratory?.laboratory || '';
  const projectLabel = selectedProject?.project_name || '';
  const instrumentLabel = selectedInstrument ? workloadInstrumentLabel(selectedInstrument) : '';
  const selectedMethodLabel = selectedMethod ? methodLabel(selectedMethod) : '';
  const customMode = independent ? custom || targets.length === 0 : instruments.length === 0 && Boolean(preview?.allow_custom);
  const requiresMethodChoice = independent || instruments.length > 0;
  const libraryAvailable = independent ? instruments.length > 0 : instruments.length > 0 || Boolean(preview?.method);
  const coefficient = independent ? selectedTarget?.coefficient : workloadCoefficient(selected, preview?.coefficient, requiresMethodChoice);
  const quantityValid = Number.isInteger(quantity) && quantity > 0 && quantity <= (preview?.remaining_quantity ?? preview?.quantity ?? 0);
  const multiplierValid = Number.isFinite(multiplier) && multiplier >= 0;
  const normalizedRecordedAt = normalizeWorkloadDateTime(recordedAt);
  const dateTimeError = !recordedAt ? '请选择工作量日期时间' : !normalizedRecordedAt ? '请输入有效的工作量日期时间' : '';
  const searchPending = workloadSearchPending(searchQueries.department, departmentLabel)
    || workloadSearchPending(searchQueries.laboratory, laboratoryLabel) || workloadSearchPending(searchQueries.project, projectLabel)
    || (!customMode && (workloadSearchPending(searchQueries.instrument, instrumentLabel) || workloadSearchPending(searchQueries.method, selectedMethodLabel)));
  const canSubmit = !bindingBusy && bindingCurrent && !searchPending && Number.isFinite(coefficient) && Boolean(normalizedRecordedAt) && quantityValid && multiplierValid
    && (customMode ? Boolean(preview?.allow_custom) && customMethod.trim().length > 0 : libraryAvailable && (!requiresMethodChoice || Boolean(selected)));
  const draftKey = independent && sourceId ? sampleWorkloadDraftKey(draftOwnerId, sourceId) : null;

  const reloadBinding = async (target: { project_id: number; group_id: number }) => {
    if (!onReload) return;
    desiredBinding.current = target;
    const requestSession = session.current;
    setBindingBusy(true); setError('');
    try { await onReload(target); }
    catch (reason: unknown) {
      if (requestSession === session.current) setError(reason instanceof Error ? reason.message : '计量选项加载失败，请重新选择后重试。');
    } finally { if (requestSession === session.current) setBindingBusy(false); }
  };

  // 只在打开另一条记录或切换用户时初始化，目标/候选刷新不覆盖日期、数量或备注。
  useEffect(() => {
    session.current += 1;
    if (!preview) return;
    let cancelled = false;
    let draft: SampleWorkloadDraft | null = null;
    try { draft = draftKey ? parseSampleWorkloadDraft(localStorage.getItem(draftKey)) : null; } catch { /* 本机存储不可用不阻止录入。 */ }
    const draftTarget = draft && findWorkloadTarget(targets, draft.project_id, draft.group_id);
    const target = draftTarget || findWorkloadTarget(targets, preview.selected_project_id, preview.selected_group_id);
    desiredBinding.current = target || null;
    setDepartmentId(target?.division_id ?? (target ? 0 : ''));
    setGroupId(target?.group_id ?? ''); setProjectId(target?.project_id ?? ''); setInstrumentId('');
    pendingMethod.current = draft?.method_id;
    setMethodId(draft?.method_id ?? (instruments.length === 1 ? instruments[0].method_id : ''));
    setCustom(draft?.custom ?? (instruments.length === 0 && Boolean(preview.allow_custom)));
    setQuantity(draft?.quantity ?? preview.remaining_quantity ?? preview.quantity);
    setMultiplier(draft?.multiplier ?? (instruments.length === 1 ? instruments[0].multiplier : preview.multiplier ?? 1));
    multiplierEdited.current = Boolean(draft);
    setNotes(draft?.notes ?? ''); setCustomMethod(draft?.custom_method_name ?? ''); setCustomInstrument(draft?.custom_instrument_name ?? '');
    const draftDate = draft && normalizeWorkloadDateTime(draft.recorded_at);
    dateEdited.current = Boolean(draftDate);
    setRecordedAt(draftDate || formatBeijingDateTimeInput()); setServerTimeFallback(false);
    setError(''); setBindingBusy(false);
    setSourceExpanded(false); setSearchQueries(EMPTY_SEARCH);
    setDraftNotice(draft ? '已恢复此账号在本机保存的待关联草稿；尚未计入工作量，请核对标准计量归属及剩余数量。' : '');
    if (draftTarget && (draftTarget.project_id !== preview.selected_project_id || draftTarget.group_id !== preview.selected_group_id)) void reloadBinding(draftTarget);
    void getServerTime().then(response => {
      if (cancelled || dateEdited.current) return;
      const value = response.code === 0 && response.data && Number.isFinite(response.data.unix_ms) ? formatBeijingDateTimeInput(response.data.unix_ms) : '';
      if (!normalizeWorkloadDateTime(value)) throw new Error('服务器时间不可用');
      setRecordedAt(value);
    }).catch(() => { if (!cancelled && !dateEdited.current) setServerTimeFallback(true); });
    return () => { cancelled = true; session.current += 1; };
  }, [sourceId, draftOwnerId]);

  useEffect(() => {
    if (!preview || !bindingCurrent) return;
    // 恢复跨项目草稿时，初次渲染仍可能带上一条记录的绑定状态，不能用旧候选消费待恢复方法。
    if (independent && (!desiredBinding.current || desiredBinding.current.project_id !== preview.selected_project_id || desiredBinding.current.group_id !== preview.selected_group_id)) return;
    const candidate = instruments.find(item => item.method_id === (pendingMethod.current ?? methodId))
      || (instruments.length === 1 ? instruments[0] : undefined);
    pendingMethod.current = undefined;
    if (methodId !== (candidate?.method_id ?? '')) setMethodId(candidate?.method_id ?? '');
    if (!multiplierEdited.current && candidate) setMultiplier(candidate.multiplier);
    if (instrumentId !== '' && !instruments.some(item => item.instrument_id === instrumentId)) setInstrumentId('');
    if (independent && selectedTarget && departmentId !== (selectedTarget.division_id ?? 0)) setDepartmentId(selectedTarget.division_id ?? 0);
  }, [instruments, bindingCurrent, preview?.selected_project_id, preview?.selected_group_id, methodId, instrumentId]);

  useEffect(() => {
    const binding = desiredBinding.current;
    if (!independent || !binding || findWorkloadTarget(targets, binding.project_id, binding.group_id)) return;
    desiredBinding.current = null; pendingMethod.current = undefined;
    setProjectId(''); setMethodId(''); setInstrumentId('');
    if (!targets.some(target => target.group_id === binding.group_id)) setGroupId('');
    if (!targets.some(target => (target.division_id ?? 0) === departmentId)) setDepartmentId('');
    setError('原计量归属已不可用，请重新选择；日期、数量及备注仍保留。');
  }, [targets, independent]);

  const saveDraft = () => {
    if (!draftKey) return;
    const draft: SampleWorkloadDraft = {
      version: 1, quantity, multiplier, recorded_at: recordedAt, notes, custom_method_name: customMethod, custom_instrument_name: customInstrument,
      custom: customMode, method_id: methodId || undefined, project_id: projectId || undefined, group_id: groupId || undefined,
    };
    if (!parseSampleWorkloadDraft(JSON.stringify(draft))) { setError('请先填写有效数量和倍率，再保存待关联草稿。'); return; }
    try {
      localStorage.setItem(draftKey, JSON.stringify(draft));
      setDraftNotice('已保存待关联草稿，仅当前账号在本机可恢复；尚未计入工作量。'); setError('');
    } catch { setError('本机存储不可用，草稿未保存；当前输入仍保留。'); }
  };

  const submit = async () => {
    if (!preview || !canSubmit || !normalizedRecordedAt || submitting.current) return;
    const requestSession = session.current;
    dateEdited.current = true;
    submitting.current = true; setSaving(true); setError('');
    try {
      await onConfirm({
        quantity, multiplier, recorded_at: normalizedRecordedAt,
        project_id: independent ? selectedTarget?.project_id : undefined, group_id: independent ? selectedTarget?.group_id : undefined,
        method_id: customMode ? undefined : (methodId || undefined),
        custom_method_name: customMode ? customMethod.trim() : undefined,
        custom_instrument_name: customMode ? (customInstrument.trim() || undefined) : undefined,
        notes: notes.trim() || undefined,
      });
      try { if (draftKey) localStorage.removeItem(draftKey); } catch { /* 已入库成功不因本机清理失败报错。 */ }
    } catch (reason: unknown) {
      if (requestSession === session.current) setError(reason instanceof Error ? reason.message : '录入失败，请检查后重试；输入已保留。');
    } finally { submitting.current = false; setSaving(false); }
  };

  const original = preview?.source_summary;
  const summary: Array<[string, string | number | undefined]> = [
    ['当前录入人', preview?.operator_username], ...(original ? [['登记编号', original.business_no] as [string, string]] : []),
    ['部门', original?.department ?? preview?.department], ['实验室', original?.laboratory ?? preview?.laboratory], ['项目', original?.project ?? preview?.project],
    ['检测类型', original?.detection_type ?? preview?.detection_type ?? '未提供'],
    ...(!independent ? [['方法 / 仪器', customMode ? (customMethod.trim() || '待自定义') : (selected?.instrument || preview?.instrument || preview?.method)] as [string, string | undefined]] : []),
    ['原始数量', original?.quantity ?? preview?.quantity], ['已录入数量', preview?.recorded_quantity ?? 0], ['剩余数量', preview?.remaining_quantity ?? preview?.quantity],
  ];

  return <Dialog open={!!preview} onClose={() => !saving && onClose()} fullWidth maxWidth="sm">
    <DialogTitle>录入取样工作量</DialogTitle>
    <DialogContent>
      {error && <Alert severity="error" sx={{ mb: 1.5 }}>{error}</Alert>}
      {draftNotice && <Alert severity="info" sx={{ mb: 1.5 }}>{draftNotice}</Alert>}
      <Box component="fieldset" disabled={saving || bindingBusy} sx={{ border: 0, p: 0, m: 0, minWidth: 0 }}>
      {preview?.recorded ? <Alert severity="info" sx={{ mb: 1.5 }}>本次取样已录入工作量。</Alert> : <>
        <Box sx={{ border: '1px solid', borderColor: 'divider', borderRadius: '2px', px: 1, py: 0.75, mb: 1.5, minWidth: 0 }}>
          <Box sx={{ display: 'flex', justifyContent: 'space-between', gap: 0.75, alignItems: 'center', flexWrap: 'wrap' }}>
            <Typography variant="body2" sx={{ minWidth: 0, overflowWrap: 'anywhere' }}>当前录入人：<Box component="strong">{preview?.operator_username || '—'}</Box></Typography>
            <Button size="small" onClick={() => setSourceExpanded(current => !current)} aria-expanded={sourceExpanded} aria-controls={`workload-source-${sourceId}`} sx={{ px: 0.5, py: 0.15, minWidth: 0, fontSize: '0.72rem', flexShrink: 0 }}>{sourceExpanded ? '收起详情' : independent ? '原登记详情' : '来源详情'}</Button>
          </Box>
          <Typography variant="caption" component="div" color="text.secondary" sx={{ overflowWrap: 'anywhere', mt: 0.25, display: '-webkit-box', WebkitBoxOrient: 'vertical', WebkitLineClamp: 1, overflow: 'hidden' }}>{original?.business_no ? `编号：${original.business_no} · ` : ''}检测类型：{original?.detection_type || preview?.detection_type || '未提供'}</Typography>
          <Typography variant="caption" component="div" color="text.secondary" sx={{ overflowWrap: 'anywhere', mt: 0.25, display: '-webkit-box', WebkitBoxOrient: 'vertical', WebkitLineClamp: 2, overflow: 'hidden' }}>来源：{original?.laboratory || preview?.laboratory || '未提供实验室'} / {original?.project || preview?.project || '未提供项目'}</Typography>
          <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: '4px 16px', mt: 0.5 }}>
            {[['原始', original?.quantity ?? preview?.quantity], ['已录入', preview?.recorded_quantity ?? 0], ['剩余', preview?.remaining_quantity ?? preview?.quantity]].map(([label, value]) => <Typography key={String(label)} variant="caption">{label} <Box component="strong" sx={{ fontSize: '0.82rem' }}>{value ?? '—'}</Box></Typography>)}
          </Box>
          <Collapse in={sourceExpanded} unmountOnExit id={`workload-source-${sourceId}`}>
            <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>{independent ? '原登记信息只读，不会随计量选择修改。' : '来源信息只读。'}</Typography>
            <Box component="dl" sx={{ ...GRID, mt: 0.5, mb: 0, gap: 0.5 }}>
              {summary.map(([label, value]) => <Box key={label} sx={{ display: 'flex', gap: 0.75, minWidth: 0, py: 0.4, borderBottom: '1px solid', borderColor: 'divider' }}><Typography component="dt" variant="caption" color="text.secondary" sx={{ flexShrink: 0, minWidth: '5em' }}>{label}</Typography><Typography component="dd" variant="body2" sx={{ m: 0, minWidth: 0, overflowWrap: 'anywhere' }}>{value || (value === 0 ? 0 : '—')}</Typography></Box>)}
            </Box>
          </Collapse>
        </Box>
        {independent && <>
          <Typography variant="subtitle2" sx={{ mb: 1 }}>标准工作量计量归属</Typography>
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1 }}>匹配仅作为推荐，可在授权范围内重新选择；仪器筛选其关联方法。</Typography>
          {targets.length === 0 && <Alert severity="warning" sx={{ mb: 1.5 }}>{preview?.reason || '当前没有可录入的标准实验室与项目。请联系管理员维护授权及主数据，可先保存本机待关联草稿。'}</Alert>}
          <Box sx={GRID}>
            <Autocomplete key={`${sourceId}:department`} size="small" autoHighlight clearOnBlur={false} options={departments} value={selectedDepartment} inputValue={searchQueries.department ?? departmentLabel} disabled={saving || bindingBusy}
              isOptionEqualToValue={(option, value) => option.division_id === value.division_id}
              getOptionLabel={option => option.department || '未指定部门'}
              filterOptions={(options, state) => filterWorkloadOptions(options, state.inputValue, option => option.department || '未指定部门')}
              onInputChange={(_event, value, reason) => trackSearch('department', value, reason)}
              onClose={(_event, reason) => { if (reason === 'selectOption') trackSearch('department', '', reason); }}
              onChange={(_event, option) => { desiredBinding.current = null; pendingMethod.current = undefined; setDepartmentId(option ? option.division_id ?? 0 : ''); setGroupId(''); setProjectId(''); setMethodId(''); setInstrumentId(''); setSearchQueries(EMPTY_SEARCH); }}
              renderInput={params => <TextField {...params} label="计量部门" placeholder="输入关键词搜索部门" />}
              ListboxProps={LISTBOX} noOptionsText="没有匹配的授权部门，请缩短或清空关键词" openText="展开部门" closeText="收起部门" clearText="清除部门及后续选择" />
            <Autocomplete key={`${sourceId}:${departmentId}:laboratory`} size="small" autoHighlight clearOnBlur={false} options={laboratories} value={selectedLaboratory} inputValue={searchQueries.laboratory ?? laboratoryLabel} disabled={saving || departmentId === '' || bindingBusy}
              isOptionEqualToValue={(option, value) => option.group_id === value.group_id} getOptionLabel={option => option.laboratory}
              filterOptions={(options, state) => filterWorkloadOptions(options, state.inputValue, option => `${option.laboratory} ${option.department}`)}
              onInputChange={(_event, value, reason) => trackSearch('laboratory', value, reason)}
              onClose={(_event, reason) => { if (reason === 'selectOption') trackSearch('laboratory', '', reason); }}
              onChange={(_event, option) => { desiredBinding.current = null; pendingMethod.current = undefined; setGroupId(option?.group_id ?? ''); setProjectId(''); setMethodId(''); setInstrumentId(''); setSearchQueries(current => ({ ...current, laboratory: null, project: null, instrument: null, method: null })); }}
              renderInput={params => <TextField {...params} label="计量实验室" placeholder="输入关键词搜索实验室" />}
              ListboxProps={LISTBOX} noOptionsText="该部门下没有匹配的授权实验室，请清空关键词" openText="展开实验室" closeText="收起实验室" clearText="清除实验室及后续选择" />
            <Autocomplete key={`${sourceId}:${departmentId}:${groupId}:project`} size="small" autoHighlight clearOnBlur={false} options={projects} value={selectedProject} inputValue={searchQueries.project ?? projectLabel} disabled={saving || !groupId || bindingBusy} sx={{ gridColumn: '1 / -1' }}
              isOptionEqualToValue={(option, value) => option.project_id === value.project_id && option.group_id === value.group_id} getOptionLabel={option => option.project_name}
              filterOptions={(options, state) => filterWorkloadOptions(options, state.inputValue, option => `${option.project_name} ${option.laboratory} ${option.department}`)}
              onInputChange={(_event, value, reason) => trackSearch('project', value, reason)}
              onClose={(_event, reason) => { if (reason === 'selectOption') trackSearch('project', '', reason); }}
              onChange={(_event, option) => {
                setProjectId(option?.project_id ?? ''); setMethodId(''); setInstrumentId(''); pendingMethod.current = undefined; desiredBinding.current = null;
                setSearchQueries(current => ({ ...current, project: null, instrument: null, method: null }));
                if (option) void reloadBinding({ project_id: option.project_id, group_id: option.group_id });
              }}
              renderOption={(props, option) => <Box component="li" {...props} key={`${option.group_id}:${option.project_id}`} sx={{ display: 'block', overflowWrap: 'anywhere' }}><Typography variant="body2">{option.project_name}</Typography><Typography variant="caption" color="text.secondary">{option.laboratory} · 工作量系数 {option.coefficient}</Typography></Box>}
              renderInput={params => <TextField {...params} label="计量项目" placeholder="输入关键词搜索当前实验室项目" />}
              ListboxProps={LISTBOX} noOptionsText="当前实验室没有匹配的有效项目，请清空关键词或更换实验室" openText="展开项目" closeText="收起项目" clearText="清除项目及方法选择" />
          </Box>
          {bindingBusy && <Alert severity="info" sx={{ mb: 1.5 }}>正在加载该计量项目的可选方法与仪器，其他输入会保留。</Alert>}
          {selectedTarget && !bindingBusy && !bindingCurrent && <Button size="small" onClick={() => void reloadBinding(selectedTarget)} disabled={saving} sx={{ mb: 1 }}>重新加载计量选项</Button>}
          {selectedTarget && original && (original.project_id !== selectedTarget.project_id || original.group_id !== selectedTarget.group_id) && <Alert severity="info" sx={{ mb: 1.5 }}>工作量将归入所选标准实验室和项目；原登记名称及记录保持不变。</Alert>}
          {preview?.allow_custom && <TextField select fullWidth size="small" label="方法录入方式" value={customMode ? 'custom' : 'standard'} disabled={saving || bindingBusy || targets.length === 0} onChange={event => { setCustom(event.target.value === 'custom'); setSearchQueries(current => ({ ...current, instrument: null, method: null })); }} sx={{ mb: 1.5 }}>
            <MenuItem value="standard">标准方法 / 关联仪器</MenuItem><MenuItem value="custom">自定义方法 / 仪器（未绑定主数据）</MenuItem>
          </TextField>}
        </>}
        {customMode ? <>
          <Alert severity="info" sx={{ mb: 1.5 }}>{independent && targets.length === 0 ? '可先填写实际方法和仪器，保存本机待关联草稿；选好有效标准计量归属前不能录入工作量。' : '自定义方法和仪器不创建或绑定标准主数据；分类汇总将显示为未分类，工作量按所选项目系数计算。'}</Alert>
          <Box sx={GRID}>
            <TextField label="自定义方法名称" size="small" required value={customMethod} onChange={event => setCustomMethod(event.target.value)} helperText="请填写实际检测方法" />
            <TextField label="自定义仪器名称" size="small" value={customInstrument} onChange={event => setCustomInstrument(event.target.value)} helperText="可不填，不会自动创建仪器" />
          </Box>
        </> : <>
          {independent && bindingCurrent && <Autocomplete key={`${sourceId}:${groupId}:${projectId}:instrument`} fullWidth size="small" autoHighlight clearOnBlur={false} options={instrumentOptions} value={selectedInstrument} inputValue={searchQueries.instrument ?? instrumentLabel} disabled={saving || bindingBusy}
            isOptionEqualToValue={(option, value) => option.instrument_id === value.instrument_id} getOptionLabel={workloadInstrumentLabel}
            filterOptions={(options, state) => filterWorkloadOptions(options, state.inputValue, option => `${workloadInstrumentLabel(option)} ${option.search_text}`)}
            onInputChange={(_event, value, reason) => trackSearch('instrument', value, reason)}
            onClose={(_event, reason) => { if (reason === 'selectOption') trackSearch('instrument', '', reason); }}
            onChange={(_event, option) => {
              const id = option?.instrument_id ?? ''; setInstrumentId(id);
              setSearchQueries(current => ({ ...current, instrument: null, method: null }));
              const filtered = filterWorkloadInstruments(instruments, id);
              if (!filtered.some(item => item.method_id === methodId)) { setMethodId(filtered.length === 1 ? filtered[0].method_id : ''); if (!multiplierEdited.current && filtered.length === 1) setMultiplier(filtered[0].multiplier); }
            }}
            renderOption={(props, option) => <Box component="li" {...props} key={option.instrument_id} sx={{ display: 'block', overflowWrap: 'anywhere' }}><Typography variant="body2">{workloadInstrumentLabel(option)}</Typography><Typography variant="caption" color="text.secondary">{[option.instrument_type, ...option.method_types].filter(Boolean).filter((value, index, values) => values.indexOf(value) === index).join(' · ') || '未提供类型'} · {option.method_count} 个关联方法</Typography></Box>}
            renderInput={params => <TextField {...params} label="仪器筛选" placeholder="全部仪器；可搜索名称、代码或类型" helperText="清空仪器筛选可恢复全部关联方法" />}
            ListboxProps={LISTBOX} noOptionsText="当前项目没有匹配的关联仪器，请清空关键词或筛选" openText="展开仪器" closeText="收起仪器" clearText="清空仪器筛选，查看全部方法" sx={{ mb: 1.5 }} />}
          {(independent || instruments.length > 0) && <Autocomplete
            key={`${sourceId}:${groupId}:${projectId}:${instrumentId}:method`} fullWidth size="small" autoHighlight clearOnBlur={false} options={bindingCurrent ? filteredMethods : []}
            value={selectedMethod}
            inputValue={searchQueries.method ?? selectedMethodLabel}
            disabled={saving || bindingBusy || !bindingCurrent || filteredMethods.length === 0}
            isOptionEqualToValue={(option, value) => option.method_id === value.method_id}
            getOptionLabel={methodLabel}
            filterOptions={(options, state) => filterWorkloadOptions(options, state.inputValue, item => `${methodLabel(item)} ${workloadMethodSearchText(item)}`)}
            onInputChange={(_event, value, reason) => trackSearch('method', value, reason)}
            onClose={(_event, reason) => { if (reason === 'selectOption') trackSearch('method', '', reason); }}
            onChange={(_event, item) => {
              setMethodId(item?.method_id ?? '');
              setSearchQueries(current => ({ ...current, method: null }));
              if (item) { setMultiplier(item.multiplier); multiplierEdited.current = false; }
            }}
            renderOption={(props, item) => <Box component="li" {...props} key={item.method_id} sx={{ display: 'block', whiteSpace: 'normal', overflowWrap: 'anywhere' }}><Typography variant="body2">{methodLabel(item)}</Typography><Typography variant="caption" color="text.secondary">{[item.instrument_type, ...(item.method_types ?? [])].filter(Boolean).filter((value, index, values) => values.indexOf(value) === index).join(' · ') || '未提供类型'}{item.match_source ? ` · ${MATCH_LABEL[item.match_source]}` : ''}</Typography></Box>}
            renderInput={params => <TextField {...params} label="检测方法 / 关联仪器" placeholder="搜索方法、代码、仪器或检测类型" helperText={`当前范围 ${bindingCurrent ? filteredMethods.length : 0} 个有效方法；搜索文字须从候选列表确认`} />}
            ListboxProps={LISTBOX} noOptionsText="当前项目及仪器范围没有匹配的方法，请清空关键词或仪器筛选" openText="展开方法" closeText="收起方法" clearText="清除所选方法" sx={{ mb: 1.5 }}
          />}
          {bindingCurrent && !libraryAvailable && <Alert severity="warning" sx={{ mb: 1.5 }}>{preview?.reason || '该项目没有可用标准方法，可选择自定义方法或联系管理员维护关联。'}</Alert>}
        </>}
        <Box sx={GRID}>
          <TextField label="工作量日期时间" type="datetime-local" size="small" required fullWidth value={recordedAt}
            onChange={event => { dateEdited.current = true; setRecordedAt(event.target.value); }}
            disabled={saving} error={Boolean(dateTimeError)} InputLabelProps={{ shrink: true }} inputProps={{ step: 1 }}
            helperText={dateTimeError || (serverTimeFallback ? '服务器时间未校准，请核对所选北京时间；统计按此时间归类，真实提交时间单独记录。' : '默认当前北京时间，可选择历史日期；统计按此时间归类，真实提交时间单独记录。')}
            sx={{ gridColumn: '1 / -1', minWidth: 0, '& .MuiInputBase-root, & input': { minWidth: 0, width: '100%' } }} />
          <TextField label="本次录入数量" type="number" size="small" value={quantity} onChange={event => setQuantity(Number(event.target.value))} error={!quantityValid} inputProps={{ min: 1, max: preview?.remaining_quantity ?? preview?.quantity }} helperText={`不得超过剩余数量 ${preview?.remaining_quantity ?? preview?.quantity}`} />
          <TextField label="金额倍率" type="number" size="small" value={multiplier} onChange={event => { multiplierEdited.current = true; setMultiplier(Number(event.target.value)); }} inputProps={{ min: 0, step: 0.1 }} helperText="仅影响导出金额，不影响工作量" />
          <TextField label="计算工作量" size="small" value={coefficient === undefined ? '请先选择计量归属' : (quantity * coefficient).toFixed(2)} InputProps={{ readOnly: true }} helperText={coefficient === undefined ? '选择后按入库系数预览' : `数量 × 项目系数 ${coefficient}`} />
          <TextField label="备注" size="small" value={notes} onChange={event => setNotes(event.target.value)} />
        </Box>
        {searchPending && <Typography variant="caption" color="warning.main">搜索文字尚未确认，请选择库中的条目或清空搜索后再录入。</Typography>}
      </>}
      </Box>
    </DialogContent>
    <DialogActions sx={{ flexWrap: 'wrap', gap: 0.5 }}>
      {draftKey && !preview?.recorded && <Button onClick={saveDraft} disabled={saving || bindingBusy} size="small">保存待关联草稿</Button>}
      <Button onClick={onClose} disabled={saving} sx={actionColorSx(actionColors, 'cancel', saving ? 'disabled' : 'idle')}>取消</Button>
      <Button variant="contained" onClick={submit} disabled={saving || !!preview?.recorded || !canSubmit} sx={actionColorSx(actionColors, 'workload', saving ? 'loading' : preview?.recorded ? 'completed' : !canSubmit ? 'disabled' : (preview?.recorded_quantity || 0) > 0 ? 'partial' : 'idle')}>{saving ? '录入中…' : '确认录入'}</Button>
    </DialogActions>
  </Dialog>;
}
