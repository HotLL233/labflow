import { filterWorkloadInstruments, findWorkloadTarget, parseSampleWorkloadDraft, sampleWorkloadDraftKey, workloadCoefficient, type SampleWorkloadDraft, type SampleWorkloadTarget } from '../utils/sampleWorkload';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Alert, Autocomplete, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, MenuItem, TextField, Typography } from '@mui/material';
import { actionColorSx, type ActionColorPalette } from '../utils/recordTableStyle';
import { getServerTime } from '../api/system';
import { formatBeijingDateTimeInput, normalizeWorkloadDateTime } from '../utils/sampleWorkloadDateTime';

export type SampleWorkloadInstrument = { method_id: number; instrument_id: number; instrument: string; method_name?: string; multiplier: number; coefficient?: number; match_source?: 'mapping' | 'record' | 'type' | 'available' };
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
  const selectedTarget = findWorkloadTarget(targets, projectId || undefined, groupId || undefined);
  const bindingCurrent = !independent || Boolean(selectedTarget && selectedTarget.project_id === preview?.selected_project_id && selectedTarget.group_id === preview?.selected_group_id);
  const filteredMethods = filterWorkloadInstruments(instruments, instrumentId);
  const selected = instruments.find(item => item.method_id === methodId);
  const customMode = independent ? custom || targets.length === 0 : instruments.length === 0 && Boolean(preview?.allow_custom);
  const requiresMethodChoice = independent || instruments.length > 0;
  const libraryAvailable = independent ? instruments.length > 0 : instruments.length > 0 || Boolean(preview?.method);
  const coefficient = independent ? selectedTarget?.coefficient : workloadCoefficient(selected, preview?.coefficient, requiresMethodChoice);
  const quantityValid = Number.isInteger(quantity) && quantity > 0 && quantity <= (preview?.remaining_quantity ?? preview?.quantity ?? 0);
  const multiplierValid = Number.isFinite(multiplier) && multiplier >= 0;
  const normalizedRecordedAt = normalizeWorkloadDateTime(recordedAt);
  const dateTimeError = !recordedAt ? '请选择工作量日期时间' : !normalizedRecordedAt ? '请输入有效的工作量日期时间' : '';
  const canSubmit = !bindingBusy && bindingCurrent && Number.isFinite(coefficient) && Boolean(normalizedRecordedAt) && quantityValid && multiplierValid
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
  const departments = [...new Map(targets.map(target => [target.division_id ?? 0, target])).values()];
  const laboratories = [...new Map(targets.filter(target => (target.division_id ?? 0) === departmentId).map(target => [target.group_id, target])).values()];
  const projects = targets.filter(target => target.group_id === groupId && (target.division_id ?? 0) === departmentId);
  const instrumentOptions = [...new Map(instruments.map(item => [item.instrument_id, item])).values()];

  return <Dialog open={!!preview} onClose={() => !saving && onClose()} fullWidth maxWidth="sm">
    <DialogTitle>录入取样工作量</DialogTitle>
    <DialogContent>
      {error && <Alert severity="error" sx={{ mb: 1.5 }}>{error}</Alert>}
      {draftNotice && <Alert severity="info" sx={{ mb: 1.5 }}>{draftNotice}</Alert>}
      <Box component="fieldset" disabled={saving || bindingBusy} sx={{ border: 0, p: 0, m: 0, minWidth: 0 }}>
      {preview?.recorded ? <Alert severity="info" sx={{ mb: 1.5 }}>本次取样已录入工作量。</Alert> : <>
        {independent && <Typography variant="subtitle2" sx={{ mb: 1 }}>原登记信息（只读，不会随计量选择修改）</Typography>}
        <Box sx={{ ...GRID, gridTemplateColumns: 'repeat(2,minmax(0,1fr))', mt: 0.5 }}>
          {summary.map(([label, value]) => <Box key={label} sx={{ border: '1px solid #d9dfe7', borderRadius: '2px', p: 0.8, minWidth: 0 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{value || (value === 0 ? 0 : '—')}</Typography></Box>)}
        </Box>
        {independent && <>
          <Typography variant="subtitle2" sx={{ mb: 1 }}>标准工作量计量归属</Typography>
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1 }}>匹配仅作为推荐，可在授权范围内重新选择；仪器筛选其关联方法。</Typography>
          {targets.length === 0 && <Alert severity="warning" sx={{ mb: 1.5 }}>{preview?.reason || '当前没有可录入的标准实验室与项目。请联系管理员维护授权及主数据，可先保存本机待关联草稿。'}</Alert>}
          <Box sx={GRID}>
            <TextField select size="small" label="计量部门" value={departmentId} disabled={saving || bindingBusy} onChange={event => { desiredBinding.current = null; pendingMethod.current = undefined; setDepartmentId(event.target.value === '' ? '' : Number(event.target.value)); setGroupId(''); setProjectId(''); setMethodId(''); setInstrumentId(''); }}>
              <MenuItem value="">请选择部门</MenuItem>{departments.map(target => <MenuItem key={target.division_id ?? 0} value={target.division_id ?? 0}>{target.department || '未指定部门'}</MenuItem>)}
            </TextField>
            <TextField select size="small" label="计量实验室" value={groupId} disabled={saving || departmentId === '' || bindingBusy} onChange={event => { desiredBinding.current = null; pendingMethod.current = undefined; setGroupId(event.target.value === '' ? '' : Number(event.target.value)); setProjectId(''); setMethodId(''); setInstrumentId(''); }}>
              <MenuItem value="">请选择实验室</MenuItem>{laboratories.map(target => <MenuItem key={target.group_id} value={target.group_id}>{target.laboratory}</MenuItem>)}
            </TextField>
            <TextField select size="small" label="计量项目" value={projectId} disabled={saving || !groupId || bindingBusy} sx={{ gridColumn: '1 / -1' }} onChange={event => {
              const id = event.target.value === '' ? '' : Number(event.target.value); setProjectId(id); setMethodId(''); setInstrumentId(''); pendingMethod.current = undefined; desiredBinding.current = null;
              if (groupId && id) void reloadBinding({ project_id: id, group_id: groupId });
            }}>
              <MenuItem value="">请选择项目</MenuItem>{projects.map(target => <MenuItem key={target.project_id} value={target.project_id}>{target.project_name} · 工作量系数 {target.coefficient}</MenuItem>)}
            </TextField>
          </Box>
          {bindingBusy && <Alert severity="info" sx={{ mb: 1.5 }}>正在加载该计量项目的可选方法与仪器，其他输入会保留。</Alert>}
          {selectedTarget && !bindingBusy && !bindingCurrent && <Button size="small" onClick={() => void reloadBinding(selectedTarget)} disabled={saving} sx={{ mb: 1 }}>重新加载计量选项</Button>}
          {selectedTarget && original && (original.project_id !== selectedTarget.project_id || original.group_id !== selectedTarget.group_id) && <Alert severity="info" sx={{ mb: 1.5 }}>工作量将归入所选标准实验室和项目；原登记名称及记录保持不变。</Alert>}
          {preview?.allow_custom && <TextField select fullWidth size="small" label="方法录入方式" value={customMode ? 'custom' : 'standard'} disabled={saving || bindingBusy || targets.length === 0} onChange={event => setCustom(event.target.value === 'custom')} sx={{ mb: 1.5 }}>
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
          {independent && bindingCurrent && <TextField select fullWidth size="small" label="仪器筛选" value={instrumentId} disabled={saving || bindingBusy} onChange={event => {
            const id = event.target.value === '' ? '' : Number(event.target.value); setInstrumentId(id);
            const filtered = filterWorkloadInstruments(instruments, id);
            if (!filtered.some(item => item.method_id === methodId)) { setMethodId(filtered.length === 1 ? filtered[0].method_id : ''); if (!multiplierEdited.current && filtered.length === 1) setMultiplier(filtered[0].multiplier); }
          }} sx={{ mb: 1.5 }}>
            <MenuItem value="">全部仪器</MenuItem>{instrumentOptions.map(item => <MenuItem key={item.instrument_id} value={item.instrument_id}>{item.instrument_id === 0 ? '未关联仪器' : item.instrument}</MenuItem>)}
          </TextField>}
          {(independent || instruments.length > 0) && <Autocomplete
            fullWidth size="small" autoHighlight options={bindingCurrent ? filteredMethods : []}
            value={bindingCurrent ? filteredMethods.find(item => item.method_id === methodId) ?? null : null}
            disabled={saving || bindingBusy || !bindingCurrent || filteredMethods.length === 0}
            isOptionEqualToValue={(option, value) => option.method_id === value.method_id}
            getOptionLabel={item => `${item.method_name || '未命名方法'} · ${item.instrument || '未关联仪器'}`}
            onChange={(_event, item) => {
              setMethodId(item?.method_id ?? '');
              if (item) { setMultiplier(item.multiplier); multiplierEdited.current = false; }
            }}
            renderOption={(props, item) => <Box component="li" {...props} key={item.method_id} sx={{ whiteSpace: 'normal', overflowWrap: 'anywhere' }}>{item.method_name || '未命名方法'} · {item.instrument || '未关联仪器'}{item.match_source ? `（${MATCH_LABEL[item.match_source]}）` : ''}</Box>}
            renderInput={params => <TextField {...params} label="检测方法 / 关联仪器" placeholder="输入方法名称或仪器搜索" helperText="从现有标准方法中选择；输入搜索文字不会自动建立关联" />}
            noOptionsText="没有匹配的方法" openText="展开方法" closeText="收起方法" clearText="清除所选方法" sx={{ mb: 1.5 }}
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
