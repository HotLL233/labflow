import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Alert, Box, Button, Checkbox, Chip, Dialog, DialogActions, DialogContent, DialogTitle, FormControl, FormControlLabel, InputLabel, LinearProgress, MenuItem, Paper, Select, Stack, Table, TableBody, TableCell, TableHead, TablePagination, TableRow, TextField, Typography, useMediaQuery, useTheme } from '@mui/material';
import { useUser } from '../UserContext';
import { requireApiSuccess } from '../api/client';
import { confirmPersonnelSummaryImport, createPersonnelChangeNotice, createPersonnelSummaryRow, deletePersonnelChangeNotice, deletePersonnelSummaryRow, exportPersonnelChangeSummary, getPersonnelChangeNotices, getPersonnelChangeOptions, getPersonnelChangePersonProjects, previewPersonnelSummaryImport, revisePersonnelChangeNotice, reviewPersonnelChangeNotice, updatePersonnelSummaryRow, withdrawPersonnelChangeNotice, type PersonnelChangeNotice, type PersonnelChangeNoticeInput, type PersonnelChangeOptions, type PersonnelChangeSummaryRow, type PersonnelFeedbackField, type PersonnelNoticeQuery, type PersonnelSummaryImportPreview } from '../api/personnelChange';
import { applyPersonnelProjectDefaults, changePersonnelForm, initialPersonnelForm, parsePersonnelChoices, PERSONNEL_BUILTIN_FIELDS, personnelFieldValue, personnelFormFields, personnelIdentity, personnelSubmission, personnelSummaryChanges, validatePersonnelForm } from '../utils/personnelChangeForm';
import { validateRecordDateRange } from '../utils/recordDateRange';

const cellSx = { border: '1px solid', borderColor: 'divider', p: 1, minWidth: 0, overflowWrap: 'anywhere', verticalAlign: 'top' } as const;
const actionSx = { minHeight: 30, minWidth: 56, px: 1, borderRadius: '2px', whiteSpace: 'nowrap' } as const;
const emptyOptions: PersonnelChangeOptions = { labs: [], summary_labs: [], summary: [], summary_revision: '', lab_projects: {}, change_types: [], change_type_configs: [], fields: [], summary_available: false, capabilities: { can_view_summary: false, can_edit_summary: false, can_view_notices: false, can_submit: false, can_review: false, can_import_summary: false, can_export_summary: false, can_edit_config: false, view_all_summary: false, edit_all_summary: false } };
const errorText = (error: unknown) => error instanceof Error ? error.message : '操作失败，请重试';
const projectText = (notice: PersonnelChangeNotice) => [notice.new_project_code, notice.source_project_codes.length ? `来源：${notice.source_project_codes.join('、')}` : '', notice.project_codes.join('、'), notice.target_project_codes.length ? `目标：${notice.target_project_codes.join('、')}` : '', notice.method_names.join('、')].filter(Boolean).join('；') || '未填写项目/方法';
const summaryText = (row: PersonnelChangeSummaryRow) => [row.lab_name, row.project_code, row.leader_name, row.person_names.join('、'), row.methods.join('、')].join(' ');
const defaultQuery = { start: '', end: '', status: '', keyword: '' };
type NoticeAction = { notice: PersonnelChangeNotice; kind: 'approve' | 'reject' | 'withdraw' | 'delete'; summaryRevision: string };
type SummaryEditor = { key?: string; row: Omit<PersonnelChangeSummaryRow, 'row_key'>; revision: string; deleting: boolean };

function PersonnelChangePage() {
  const navigate = useNavigate();
  const theme = useTheme();
  const mobile = useMediaQuery(theme.breakpoints.down('sm'));
  const [searchParams, setSearchParams] = useSearchParams();
  const targetNoticeNo = searchParams.has('notice_no') ? searchParams.get('notice_no')! : undefined;
  const activeTarget = useRef(targetNoticeNo);
  const routeGeneration = useRef(0);
  if (activeTarget.current !== targetNoticeNo) { activeTarget.current = targetNoticeNo; routeGeneration.current++; }
  const [options, setOptions] = useState<PersonnelChangeOptions>(emptyOptions);
  const [notices, setNotices] = useState<PersonnelChangeNotice[]>([]);
  const [form, setForm] = useState<PersonnelChangeNoticeInput>(initialPersonnelForm);
  const formRef = useRef(form); formRef.current = form;
  const [revisionTarget, setRevisionTarget] = useState<PersonnelChangeNotice | null>(null);
  const [revisionReason, setRevisionReason] = useState('');
  const [existingProjects, setExistingProjects] = useState<string[]>([]);
  const [personLoading, setPersonLoading] = useState(false);
  const [personError, setPersonError] = useState('');
  const projectTouched = useRef(false);
  const personRequest = useRef(0);
  const [query, setQuery] = useState(defaultQuery);
  const [activeQuery, setActiveQuery] = useState(defaultQuery);
  const [noticePage, setNoticePage] = useState(0);
  const [noticePageSize, setNoticePageSize] = useState(20);
  const [summaryQuery, setSummaryQuery] = useState('');
  const [summaryPage, setSummaryPage] = useState(0);
  const [summaryPageSize, setSummaryPageSize] = useState(20);
  const [detail, setDetail] = useState<PersonnelChangeNotice | null>(null);
  const [noticeAction, setNoticeAction] = useState<NoticeAction | null>(null);
  const [actionReason, setActionReason] = useState('');
  const [summaryEditor, setSummaryEditor] = useState<SummaryEditor | null>(null);
  const [summaryReason, setSummaryReason] = useState('');
  const [importPreview, setImportPreview] = useState<PersonnelSummaryImportPreview | null>(null);
  const [importReason, setImportReason] = useState('');
  const [importPage, setImportPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [needsRefresh, setNeedsRefresh] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const mounted = useRef(true);
  const loadRequest = useRef(0);
  const loadController = useRef<AbortController | null>(null);
  const operationPending = useRef(false);
  const capabilities = options.capabilities;
  const writeDisabled = busy || loading || needsRefresh;
  const fields = useMemo(() => personnelFormFields(form, options.fields), [form, options.fields]);
  const projects = options.lab_projects[form.lab_name] || [];
  const targetProjects = options.lab_projects[form.target_lab_name || form.lab_name] || [];
  const summaryRows = useMemo(() => options.summary.filter(row => summaryText(row).toLowerCase().includes(summaryQuery.trim().toLowerCase())), [options.summary, summaryQuery]);
  const pageNotices = notices.slice(noticePage * noticePageSize, (noticePage + 1) * noticePageSize);
  const safeSummaryPage = Math.min(summaryPage, Math.max(0, Math.ceil(summaryRows.length / summaryPageSize) - 1));
  const pageSummary = summaryRows.slice(safeSummaryPage * summaryPageSize, (safeSummaryPage + 1) * summaryPageSize);
  const importChanges = useMemo(() => importPreview ? personnelSummaryChanges(options.summary, importPreview.rows) : [], [importPreview, options.summary]);

  const load = async (nextQuery = activeQuery) => {
    const request = ++loadRequest.current;
    const generation = routeGeneration.current;
    loadController.current?.abort(); const controller = new AbortController(); loadController.current = controller;
    const fresh = () => mounted.current && generation === routeGeneration.current && request === loadRequest.current && !controller.signal.aborted;
    setLoading(true); setError('');
    try {
      const next = requireApiSuccess(await getPersonnelChangeOptions(controller.signal), '人员变动配置加载失败');
      if (!fresh()) return;
      if (!next) throw new Error('人员变动配置为空');
      setOptions(next);
      setForm(current => {
        if (current.summary_revision) return current;
        const type = next.change_type_configs.find(item => item.enabled && item.action_key === current.action_key) || next.change_type_configs.find(item => item.enabled);
        return { ...current, action_key: type?.action_key || current.action_key, change_type: type?.label || current.change_type, summary_revision: next.summary_revision, lab_name: current.lab_name || next.labs[0]?.name || '', target_lab_name: current.target_lab_name || next.labs[0]?.name || '' };
      });
      const params: PersonnelNoticeQuery = targetNoticeNo !== undefined ? { notice_no: targetNoticeNo } : { days: 30, ...Object.fromEntries(Object.entries(nextQuery).filter(([, value]) => value.trim())) };
      const list = next.capabilities.can_view_notices ? requireApiSuccess(await getPersonnelChangeNotices(params, controller.signal), '通知记录加载失败') || [] : [];
      if (!fresh()) return;
      setNotices(list); setNoticePage(0); setNeedsRefresh(false);
      if (targetNoticeNo !== undefined) { const target = list.find(item => item.notice_no === targetNoticeNo); setDetail(target || null); if (!target) setError('该通知不存在、已删除或当前账号无权查看'); }
    } catch (failure) { if (fresh()) { setNeedsRefresh(true); setError(errorText(failure)); } }
    finally { if (fresh()) setLoading(false); }
  };

  useEffect(() => {
    mounted.current = true; setNotices([]); setDetail(null); setNoticeAction(null); setSummaryEditor(null); setImportPreview(null); void load();
    return () => { mounted.current = false; loadRequest.current++; personRequest.current++; loadController.current?.abort(); };
  }, [targetNoticeNo]);

  useEffect(() => {
    const identity = personnelIdentity(form);
    const request = ++personRequest.current;
    const controller = new AbortController();
    setExistingProjects([]); setPersonError('');
    if (!capabilities.can_submit || !form.lab_name || !form.person_name?.trim() || !['employee_join', 'employee_leave', 'internal_transfer', 'transfer_out', 'transfer_in'].includes(form.action_key)) { setPersonLoading(false); return; }
    setPersonLoading(true);
    const timer = setTimeout(() => {
      void getPersonnelChangePersonProjects(form.lab_name, form.person_name!.trim(), controller.signal).then(response => {
        if (!mounted.current || request !== personRequest.current || controller.signal.aborted || personnelIdentity(formRef.current) !== identity) return;
        const personProjects = requireApiSuccess(response, '人员项目查询失败') || [];
        setExistingProjects(personProjects);
        setForm(current => applyPersonnelProjectDefaults(current, identity, personProjects, projectTouched.current));
      }).catch(failure => { if (mounted.current && request === personRequest.current && !controller.signal.aborted) setPersonError(errorText(failure)); })
        .finally(() => { if (mounted.current && request === personRequest.current && !controller.signal.aborted) setPersonLoading(false); });
    }, 250);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [capabilities.can_submit, form.action_key, form.lab_name, form.person_name]);

  const updateForm = (patch: Partial<PersonnelChangeNoticeInput>, projectChoice = false) => {
    if (busy) return;
    const next = changePersonnelForm(formRef.current, patch);
    if (personnelIdentity(next) !== personnelIdentity(formRef.current)) { personRequest.current++; projectTouched.current = false; setExistingProjects([]); }
    if (projectChoice) projectTouched.current = true;
    formRef.current = next; setForm(next);
  };
  const resetForm = () => {
    personRequest.current++; projectTouched.current = false; setRevisionTarget(null); setRevisionReason('');
    const type = options.change_type_configs.find(item => item.enabled);
    const next = { ...initialPersonnelForm(), action_key: type?.action_key || 'employee_join', change_type: type?.label || '', summary_revision: options.summary_revision, lab_name: options.labs[0]?.name || '', target_lab_name: options.labs[0]?.name || '' };
    formRef.current = next; setForm(next);
  };
  const runMutation = async (task: () => Promise<unknown>, success: string, onSuccess: () => void = () => {}) => {
    if (operationPending.current || needsRefresh || loading) return;
    operationPending.current = true; setBusy(true); setError(''); setMessage('');
    const generation = routeGeneration.current;
    const fresh = () => mounted.current && generation === routeGeneration.current && activeTarget.current === targetNoticeNo;
    try {
      await task(); if (!fresh()) return;
      onSuccess(); setMessage(success); await load();
    } catch (failure) { if (fresh()) { setNeedsRefresh(true); setError(`${errorText(failure)}。请刷新核对操作结果后继续，系统不会自动重试。`); } }
    finally { operationPending.current = false; if (mounted.current) setBusy(false); }
  };
  const submit = async () => {
    if (!capabilities.can_submit || operationPending.current) return;
    const data = personnelSubmission(form, options.fields);
    const invalid = validatePersonnelForm(data, options.fields);
    if (invalid) { setError(invalid); return; }
    if (form.summary_revision !== options.summary_revision) { setError('汇总表已更新，请重置表单或重新打开修订记录后再提交'); return; }
    if (revisionTarget && !revisionReason.trim()) { setError('请填写修订原因'); return; }
    await runMutation(async () => {
      requireApiSuccess(await (revisionTarget ? revisePersonnelChangeNotice(revisionTarget.notice_no, { ...data, reason: revisionReason.trim(), notice_revision: revisionTarget.revision }) : createPersonnelChangeNotice(data)), '通知提交失败');
    }, revisionTarget ? '修订已重新提交审批，通知编号不变' : '通知已提交审批；尚未同步汇总表', () => { personRequest.current++; setRevisionTarget(null); setRevisionReason(''); projectTouched.current = false; setForm(initialPersonnelForm()); });
  };
  const startRevision = (notice: PersonnelChangeNotice) => {
    if (!notice.can_revise || writeDisabled) return;
    personRequest.current++; projectTouched.current = true; setDetail(null); setRevisionTarget(notice); setRevisionReason('');
    const next = { ...initialPersonnelForm(), action_key: notice.action_key, change_type: notice.change_type, summary_revision: options.summary_revision,
      lab_name: notice.lab_name, person_name: notice.person_name, project_codes: [...notice.project_codes], source_project_codes: [...notice.source_project_codes],
      target_lab_name: notice.target_lab_name, target_project_codes: [...notice.target_project_codes], transfer_notice_no: notice.transfer_notice_no,
      method_names: [...notice.method_names], new_project_code: notice.new_project_code, is_high_tech: notice.is_high_tech, high_tech_name: notice.high_tech_name,
      effective_at: notice.effective_at, notes: notice.notes, extra_fields: { ...notice.extra_fields } };
    formRef.current = next; setForm(next); setError(''); window.scrollTo({ top: 0, behavior: 'smooth' });
  };
  const confirmNoticeAction = async () => {
    if (!noticeAction) return;
    const { notice, kind, summaryRevision } = noticeAction;
    if (kind !== 'approve' && !actionReason.trim()) { setError('请填写本次操作原因'); return; }
    if (summaryRevision !== options.summary_revision && (kind === 'approve' || kind === 'reject')) { setError('汇总表已更新，请关闭后重新复核审批'); return; }
    await runMutation(async () => {
      if (kind === 'withdraw') requireApiSuccess(await withdrawPersonnelChangeNotice(notice.notice_no, { reason: actionReason.trim(), notice_revision: notice.revision }), '撤回失败');
      else if (kind === 'delete') requireApiSuccess(await deletePersonnelChangeNotice(notice.notice_no, { reason: actionReason.trim(), notice_revision: notice.revision }), '删除失败');
      else requireApiSuccess(await reviewPersonnelChangeNotice(notice.notice_no, { decision: kind, reason: actionReason.trim(), summary_revision: summaryRevision, notice_revision: notice.revision }), '审批失败');
    }, kind === 'approve' ? (notice.action_key === 'notice_only' ? '通知已通过；该类型不修改汇总表' : '审批已通过，汇总表已同步并备份') : kind === 'reject' ? '通知已驳回，发起人可以修订重提' : kind === 'withdraw' ? '通知已撤回，可以修订后重新提交' : '通知已删除；已同步的汇总关系未回滚', () => { setNoticeAction(null); setDetail(null); });
  };
  const openSummary = (row?: PersonnelChangeSummaryRow, deleting = false) => {
    if (!capabilities.can_edit_summary || writeDisabled) return;
    setSummaryReason(''); setSummaryEditor({ key: row?.row_key, row: row ? { ...row, person_names: [...row.person_names], methods: [...row.methods] } : { lab_name: options.summary_labs[0]?.name || '', leader_name: '', person_names: [], project_code: '', methods: [] }, revision: options.summary_revision, deleting });
  };
  const saveSummary = async () => {
    if (!summaryEditor || !capabilities.can_edit_summary) return;
    if (!summaryReason.trim()) { setError('请填写汇总表维护原因'); return; }
    if (!summaryEditor.row.lab_name || !summaryEditor.row.project_code.trim()) { setError('请填写实验室和项目代号'); return; }
    if (summaryEditor.revision !== options.summary_revision) { setError('汇总表已更新，请关闭后重新打开维护窗口'); return; }
    const data = { summary_revision: summaryEditor.revision, reason: summaryReason.trim(), row: summaryEditor.row };
    await runMutation(async () => {
      requireApiSuccess(await (summaryEditor.deleting && summaryEditor.key ? deletePersonnelSummaryRow(summaryEditor.key, data) : summaryEditor.key ? updatePersonnelSummaryRow(summaryEditor.key, data) : createPersonnelSummaryRow(data)), '汇总维护失败');
    }, '汇总表已保存并保留变更记录', () => setSummaryEditor(null));
  };
  const prepareImport = async (file: File | null) => {
    if (!file || !capabilities.can_import_summary || operationPending.current || writeDisabled) return;
    operationPending.current = true; setBusy(true); setError('');
    const generation = routeGeneration.current;
    const fresh = () => mounted.current && generation === routeGeneration.current && activeTarget.current === targetNoticeNo;
    try { const result = requireApiSuccess(await previewPersonnelSummaryImport(file, options.summary_revision), '导入预览失败'); if (!fresh()) return; if (!result) throw new Error('导入预览为空'); setImportPreview(result); setImportReason(''); setImportPage(0); }
    catch (failure) { if (fresh()) setError(`${errorText(failure)}；尚未执行导入`); }
    finally { operationPending.current = false; if (mounted.current) setBusy(false); }
  };
  const applyImport = async () => {
    if (!importPreview || !capabilities.can_import_summary || !importReason.trim()) return;
    if (importPreview.summary_revision !== options.summary_revision || !Number.isFinite(Date.parse(importPreview.expires_at)) || Date.parse(importPreview.expires_at) <= Date.now()) { setError('导入预览已过期或汇总表已变化，请重新上传预览'); return; }
    await runMutation(async () => { requireApiSuccess(await confirmPersonnelSummaryImport({ preview_id: importPreview.preview_id, summary_revision: importPreview.summary_revision, reason: importReason.trim() }), '确认导入失败'); }, '汇总表导入完成并已备份', () => setImportPreview(null));
  };
  const searchNotices = () => {
    if ((query.start || query.end) && validateRecordDateRange(query.start, query.end)) { setError(validateRecordDateRange(query.start, query.end)); return; }
    setActiveQuery(query); void load(query);
  };
  const fieldLabel = (key: string, fallback: string) => options.fields.find(field => field.key === key)?.label || fallback;
  const setField = (field: PersonnelFeedbackField, value: string | string[] | boolean) => {
    if (PERSONNEL_BUILTIN_FIELDS.some(([key]) => key === field.key)) updateForm({ [field.key]: value } as Partial<PersonnelChangeNoticeInput>, ['project_codes', 'source_project_codes', 'target_project_codes'].includes(field.key));
    else updateForm({ extra_fields: { ...formRef.current.extra_fields, [field.key]: Array.isArray(value) ? JSON.stringify(value) : String(value) } });
  };
  const renderFormField = (field: PersonnelFeedbackField) => {
    const value = personnelFieldValue(form, field.key);
    const disabled = writeDisabled;
    const label = field.key === 'lab_name' && form.action_key === 'transfer_in' ? fieldLabel('target_lab_name', '目标实验室') : field.label;
    if (field.key === 'change_type') return <FormControl key={field.key} size="small" fullWidth required disabled={disabled}><InputLabel>{label}</InputLabel><Select label={label} value={form.change_type} onChange={event => { const type = options.change_type_configs.find(item => item.label === event.target.value); if (type) updateForm({ action_key: type.action_key, change_type: type.label }); }}>{options.change_type_configs.filter(type => type.enabled).map(type => <MenuItem key={`${type.action_key}:${type.label}`} value={type.label}>{type.label}{type.action_key === 'notice_only' ? '（仅通知）' : ''}</MenuItem>)}</Select></FormControl>;
    if (field.key === 'lab_name' || field.key === 'target_lab_name') return <FormControl key={field.key} size="small" fullWidth required={field.required} disabled={disabled}><InputLabel>{label}</InputLabel><Select label={label} value={form.action_key === 'transfer_in' ? form.target_lab_name || '' : String(value)} onChange={event => { const lab = String(event.target.value); updateForm(form.action_key === 'transfer_in' ? { lab_name: lab, target_lab_name: lab } : { [field.key]: lab }); }}>{options.labs.map(lab => <MenuItem key={lab.id} value={lab.name}>{lab.name}</MenuItem>)}</Select></FormControl>;
    if (['project_codes', 'source_project_codes', 'target_project_codes'].includes(field.key) || field.field_type === 'select' || field.field_type === 'multi-select') {
      const multiple = field.field_type === 'multi-select' || field.key !== 'change_type' && ['project_codes', 'source_project_codes', 'target_project_codes'].includes(field.key);
      const choices = field.key === 'target_project_codes' ? (form.action_key === 'transfer_in' ? targetProjects : projects) : ['project_codes', 'source_project_codes'].includes(field.key) ? projects : field.options || [];
      const selected = multiple ? (Array.isArray(value) ? value : parsePersonnelChoices(String(value))) : String(value);
      return <FormControl key={field.key} size="small" fullWidth required={field.required} disabled={disabled}><InputLabel>{label}</InputLabel><Select label={label} multiple={multiple} value={selected} onChange={event => setField(field, multiple ? event.target.value as string[] : String(event.target.value))} renderValue={current => Array.isArray(current) ? current.join('、') : current}>{!multiple && !field.required && <MenuItem value="">不选择</MenuItem>}{choices.map(choice => <MenuItem key={choice} value={choice}>{multiple && <Checkbox size="small" checked={Array.isArray(selected) && selected.includes(choice)} />}{choice}</MenuItem>)}</Select></FormControl>;
    }
    if (field.field_type === 'boolean') {
      if (field.key === 'is_high_tech') return <FormControlLabel key={field.key} control={<Checkbox checked={Boolean(value)} disabled={disabled} onChange={event => setField(field, event.target.checked)} />} label={label} />;
      return <FormControl key={field.key} size="small" fullWidth required={field.required} disabled={disabled}><InputLabel>{label}</InputLabel><Select label={label} value={String(value)} onChange={event => setField(field, String(event.target.value))}><MenuItem value="">请选择</MenuItem><MenuItem value="true">是</MenuItem><MenuItem value="false">否</MenuItem></Select></FormControl>;
    }
    if (field.field_type === 'multi-text' || field.key === 'method_names') {
      const values = Array.isArray(value) ? value : parsePersonnelChoices(String(value));
      return <Box key={field.key} sx={{ minWidth: 0, border: '1px solid', borderColor: 'divider', p: 1, borderRadius: '2px' }}><Typography variant="body2" fontWeight={600}>{label}{field.required ? ' *' : ''}</Typography><Stack spacing={0.75} sx={{ mt: 0.75 }}>{values.map((item, index) => <Box key={index} sx={{ display: 'flex', gap: 0.5 }}><TextField size="small" fullWidth value={item} inputProps={{ 'aria-label': `${label} ${index + 1}` }} disabled={disabled} onChange={event => setField(field, values.map((current, i) => i === index ? event.target.value : current))} /><Button size="small" color="error" disabled={disabled} onClick={() => setField(field, values.filter((_, i) => i !== index))}>移除</Button></Box>)}</Stack><Button size="small" disabled={disabled} onClick={() => setField(field, [...values, ''])}>添加一条</Button><Typography variant="caption" color="text.secondary" display="block">逐条填写；检测方法不要在一条内用逗号、顿号、分号或换行分隔。</Typography></Box>;
    }
    return <TextField key={field.key} size="small" fullWidth label={label} required={field.required} value={String(value)} disabled={disabled} type={field.field_type === 'date' ? 'date' : 'text'} multiline={field.field_type === 'textarea'} minRows={field.field_type === 'textarea' ? 2 : undefined} InputLabelProps={field.field_type === 'date' ? { shrink: true } : undefined} onChange={event => setField(field, event.target.value)} helperText={field.key === 'effective_at' ? '用于记录此次变动日期，不是定时生效；审批通过即同步。' : field.key === 'transfer_notice_no' ? '必填已同步的调出通知编号；人员须一致且来源与目标实验室不同，每条调出只能迁入一次。外部新员工请用员工加入。' : undefined} />;
  };
  const startNoticeAction = (notice: PersonnelChangeNotice, kind: NoticeAction['kind']) => { setDetail(null); setNoticeAction({ notice, kind, summaryRevision: options.summary_revision }); setActionReason(''); };
  const noticeButtons = (notice: PersonnelChangeNotice, includeDetail = true) => <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
    {includeDetail && <Button size="small" variant="outlined" sx={actionSx} onClick={() => setDetail(notice)}>详情</Button>}
    {notice.can_review && <><Button size="small" variant="outlined" sx={actionSx} disabled={writeDisabled} onClick={() => startNoticeAction(notice, 'approve')}>通过</Button><Button size="small" variant="outlined" color="error" sx={actionSx} disabled={writeDisabled} onClick={() => startNoticeAction(notice, 'reject')}>驳回</Button></>}
    {notice.can_revise && <Button size="small" variant="outlined" sx={actionSx} disabled={writeDisabled} onClick={() => startRevision(notice)}>修订重提</Button>}
    {notice.can_withdraw && <Button size="small" variant="outlined" color="warning" sx={actionSx} disabled={writeDisabled} onClick={() => startNoticeAction(notice, 'withdraw')}>撤回</Button>}
    {capabilities.can_review && notice.review_status !== '已删除' && <Button size="small" variant="outlined" color="error" sx={actionSx} disabled={writeDisabled} onClick={() => startNoticeAction(notice, 'delete')}>删除</Button>}
  </Box>;
  const noticeStatus = (notice: PersonnelChangeNotice) => <Stack spacing={0.5}><Chip size="small" sx={{ alignSelf: 'flex-start', maxWidth: '100%' }} label={`审批/汇总：${notice.review_status || '待审批'}`} color={['已同步', '已通过'].includes(notice.review_status) ? 'success' : notice.review_status === '已驳回' ? 'error' : 'default'} /><Typography variant="caption" sx={{ overflowWrap: 'anywhere' }}>通知投递：{notice.delivery_status || '待核对'}</Typography>{notice.review_reason && <Typography variant="caption" color="text.secondary" sx={{ overflowWrap: 'anywhere' }}>{notice.review_reason}</Typography>}</Stack>;
  const dataCell = (label: string, value: React.ReactNode, span = false) => <Box sx={{ ...cellSx, gridColumn: span ? '1 / -1' : undefined, py: 0.75 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Box sx={{ fontSize: 14, overflowWrap: 'anywhere', maxHeight: 110, overflow: 'auto', whiteSpace: 'pre-wrap' }}>{value || '—'}</Box></Box>;
  const detailValues: [string, React.ReactNode][] = detail ? [
    ['通知编号', detail.notice_no], ['提交人', detail.actor_username], ['创建时间', detail.created_at], ['变动类型', detail.change_type],
    [fieldLabel('lab_name', '实验室'), detail.lab_name], [fieldLabel('person_name', '人员'), detail.person_name],
    [fieldLabel('target_lab_name', '目标实验室'), detail.target_lab_name], [fieldLabel('project_codes', '关联项目'), detail.project_codes.join('、')],
    [fieldLabel('source_project_codes', '来源项目'), detail.source_project_codes.join('、')], [fieldLabel('target_project_codes', '目标项目'), detail.target_project_codes.join('、')],
    [fieldLabel('transfer_notice_no', '关联调出通知'), detail.transfer_notice_no], [fieldLabel('new_project_code', '新增项目代号'), detail.new_project_code],
    ['新增项目名称', detail.new_project_name], [fieldLabel('method_names', '检测方法'), detail.method_names.join('\n')],
    [fieldLabel('effective_at', '记录日期'), detail.effective_at], [fieldLabel('is_high_tech', '高新项目'), detail.is_high_tech ? '是' : '否'],
    [fieldLabel('high_tech_name', '高新项目名称'), detail.high_tech_name], [fieldLabel('notes', '备注'), detail.notes],
    ['审批/汇总状态', detail.review_status], ['通知投递状态', detail.delivery_status], ['审批人', detail.reviewed_by], ['审批时间', detail.reviewed_at], ['审批说明', detail.review_reason], ['更新时间', detail.updated_at],
    ...Object.entries(detail.extra_fields).map(([key, value]): [string, React.ReactNode] => [fieldLabel(key, key), value === 'true' ? '是' : value === 'false' ? '否' : value.startsWith('[') ? parsePersonnelChoices(value).join('、') : value]),
  ] : [];

  return <Box sx={{ minWidth: 0 }}>
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, alignItems: 'center', mb: 1.5 }}><Button className="labflow-back-button" size="small" variant="outlined" onClick={() => navigate(-1)}>返回上一页</Button><Typography variant="h5" fontWeight={800} sx={{ flex: '1 1 180px' }}>人员变动通知</Typography><Button size="small" variant="outlined" disabled={busy || loading} onClick={() => void load()}>刷新</Button></Box>
    <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>通知与项目人员汇总独立管理；审批通过后同步汇总，不直接修改程序的用户、项目或方法主数据。</Typography>
    {targetNoticeNo !== undefined && <Alert severity="info" sx={{ mb: 1.5 }} action={<Button size="small" onClick={() => { const next = new URLSearchParams(searchParams); next.delete('notice_no'); setSearchParams(next, { replace: true }); }}>返回通知列表</Button>}>正在查看关联通知 {targetNoticeNo}，不受日期筛选限制。</Alert>}
    {error && <Alert severity="error" sx={{ mb: 1.5 }}>{error}</Alert>}
    {message && <Alert severity="success" sx={{ mb: 1.5 }} onClose={() => setMessage('')}>{message}</Alert>}
    {needsRefresh && <Alert severity="warning" sx={{ mb: 1.5 }}>操作结果或数据版本需重新核对，写入已暂停。请刷新后重新打开对应操作窗口。</Alert>}
    {(loading || busy) && <LinearProgress sx={{ mb: 1.5 }} />}
    {!loading && !options.summary_available && <Alert severity="info" sx={{ mb: 1.5 }}>尚无项目人员汇总。可由有权限的组长新增记录，或由管理员、分析组长预览并导入汇总表。</Alert>}
    {capabilities.can_submit && <Paper variant="outlined" sx={{ p: { xs: 1.25, sm: 2 }, mb: 2 }}>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, alignItems: 'center', mb: 1.25 }}><Typography fontWeight={800} sx={{ flex: 1 }}>{revisionTarget ? `修订通知 ${revisionTarget.notice_no}` : '发起通知'}</Typography><Button size="small" variant="outlined" disabled={busy} onClick={resetForm}>{revisionTarget ? '取消修订' : '重置表单'}</Button></Box>
      {form.summary_revision && form.summary_revision !== options.summary_revision && <Alert severity="warning" sx={{ mb: 1 }}>汇总表已更新，请重置表单，或重新打开待修订通知再提交。</Alert>}
      {form.action_key === 'notice_only' && <Alert severity="info" sx={{ mb: 1 }}>此类型仅通知与审批，审批通过不会调整项目人员汇总。</Alert>}
      {form.action_key === 'transfer_out' && <Alert severity="info" sx={{ mb: 1 }}>调出只移除当前实验室关系；目标实验室由其组长另行提交调入，须填写本通知编号建立关联。</Alert>}
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', md: 'repeat(2,minmax(0,1fr))' }, gap: 1.25 }}>{fields.map(renderFormField)}</Box>
      {form.person_name?.trim() && <Typography variant="caption" color={personError ? 'error' : 'text.secondary'} display="block" sx={{ mt: 1 }}>{personLoading ? '正在查询此人员的项目；手动勾选不会被查询结果覆盖。' : personError || (existingProjects.length ? `已有关联项目：${existingProjects.join('、')}` : '未匹配到已有关联项目，请核对姓名与实验室；不会套用其他人员的项目。')}</Typography>}
      {revisionTarget && <TextField fullWidth size="small" required multiline minRows={2} label="修订原因" value={revisionReason} disabled={writeDisabled} onChange={event => setRevisionReason(event.target.value)} inputProps={{ maxLength: 500 }} sx={{ mt: 1.25 }} />}
      <Box sx={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'flex-end', gap: 1, mt: 1.5 }}><Button variant="contained" disabled={writeDisabled || personLoading || Boolean(personError)} onClick={() => void submit()}>{busy ? '处理中…' : revisionTarget ? '修订后重新提交审批' : '提交审批'}</Button></Box>
    </Paper>}

    {capabilities.can_view_notices && <Paper variant="outlined" sx={{ p: { xs: 1.25, sm: 2 }, mb: 2 }}>
      <Typography fontWeight={800} sx={{ mb: 1 }}>通知记录</Typography>
      {targetNoticeNo === undefined && <><Box component="form" onSubmit={event => { event.preventDefault(); searchNotices(); }} sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2,minmax(0,1fr))', md: 'repeat(2,minmax(120px,1fr)) minmax(100px,1fr) minmax(150px,1.5fr) auto' }, gap: 1, mb: 1 }}>
        <TextField size="small" type="date" label="开始日期" InputLabelProps={{ shrink: true }} value={query.start} disabled={busy} onChange={event => setQuery(current => ({ ...current, start: event.target.value }))} />
        <TextField size="small" type="date" label="结束日期" InputLabelProps={{ shrink: true }} value={query.end} disabled={busy} onChange={event => setQuery(current => ({ ...current, end: event.target.value }))} />
        <FormControl size="small"><InputLabel>审批状态</InputLabel><Select label="审批状态" value={query.status} disabled={busy} onChange={event => setQuery(current => ({ ...current, status: String(event.target.value) }))}><MenuItem value="">全部状态</MenuItem>{['待审批', '已同步', '已通过', '已驳回', '已撤回', '已删除'].map(status => <MenuItem key={status} value={status}>{status}</MenuItem>)}</Select></FormControl>
        <TextField size="small" label="人员 / 实验室 / 通知编号" value={query.keyword} disabled={busy} inputProps={{ maxLength: 100 }} onChange={event => setQuery(current => ({ ...current, keyword: event.target.value }))} />
        <Button type="submit" variant="outlined" disabled={busy || loading} sx={{ minHeight: 40 }}>查询</Button>
      </Box><Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 1.25 }}>不指定日期时显示近 30 天记录，并包含所有待审批记录。可按日期、状态和人员查询历史。</Typography></>}
      {notices.length === 0 ? <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>当前范围没有通知记录。</Typography> : mobile ? <Stack spacing={1}>{pageNotices.map(notice => <Paper key={notice.notice_no} variant="outlined" sx={{ p: 1, borderRadius: '2px', minWidth: 0 }}>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.75, alignItems: 'center', mb: 0.75 }}><Typography variant="body2" fontWeight={800} sx={{ overflowWrap: 'anywhere' }}>{notice.notice_no}</Typography><Typography variant="caption" color="text.secondary">{notice.change_type}</Typography></Box>
        <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 0.5, mb: 0.75 }}>{dataCell(fieldLabel('lab_name', '实验室'), notice.lab_name)}{dataCell(fieldLabel('person_name', '人员'), notice.person_name || notice.actor_username)}{dataCell('项目 / 方法', projectText(notice), true)}{dataCell(fieldLabel('effective_at', '记录日期'), notice.effective_at)}{dataCell('提交人', notice.actor_username)}</Box>
        <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>创建：{notice.created_at}</Typography>{noticeStatus(notice)}<Box sx={{ mt: 0.75 }}>{noticeButtons(notice)}</Box>
      </Paper>)}</Stack> : <Table size="small" sx={{ tableLayout: 'fixed', width: '100%', '& th, & td': cellSx }}><TableHead><TableRow><TableCell sx={{ width: '23%' }}>通知 / 类型 / 人员</TableCell><TableCell sx={{ width: '27%' }}>实验室 / 项目方法</TableCell><TableCell sx={{ width: '14%' }}>记录日期</TableCell><TableCell sx={{ width: '18%' }}>审批与通知状态</TableCell><TableCell>操作</TableCell></TableRow></TableHead><TableBody>{pageNotices.map(notice => <TableRow key={notice.notice_no}><TableCell><Typography variant="body2" fontWeight={700}>{notice.notice_no}</Typography><Typography variant="body2">{notice.change_type} · {notice.person_name || '—'}</Typography><Typography variant="caption" color="text.secondary">提交：{notice.actor_username}</Typography></TableCell><TableCell><Typography variant="body2" fontWeight={600}>{notice.lab_name}{notice.target_lab_name && notice.target_lab_name !== notice.lab_name ? ` → ${notice.target_lab_name}` : ''}</Typography><Typography variant="body2">{projectText(notice)}</Typography></TableCell><TableCell>{notice.effective_at || '—'}</TableCell><TableCell>{noticeStatus(notice)}</TableCell><TableCell>{noticeButtons(notice)}</TableCell></TableRow>)}</TableBody></Table>}
      <TablePagination component="div" count={notices.length} page={noticePage} rowsPerPage={noticePageSize} rowsPerPageOptions={[20, 50, 100]} onPageChange={(_, page) => setNoticePage(page)} onRowsPerPageChange={event => { setNoticePageSize(Number(event.target.value)); setNoticePage(0); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap' }, '& .MuiTablePagination-spacer': { flex: '0 0 auto' } }} />
    </Paper>}
    {capabilities.can_view_summary && <Paper variant="outlined" sx={{ p: { xs: 1.25, sm: 2 } }}>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 1, mb: 1 }}><Typography fontWeight={800} sx={{ flex: '1 1 180px' }}>项目及人员汇总表</Typography>
        {capabilities.can_edit_summary && <Button size="small" variant="outlined" disabled={writeDisabled} onClick={() => openSummary()}>新增记录</Button>}
        {capabilities.can_import_summary && <Button size="small" variant="outlined" component="label" disabled={writeDisabled}>导入预览<input hidden type="file" accept=".xlsx" onChange={event => { void prepareImport(event.target.files?.[0] || null); event.currentTarget.value = ''; }} /></Button>}
        {capabilities.can_export_summary && <Button size="small" variant="outlined" disabled={busy || !options.summary_available} onClick={() => { void exportPersonnelChangeSummary().catch(failure => setError(errorText(failure))); }}>导出汇总</Button>}
      </Box>
      <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 1 }}>{capabilities.view_all_summary ? '当前可查看全部实验室。' : '仅显示当前账号关联实验室。'}{capabilities.can_edit_summary ? (capabilities.edit_all_summary ? ' 可维护全部汇总记录，保存需填写原因。' : ' 可维护关联实验室记录，保存需填写原因。') : ' 当前为只读查看。'}</Typography>
      <TextField size="small" fullWidth label="筛选实验室、项目、人员或方法" value={summaryQuery} onChange={event => { setSummaryQuery(event.target.value); setSummaryPage(0); }} sx={{ mb: 1.25 }} />
      {summaryRows.length === 0 ? <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>当前范围没有汇总记录。</Typography> : mobile ? <Stack spacing={1}>{pageSummary.map(row => <Paper key={row.row_key} variant="outlined" sx={{ p: 1, minWidth: 0, borderRadius: '2px' }}><Typography variant="body2" fontWeight={800} sx={{ overflowWrap: 'anywhere', mb: 0.75 }}>{row.lab_name} · {row.project_code}</Typography><Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 0.5 }}>{dataCell('负责人', row.leader_name)}{dataCell('实验人员', row.person_names.join('、'))}{dataCell('检测方法', row.methods.join('\n'), true)}</Box>{capabilities.can_edit_summary && <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.75 }}><Button size="small" variant="outlined" sx={actionSx} disabled={writeDisabled} onClick={() => openSummary(row)}>编辑</Button><Button size="small" variant="outlined" color="error" sx={actionSx} disabled={writeDisabled} onClick={() => openSummary(row, true)}>删除</Button></Box>}</Paper>)}</Stack> : <Table size="small" sx={{ tableLayout: 'fixed', width: '100%', '& th, & td': cellSx }}><TableHead><TableRow>{['实验室 / 项目', '负责人', '实验人员', '检测方法', ...(capabilities.can_edit_summary ? ['操作'] : [])].map(title => <TableCell key={title}>{title}</TableCell>)}</TableRow></TableHead><TableBody>{pageSummary.map(row => <TableRow key={row.row_key}><TableCell><Typography variant="body2" fontWeight={600}>{row.lab_name}</Typography>{row.project_code}</TableCell><TableCell>{row.leader_name || '—'}</TableCell><TableCell>{row.person_names.join('、') || '—'}</TableCell><TableCell sx={{ whiteSpace: 'pre-wrap' }}>{row.methods.join('\n') || '—'}</TableCell>{capabilities.can_edit_summary && <TableCell><Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}><Button size="small" variant="outlined" sx={actionSx} disabled={writeDisabled} onClick={() => openSummary(row)}>编辑</Button><Button size="small" variant="outlined" color="error" sx={actionSx} disabled={writeDisabled} onClick={() => openSummary(row, true)}>删除</Button></Box></TableCell>}</TableRow>)}</TableBody></Table>}
      <TablePagination component="div" count={summaryRows.length} page={safeSummaryPage} rowsPerPage={summaryPageSize} rowsPerPageOptions={[20, 50, 100]} onPageChange={(_, page) => setSummaryPage(page)} onRowsPerPageChange={event => { setSummaryPageSize(Number(event.target.value)); setSummaryPage(0); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap' }, '& .MuiTablePagination-spacer': { flex: '0 0 auto' } }} />
    </Paper>}
    {!loading && !capabilities.can_view_summary && !capabilities.can_view_notices && <Alert severity="info">当前账号尚无此模块的查看权限，请联系管理员核对角色和关联实验室。</Alert>}

    <Dialog open={Boolean(detail)} onClose={() => setDetail(null)} fullScreen={mobile} fullWidth maxWidth="md"><DialogTitle>通知详情</DialogTitle><DialogContent dividers><Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', sm: 'repeat(2,minmax(0,1fr))' }, gap: 0.75 }}>{detailValues.filter(([, value]) => value !== '' && value !== undefined && value !== null).map(([label, value], index) => <React.Fragment key={`${label}:${index}`}>{dataCell(label, value)}</React.Fragment>)}</Box><Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 1 }}>记录日期用于业务记录，不是定时生效；通知投递和审批/汇总状态分别记录。</Typography></DialogContent><DialogActions sx={{ flexWrap: 'wrap', gap: 0.5 }}><Button variant="outlined" onClick={() => setDetail(null)}>关闭</Button>{detail && noticeButtons(detail, false)}</DialogActions></Dialog>

    <Dialog open={Boolean(noticeAction)} onClose={() => !busy && setNoticeAction(null)} fullScreen={mobile} fullWidth maxWidth="sm"><DialogTitle>{noticeAction?.kind === 'approve' ? '确认通过' : noticeAction?.kind === 'reject' ? '驳回通知' : noticeAction?.kind === 'withdraw' ? '撤回通知' : '删除通知'}</DialogTitle><DialogContent dividers>
      <Typography variant="body2" fontWeight={700} sx={{ mb: 1 }}>{noticeAction?.notice.notice_no} · {noticeAction?.notice.change_type}</Typography>
      <Typography variant="body2" sx={{ mb: 1 }}>{noticeAction ? projectText(noticeAction.notice) : ''}</Typography>
      <Alert severity={noticeAction?.kind === 'delete' ? 'warning' : 'info'} sx={{ mb: 1.5 }}>{noticeAction?.kind === 'approve' ? noticeAction.notice.action_key === 'notice_only' ? '此类型仅审批通过，不修改汇总表。' : '通过后立即同步当前项目人员汇总，并保留备份。记录日期不是定时执行时间。' : noticeAction?.kind === 'reject' ? '驳回不会修改汇总表，发起人可修订并重新提交。' : noticeAction?.kind === 'withdraw' ? '仅撤回尚未审批的申请，不修改汇总表；撤回后可修订重提。' : '删除仅标记通知状态，不会恢复已同步的汇总关系。需要恢复关系时请提交反向变动或维护汇总表。'}</Alert>
      <TextField size="small" fullWidth multiline minRows={2} label={noticeAction?.kind === 'approve' ? '审批说明（可选）' : '操作原因'} required={noticeAction?.kind !== 'approve'} value={actionReason} onChange={event => setActionReason(event.target.value)} disabled={busy} inputProps={{ maxLength: 500 }} />
      {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
    </DialogContent><DialogActions sx={{ flexWrap: 'wrap', gap: 0.5 }}><Button variant="outlined" disabled={busy} onClick={() => setNoticeAction(null)}>取消</Button><Button variant="contained" color={noticeAction?.kind === 'approve' ? 'primary' : 'warning'} disabled={writeDisabled || noticeAction?.kind !== 'approve' && !actionReason.trim()} onClick={() => void confirmNoticeAction()}>确认{noticeAction?.kind === 'approve' ? '通过' : noticeAction?.kind === 'reject' ? '驳回' : noticeAction?.kind === 'withdraw' ? '撤回' : '删除'}</Button></DialogActions></Dialog>

    <Dialog open={Boolean(summaryEditor)} onClose={() => !busy && setSummaryEditor(null)} fullScreen={mobile} fullWidth maxWidth="sm"><DialogTitle>{summaryEditor?.deleting ? '删除汇总记录' : summaryEditor?.key ? '编辑汇总记录' : '新增汇总记录'}</DialogTitle><DialogContent dividers>
      <Alert severity={summaryEditor?.deleting ? 'warning' : 'info'} sx={{ mb: 1.5 }}>{summaryEditor?.deleting ? '将移除此实验室项目的汇总行。此操作不修改程序主数据，也不会撤销历史通知。' : '维护独立汇总表，保存时校验数据版本并记录原因；不修改程序主数据。'}</Alert>
      {summaryEditor && <Stack spacing={1.25}><FormControl size="small" fullWidth disabled={busy || summaryEditor.deleting}><InputLabel>实验室</InputLabel><Select label="实验室" value={summaryEditor.row.lab_name} onChange={event => setSummaryEditor(current => current && ({ ...current, row: { ...current.row, lab_name: String(event.target.value) } }))}>{options.summary_labs.map(lab => <MenuItem key={lab.id} value={lab.name}>{lab.name}</MenuItem>)}</Select></FormControl>
        <TextField size="small" label="项目代号" required value={summaryEditor.row.project_code} disabled={busy || summaryEditor.deleting} onChange={event => setSummaryEditor(current => current && ({ ...current, row: { ...current.row, project_code: event.target.value } }))} />
        <TextField size="small" label="实验室负责人" value={summaryEditor.row.leader_name} disabled={busy || summaryEditor.deleting} onChange={event => setSummaryEditor(current => current && ({ ...current, row: { ...current.row, leader_name: event.target.value } }))} />
        <TextField size="small" label="实验人员（每行一人）" multiline minRows={2} value={summaryEditor.row.person_names.join('\n')} disabled={busy || summaryEditor.deleting} onChange={event => setSummaryEditor(current => current && ({ ...current, row: { ...current.row, person_names: event.target.value.split('\n') } }))} />
        <TextField size="small" label="检测方法（每行一条）" multiline minRows={3} value={summaryEditor.row.methods.join('\n')} disabled={busy || summaryEditor.deleting} onChange={event => setSummaryEditor(current => current && ({ ...current, row: { ...current.row, methods: event.target.value.split('\n') } }))} />
        <TextField size="small" label="维护原因" required multiline minRows={2} value={summaryReason} disabled={busy} inputProps={{ maxLength: 500 }} onChange={event => setSummaryReason(event.target.value)} />
      </Stack>}
      {summaryEditor && summaryEditor.revision !== options.summary_revision && <Alert severity="warning" sx={{ mt: 1 }}>汇总表版本已变化，请关闭并重新打开此记录。</Alert>}
      {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
    </DialogContent><DialogActions><Button variant="outlined" disabled={busy} onClick={() => setSummaryEditor(null)}>取消</Button><Button variant="contained" color={summaryEditor?.deleting ? 'error' : 'primary'} disabled={writeDisabled || !summaryReason.trim() || summaryEditor?.revision !== options.summary_revision} onClick={() => void saveSummary()}>{summaryEditor?.deleting ? '确认删除' : '保存记录'}</Button></DialogActions></Dialog>

    <Dialog open={Boolean(importPreview)} onClose={() => !busy && setImportPreview(null)} fullScreen={mobile} fullWidth maxWidth="md"><DialogTitle>汇总表导入差异预览</DialogTitle><DialogContent dividers>
      <Alert severity="warning" sx={{ mb: 1.5 }}>当前仅生成预览，尚未修改汇总表。确认后按差异替换，导入前自动备份；请核对新增、修改和删除内容。</Alert>
      {importPreview && <><Typography fontWeight={700}>新增 {importPreview.added} 条 · 修改 {importPreview.changed} 条 · 删除 {importPreview.removed} 条</Typography><Typography variant="caption" color="text.secondary">预览有效期至 {importPreview.expires_at}</Typography>
        <Stack spacing={1} sx={{ my: 1.5 }}>{importChanges.slice(importPage * 20, (importPage + 1) * 20).map((change, index) => <Paper variant="outlined" key={index} sx={{ p: 1, minWidth: 0 }}><Typography variant="body2" fontWeight={700}>{change.kind} · {(change.after || change.before)?.lab_name} · {(change.after || change.before)?.project_code}</Typography>{change.before && <Typography variant="body2" sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>原：负责人 {change.before.leader_name || '—'}；人员 {change.before.person_names.join('、') || '—'}；方法 {change.before.methods.join('、') || '—'}</Typography>}{change.after && <Typography variant="body2" sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>新：负责人 {change.after.leader_name || '—'}；人员 {change.after.person_names.join('、') || '—'}；方法 {change.after.methods.join('、') || '—'}</Typography>}</Paper>)}</Stack>
        <TablePagination component="div" count={importChanges.length} page={importPage} rowsPerPage={20} rowsPerPageOptions={[]} onPageChange={(_, page) => setImportPage(page)} labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { px: 0, flexWrap: 'wrap' } }} />
      </>}
      <TextField size="small" fullWidth multiline minRows={2} required label="导入原因" value={importReason} inputProps={{ maxLength: 500 }} disabled={busy} onChange={event => setImportReason(event.target.value)} />
      {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
    </DialogContent><DialogActions sx={{ flexWrap: 'wrap', gap: 0.5 }}><Button variant="outlined" disabled={busy} onClick={() => setImportPreview(null)}>取消导入</Button><Button variant="contained" color="warning" disabled={writeDisabled || !importReason.trim() || !importPreview || importPreview.added + importPreview.changed + importPreview.removed === 0} onClick={() => void applyImport()}>确认差异并导入</Button></DialogActions></Dialog>
  </Box>;
}

export default function PersonnelChangeSessionPage() {
  const { user, token } = useUser();
  return <PersonnelChangePage key={`${user?.id}:${token}`} />;
}


