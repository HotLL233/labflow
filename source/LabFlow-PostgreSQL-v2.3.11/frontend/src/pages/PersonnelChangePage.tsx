import React, { useEffect, useMemo, useState } from 'react';
import {
  Alert, Box, Button, Checkbox, Chip, Dialog, DialogActions, DialogContent, DialogTitle,
  FormControl, FormControlLabel, InputLabel, MenuItem, Paper, Select, Stack, Table,
  TableBody, TableCell, TableContainer, TableHead, TableRow, TextField, Typography,
} from '@mui/material';
import FileDownloadOutlinedIcon from '@mui/icons-material/FileDownloadOutlined';
import FileUploadOutlinedIcon from '@mui/icons-material/FileUploadOutlined';
import RefreshOutlinedIcon from '@mui/icons-material/RefreshOutlined';
import { useUser } from '../UserContext';
import {
  createPersonnelChangeNotice, deletePersonnelChangeNotice, exportPersonnelChangeSummary, getPersonnelChangeNotices,
  getPersonnelChangeOptions, getPersonnelChangePersonProjects, importPersonnelChangeSummary, reviewPersonnelChangeNotice,
  type PersonnelChangeNotice, type PersonnelChangeNoticeInput, type PersonnelChangeOptions, type PersonnelFeedbackField,
} from '../api/client';

const recordValueCellSx = {
  minWidth: 0,
  boxSizing: 'border-box',
  border: '1px solid #d9dfe7',
  borderRadius: '2px',
  bgcolor: '#fff',
  maxHeight: 180,
  overflow: 'auto',
  whiteSpace: 'normal',
  overflowWrap: 'anywhere',
  wordBreak: 'break-word',
  verticalAlign: 'top',
};

type FormState = PersonnelChangeNoticeInput;

const initialForm = (): FormState => ({
  change_type: '员工加入', lab_name: '', person_name: '', project_codes: [], method_names: [],
  source_project_codes: [], target_lab_name: '', target_project_codes: [], transfer_notice_no: '',
  new_project_code: '', is_high_tech: false, high_tech_name: '',
  effective_at: new Date().toISOString().slice(0, 10), notes: '', extra_fields: {},
});

const personRequired = (type: string) => ['员工加入', '员工离职', '实验室内人员调动', '跨实验室人员调出', '跨实验室人员调入'].includes(type);

const PersonnelChangePage: React.FC = () => {
  const { user } = useUser();
  const [options, setOptions] = useState<PersonnelChangeOptions>({ labs: [], summary: [], lab_projects: {}, change_types: [], fields: [], summary_available: false });
  const [existingPersonProjects, setExistingPersonProjects] = useState<string[]>([]);
  const [notices, setNotices] = useState<PersonnelChangeNotice[]>([]);
  const [form, setForm] = useState<FormState>(initialForm());
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [importing, setImporting] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [reviewTarget, setReviewTarget] = useState<PersonnelChangeNotice | null>(null);
  const [reviewDecision, setReviewDecision] = useState<'approve' | 'reject'>('approve');
  const [reviewReason, setReviewReason] = useState('');
  const [detailTarget, setDetailTarget] = useState<PersonnelChangeNotice | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PersonnelChangeNotice | null>(null);
  const [deleteReason, setDeleteReason] = useState('');
  const [deleting, setDeleting] = useState(false);

  const isAdmin = Boolean(user?.is_admin);
  const isAnalysisLeader = Boolean(user?.role_names?.includes('分析检测组长'));
  const isRdLeader = Boolean(user?.role_names?.includes('研发送样组长'));
  const canManageBaseline = isAdmin || isAnalysisLeader;
  const canSubmit = isAdmin || isRdLeader;
  const canReview = isAdmin || isAnalysisLeader;

  const load = async () => {
    setLoading(true);
    setError('');
    try {
      const [optionResult, noticeResult] = await Promise.all([getPersonnelChangeOptions(), getPersonnelChangeNotices(7)]);
      if (optionResult.code !== 0 || !optionResult.data) throw new Error(optionResult.message || '人员变动配置加载失败');
      setOptions(optionResult.data);
      setNotices(noticeResult.code === 0 && noticeResult.data ? noticeResult.data : []);
      setForm(current => ({
        ...current,
        change_type: optionResult.data!.change_types.includes(current.change_type) ? current.change_type : optionResult.data!.change_types[0] || '',
        lab_name: current.lab_name || optionResult.data!.labs[0]?.name || '',
        target_lab_name: current.target_lab_name || optionResult.data!.labs[0]?.name || '',
      }));
    } catch (loadError: any) {
      setError(loadError?.message || '人员变动功能加载失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { void load(); }, []);

  const projectCodes = useMemo(() => options.lab_projects[form.lab_name] || [], [options.lab_projects, form.lab_name]);
  const targetProjectCodes = useMemo(() => options.lab_projects[form.target_lab_name || ''] || [], [options.lab_projects, form.target_lab_name]);
  useEffect(() => {
    const personName = form.person_name?.trim() || '';
    if (!canSubmit || !form.lab_name || !personName) { setExistingPersonProjects([]); return; }
    let active = true;
    void getPersonnelChangePersonProjects(form.lab_name, personName)
      .then(result => { if (active) setExistingPersonProjects(result.code === 0 ? result.data || [] : []); })
      .catch(() => { if (active) setExistingPersonProjects([]); });
    return () => { active = false; };
  }, [canSubmit, form.lab_name, form.person_name]);
  const selectableProjectCodes = useMemo(() => {
    if (form.change_type === '实验室内人员调动' || form.change_type === '员工加入' || form.change_type === '跨实验室人员调入' || form.change_type === '实验室新增方法') return projectCodes;
    return existingPersonProjects.length ? existingPersonProjects : projectCodes;
  }, [existingPersonProjects, form.change_type, projectCodes]);

  useEffect(() => {
    if (form.change_type === '员工加入' || form.change_type === '跨实验室人员调入' || form.change_type === '实验室新增方法') {
      setForm(current => ({ ...current, project_codes: [] }));
    } else if (personRequired(form.change_type) && form.person_name?.trim()) {
      setForm(current => ({ ...current, project_codes: existingPersonProjects.length ? existingPersonProjects : projectCodes }));
    }
  }, [form.change_type, form.person_name, form.lab_name, form.target_lab_name]);

  const update = (patch: Partial<FormState>) => setForm(current => ({ ...current, ...patch }));
  const customFields = useMemo(() => options.fields.filter(field => field.enabled && !['change_type', 'lab_name', 'person_name', 'project_codes', 'method_names', 'effective_at', 'is_high_tech', 'high_tech_name', 'notes'].includes(field.key) && (field.change_types.length === 0 || field.change_types.includes(form.change_type))), [options.fields, form.change_type]);

  const submit = async () => {
    if (!form.lab_name || (form.change_type !== '实验室新增方法' && !form.effective_at) || (personRequired(form.change_type) && !form.person_name?.trim()) || (form.change_type === '员工加入' && form.project_codes.length === 0) || (form.change_type === '跨实验室人员调入' && (!form.target_lab_name || !(form.target_project_codes || []).length)) || (form.change_type === '实验室内人员调动' && (!(form.source_project_codes || []).length || !(form.target_project_codes || []).length)) || customFields.some(field => field.required && !form.extra_fields?.[field.key]?.trim())) {
      setError('请填写实验室、该类型要求的生效日期和人员信息');
      return;
    }
    setSaving(true);
    setError('');
    try {
      const result = await createPersonnelChangeNotice(form);
      if (result.code !== 0) throw new Error(result.message || '通知提交失败');
      setMessage(`${result.data?.notice_no || '通知'} 已发起，等待系统管理员或分析检测组长审批`);
      setForm({ ...initialForm(), lab_name: form.lab_name });
      await load();
    } catch (submitError: any) {
      setError(submitError?.message || '通知提交失败');
    } finally {
      setSaving(false);
    }
  };

  const importSummary = async (file: File | null) => {
    if (!file) return;
    setImporting(true);
    setError('');
    try {
      const result = await importPersonnelChangeSummary(file);
      if (result.code !== 0) throw new Error(result.message || '汇总表导入失败');
      setMessage('项目及人员汇总表已更新，未写入程序数据库');
      await load();
    } catch (importError: any) {
      setError(importError?.message || '汇总表导入失败');
    } finally {
      setImporting(false);
    }
  };

  const startReview = (notice: PersonnelChangeNotice, decision: 'approve' | 'reject') => {
    setReviewTarget(notice);
    setReviewDecision(decision);
    setReviewReason('');
  };

  const review = async () => {
    if (!reviewTarget) return;
    if (reviewDecision === 'reject' && !reviewReason.trim()) {
      setError('驳回通知必须填写原因');
      return;
    }
    setReviewing(true);
    setError('');
    try {
      const result = await reviewPersonnelChangeNotice(reviewTarget.notice_no, reviewDecision, reviewReason);
      if (result.code !== 0) throw new Error(result.message || '审批处理失败');
      setMessage(reviewDecision === 'approve' ? '通知已同步，项目及人员汇总表已更新并完成备份' : '通知已驳回');
      setReviewTarget(null);
      await load();
    } catch (reviewError: any) {
      setError(reviewError?.message || '审批处理失败');
    } finally {
      setReviewing(false);
    }
  };

  const deleteNotice = async () => {
    if (!deleteTarget) return;
    if (!deleteReason.trim()) {
      setError('删除通知必须填写原因');
      return;
    }
    setDeleting(true);
    setError('');
    try {
      const result = await deletePersonnelChangeNotice(deleteTarget.notice_no, deleteReason);
      if (result.code !== 0) throw new Error(result.message || '删除失败');
      setMessage('通知已删除');
      setDeleteTarget(null);
      setDeleteReason('');
      await load();
    } catch (deleteError: any) {
      setError(deleteError?.message || '删除失败');
    } finally {
      setDeleting(false);
    }
  };

  return <Box>
    <Box sx={{ display: 'flex', alignItems: 'flex-start', gap: 1, mb: 2, flexWrap: 'wrap' }}>
      <Box sx={{ flex: 1 }}>
        <Typography variant="h5" fontWeight={800}>人员变动通知</Typography>
        <Typography variant="body2" color="text.secondary">通知独立保存；仅在审批通过后更新独立项目及人员汇总表，不写入程序数据库。</Typography>
      </Box>
      <Stack direction="row" spacing={1}>
        {canManageBaseline && <Button component="label" startIcon={<FileUploadOutlinedIcon />} disabled={importing}>{importing ? '导入中...' : '更新汇总表'}<input hidden type="file" accept=".xlsx" onChange={event => { void importSummary(event.target.files?.[0] || null); event.currentTarget.value = ''; }} /></Button>}
        {canManageBaseline && <Button startIcon={<FileDownloadOutlinedIcon />} onClick={() => void exportPersonnelChangeSummary()} disabled={!options.summary_available}>导出汇总表</Button>}
        <Button startIcon={<RefreshOutlinedIcon />} onClick={() => void load()}>刷新</Button>
      </Stack>
    </Box>
    {error && <Alert severity="error" sx={{ mb: 2 }} onClose={() => setError('')}>{error}</Alert>}
    {message && <Alert severity="success" sx={{ mb: 2 }} onClose={() => setMessage('')}>{message}</Alert>}
    {!loading && !options.summary_available && <Alert severity="warning" sx={{ mb: 2 }}>尚未导入项目及人员汇总表。请由系统管理员或分析检测组长上传模板后再发起或审批通知。</Alert>}

    {canSubmit && options.summary_available && <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
      <Typography fontWeight={800} sx={{ mb: 1.5 }}>发起通知</Typography>
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: 'repeat(2, minmax(0, 1fr))' }, gap: 1.25 }}>
        <FormControl size="small" fullWidth><InputLabel>{form.change_type === '跨实验室人员调入' ? '目标实验室' : '实验室'}</InputLabel><Select label={form.change_type === '跨实验室人员调入' ? '目标实验室' : '实验室'} value={form.change_type === '跨实验室人员调入' ? (form.target_lab_name || '') : form.lab_name} onChange={event => form.change_type === '跨实验室人员调入' ? update({ target_lab_name: String(event.target.value), target_project_codes: [] }) : update({ lab_name: String(event.target.value), project_codes: [], source_project_codes: [], target_project_codes: [] })}>{options.labs.map(lab => <MenuItem key={lab.id} value={lab.name}>{lab.name}</MenuItem>)}</Select></FormControl>
        <FormControl size="small" fullWidth><InputLabel>变动类型</InputLabel><Select label="变动类型" value={form.change_type} onChange={event => update({ change_type: String(event.target.value), project_codes: [], source_project_codes: [], target_project_codes: [], method_names: [] })}>{options.change_types.map(type => <MenuItem key={type} value={type}>{type}</MenuItem>)}</Select></FormControl>
        {(personRequired(form.change_type) || form.change_type === '实验室新增项目') && <TextField size="small" label={form.change_type === '实验室新增项目' ? '项目人员（可选）' : '人员'} required={personRequired(form.change_type)} value={form.person_name || ''} onChange={event => update({ person_name: event.target.value })} helperText={form.change_type === '实验室内人员调动' ? '项目候选为该课题组长关联实验室的全部项目；已关联项目默认勾选，取消勾选即取消关联。' : (existingPersonProjects.length ? `已有人员关联项目（默认勾选，取消勾选即取消关联）：${existingPersonProjects.join('、')}` : '未匹配时按新人员显示本实验室全部项目')} />}
        {form.change_type === '实验室新增项目' ? <>
          <TextField size="small" label="新增项目代号" required value={form.new_project_code || ''} onChange={event => update({ new_project_code: event.target.value })} />
          <FormControlLabel control={<Checkbox checked={Boolean(form.is_high_tech)} onChange={event => update({ is_high_tech: event.target.checked, high_tech_name: event.target.checked ? form.high_tech_name : '' })} />} label="高新项目" />
          {form.is_high_tech && <TextField size="small" label="高新项目名称" required value={form.high_tech_name || ''} onChange={event => update({ high_tech_name: event.target.value })} />}
        </> : form.change_type === '实验室内人员调动' ? <>
          <FormControl size="small" fullWidth><InputLabel>来源项目</InputLabel><Select multiple label="来源项目" value={form.source_project_codes || []} onChange={event => update({ source_project_codes: event.target.value as string[] })} renderValue={selected => selected.join('、')}>{selectableProjectCodes.map(code => <MenuItem key={code} value={code}><Checkbox checked={(form.source_project_codes || []).includes(code)} />{code}</MenuItem>)}</Select></FormControl>
          <FormControl size="small" fullWidth><InputLabel>目标项目</InputLabel><Select multiple label="目标项目" value={form.target_project_codes || []} onChange={event => update({ target_project_codes: event.target.value as string[] })} renderValue={selected => selected.join('、')}>{projectCodes.map(code => <MenuItem key={code} value={code}><Checkbox checked={(form.target_project_codes || []).includes(code)} />{code}</MenuItem>)}</Select></FormControl>
        </> : form.change_type === '跨实验室人员调入' ? <>
          <FormControl size="small" fullWidth><InputLabel>目标项目</InputLabel><Select multiple label="目标项目" value={form.target_project_codes || []} onChange={event => update({ target_project_codes: event.target.value as string[] })} renderValue={selected => selected.join('、')}>{targetProjectCodes.map(code => <MenuItem key={code} value={code}><Checkbox checked={(form.target_project_codes || []).includes(code)} />{code}</MenuItem>)}</Select></FormControl>
          <TextField size="small" label="关联调出通知编号（可选）" value={form.transfer_notice_no || ''} onChange={event => update({ transfer_notice_no: event.target.value })} />
        </> : <FormControl size="small" fullWidth><InputLabel>关联项目</InputLabel><Select multiple label="关联项目" value={form.project_codes} onChange={event => update({ project_codes: event.target.value as string[] })} renderValue={selected => selected.join('、')}>
          {selectableProjectCodes.map(code => <MenuItem key={code} value={code}><Checkbox checked={form.project_codes.includes(code)} />{code}</MenuItem>)}
        </Select></FormControl>}
        {['实验室新增项目', '实验室新增方法'].includes(form.change_type) && <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.75 }}><Typography variant="caption" color="text.secondary">检测方法需逐条填写，禁止逗号、顿号、分号和换行。</Typography>{form.method_names.map((method, index) => <Box key={`method-${index}`} sx={{ display: 'flex', gap: 0.75 }}><TextField size="small" fullWidth label={`检测方法 ${index + 1}`} value={method} onChange={event => { const value = event.target.value; if (/[，,、；;\r\n]/.test(value)) return; update({ method_names: form.method_names.map((item, itemIndex) => itemIndex === index ? value : item) }); }} /><Button size="small" color="error" onClick={() => update({ method_names: form.method_names.filter((_, itemIndex) => itemIndex !== index) })}>删除</Button></Box>)}<Button size="small" onClick={() => update({ method_names: [...form.method_names, ''] })}>新增方法</Button></Box>}
        {form.change_type === '跨实验室人员调出' && <Typography variant="caption" color="text.secondary" sx={{ alignSelf: 'center' }}>仅迁出当前实验室的人员关系；目标实验室须由其组长另行提交“跨实验室人员调入”。</Typography>}
        {form.change_type !== '实验室新增方法' && <TextField size="small" type="date" label="生效日期" required InputLabelProps={{ shrink: true }} value={form.effective_at} onChange={event => update({ effective_at: event.target.value })} />}
        <TextField size="small" label="备注" value={form.notes || ''} onChange={event => update({ notes: event.target.value })} multiline minRows={2} sx={{ gridColumn: { xs: 'auto', md: 'span 2' } }} />
        {customFields.map((field: PersonnelFeedbackField) => <TextField key={field.key} size="small" type={field.field_type === 'date' ? 'date' : 'text'} label={field.label} required={field.required} multiline={field.field_type === 'textarea'} minRows={field.field_type === 'textarea' ? 2 : undefined} InputLabelProps={field.field_type === 'date' ? { shrink: true } : undefined} value={form.extra_fields?.[field.key] || ''} onChange={event => update({ extra_fields: { ...(form.extra_fields || {}), [field.key]: event.target.value } })} />)}
        <Box sx={{ gridColumn: { xs: 'auto', md: 'span 2' }, display: 'flex', justifyContent: 'flex-end' }}><Button variant="contained" disabled={saving} onClick={() => void submit()}>{saving ? '发送中...' : '发起通知'}</Button></Box>
      </Box>
    </Paper>}
    {!canSubmit && !loading && <Alert severity="info" sx={{ mb: 2 }}>当前账号可查看通知、汇总表并按权限审批；只有系统管理员和研发送样组长可以发起通知。</Alert>}

    <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
      <Typography fontWeight={800} sx={{ mb: 1 }}>近 7 天通知记录</Typography>
      <TableContainer><Table size="small" sx={{ '& tbody td': recordValueCellSx }}><TableHead><TableRow>{['通知编号', '变动类型', '实验室', '人员', '项目/方法', '生效日期', '状态', '操作'].map(title => <TableCell key={title} sx={{ fontWeight: 800 }}>{title}</TableCell>)}</TableRow></TableHead><TableBody>
        {notices.map(notice => <TableRow key={notice.notice_no}>
          <TableCell sx={{ fontFamily: 'monospace', fontSize: '0.8rem' }}>{notice.notice_no}</TableCell><TableCell>{notice.change_type}</TableCell><TableCell>{notice.lab_name}</TableCell><TableCell>{notice.person_name || '-'}</TableCell>
          <TableCell sx={{ whiteSpace: 'normal', wordBreak: 'break-word' }}>
            {notice.change_type === '实验室新增项目' ? (
              <>
                {notice.new_project_code && <div>项目代号: {notice.new_project_code}</div>}
                {notice.new_project_name && <div>项目名称: {notice.new_project_name}</div>}
                {notice.method_names && notice.method_names.length > 0 && <div>检测方法: {notice.method_names.join('、')}</div>}
                {notice.person_name && <div>人员: {notice.person_name}</div>}
              </>
            ) : notice.change_type === '实验室新增方法' ? (
              <>
                {notice.project_codes && notice.project_codes.length > 0 && <div>项目: {notice.project_codes.join('、')}</div>}
                {notice.method_names && notice.method_names.length > 0 && <div>方法: {notice.method_names.join('、')}</div>}
              </>
            ) : (
              [notice.project_codes?.join('、'), notice.method_names?.join('、')].filter(Boolean).join(' / ') || '-'
            )}
          </TableCell><TableCell>{notice.effective_at}</TableCell>
          <TableCell><Stack spacing={0.5}><Chip size="small" label={notice.review_status || '待审批'} color={notice.review_status === '已同步' ? 'success' : notice.review_status === '已驳回' ? 'error' : 'warning'} />{notice.review_reason && <Typography variant="caption">{notice.review_reason}</Typography>}</Stack></TableCell>
          <TableCell>
            <Stack direction="row" spacing={0.5}>
              <Button size="small" onClick={() => setDetailTarget(notice)}>详情</Button>
              {canReview && notice.review_status === '待审批' && notice.actor_username !== user?.username && <>
                <Button size="small" onClick={() => startReview(notice, 'approve')}>通过</Button>
                <Button size="small" color="error" onClick={() => startReview(notice, 'reject')}>驳回</Button>
              </>}
              {canReview && notice.review_status !== '已删除' && <Button size="small" color="error" onClick={() => { setDeleteTarget(notice); setDeleteReason(''); }}>删除</Button>}
            </Stack>
          </TableCell>
        </TableRow>)}
        {notices.length === 0 && <TableRow><TableCell colSpan={8} align="center">暂无近 7 天通知记录</TableCell></TableRow>}
      </TableBody></Table></TableContainer>
    </Paper>

    {canManageBaseline && <Paper variant="outlined" sx={{ p: 2 }}>
      <Typography fontWeight={800} sx={{ mb: 1 }}>项目及人员汇总表</Typography>
      <TableContainer><Table size="small" sx={{ '& tbody td': recordValueCellSx }}><TableHead><TableRow>{['实验室', '实验室负责人', '实验人员', '项目代号', '检测方法'].map(title => <TableCell key={title} sx={{ fontWeight: 800 }}>{title}</TableCell>)}</TableRow></TableHead><TableBody>
        {options.summary.map((row, index) => <TableRow key={`${row.lab_name}-${row.project_code}-${index}`}><TableCell>{row.lab_name}</TableCell><TableCell>{row.leader_name || '-'}</TableCell><TableCell sx={{ whiteSpace: 'normal', wordBreak: 'break-word' }}>{row.person_names.join('、') || '-'}</TableCell><TableCell>{row.project_code}</TableCell><TableCell sx={{ whiteSpace: 'pre-line', wordBreak: 'break-word', verticalAlign: 'top' }}>{row.methods.join('\n') || '-'}</TableCell></TableRow>)}
        {options.summary.length === 0 && <TableRow><TableCell colSpan={5} align="center">尚未导入汇总表</TableCell></TableRow>}
      </TableBody></Table></TableContainer>
    </Paper>}

    <Dialog open={Boolean(reviewTarget)} onClose={() => !reviewing && setReviewTarget(null)} fullWidth maxWidth="sm">
      <DialogTitle>{reviewDecision === 'approve' ? '通过人员变动通知' : '驳回人员变动通知'}</DialogTitle>
      <DialogContent><Typography variant="body2" sx={{ mb: 2 }}>{reviewDecision === 'approve' ? '通过后将更新独立项目及人员汇总表，并自动保留审批前备份。' : '驳回后不会调整汇总表。'}</Typography>
        <TextField autoFocus fullWidth multiline minRows={2} label={reviewDecision === 'approve' ? '审批说明（可选）' : '驳回原因'} required={reviewDecision === 'reject'} value={reviewReason} onChange={event => setReviewReason(event.target.value)} />
      </DialogContent>
      <DialogActions><Button onClick={() => setReviewTarget(null)} disabled={reviewing}>取消</Button><Button variant="contained" color={reviewDecision === 'approve' ? 'primary' : 'error'} onClick={() => void review()} disabled={reviewing}>{reviewing ? '处理中...' : '确认'}</Button></DialogActions>
    </Dialog>

    <Dialog open={Boolean(detailTarget)} onClose={() => setDetailTarget(null)} fullWidth maxWidth="md">
      <DialogTitle>通知详情</DialogTitle>
      <DialogContent>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'auto 1fr' }, gap: 1.5, pt: 1 }}>
          <Typography variant="body2" fontWeight={600}>通知编号：</Typography><Typography variant="body2">{detailTarget?.notice_no}</Typography>
          <Typography variant="body2" fontWeight={600}>创建时间：</Typography><Typography variant="body2">{detailTarget?.created_at}</Typography>
          <Typography variant="body2" fontWeight={600}>提交人：</Typography><Typography variant="body2">{detailTarget?.actor_username}</Typography>
          <Typography variant="body2" fontWeight={600}>变动类型：</Typography><Typography variant="body2">{detailTarget?.change_type}</Typography>
          <Typography variant="body2" fontWeight={600}>实验室：</Typography><Typography variant="body2">{detailTarget?.lab_name}</Typography>
          {detailTarget?.target_lab_name && <><Typography variant="body2" fontWeight={600}>目标实验室：</Typography><Typography variant="body2">{detailTarget.target_lab_name}</Typography></>}
          {detailTarget?.person_name && <><Typography variant="body2" fontWeight={600}>人员名称：</Typography><Typography variant="body2">{detailTarget.person_name}</Typography></>}
          {detailTarget?.source_project_codes && detailTarget.source_project_codes.length > 0 && <><Typography variant="body2" fontWeight={600}>来源项目：</Typography><Typography variant="body2">{detailTarget.source_project_codes.join('、')}</Typography></>}
          {detailTarget?.project_codes && detailTarget.project_codes.length > 0 && <><Typography variant="body2" fontWeight={600}>项目代号：</Typography><Typography variant="body2">{detailTarget.project_codes.join('、')}</Typography></>}
          {detailTarget?.target_project_codes && detailTarget.target_project_codes.length > 0 && <><Typography variant="body2" fontWeight={600}>目标项目代号：</Typography><Typography variant="body2">{detailTarget.target_project_codes.join('、')}</Typography></>}
          {detailTarget?.transfer_notice_no && <><Typography variant="body2" fontWeight={600}>关联调出通知：</Typography><Typography variant="body2">{detailTarget.transfer_notice_no}</Typography></>}
          {detailTarget?.method_names && detailTarget.method_names.length > 0 && <><Typography variant="body2" fontWeight={600}>检测方法：</Typography><Typography variant="body2">{detailTarget.method_names.join('、')}</Typography></>}
          {detailTarget?.new_project_code && <><Typography variant="body2" fontWeight={600}>新增项目代号：</Typography><Typography variant="body2">{detailTarget.new_project_code}</Typography></>}
          {detailTarget?.new_project_name && <><Typography variant="body2" fontWeight={600}>新增项目名称：</Typography><Typography variant="body2">{detailTarget.new_project_name}</Typography></>}
          {detailTarget?.is_high_tech && <><Typography variant="body2" fontWeight={600}>高新项目：</Typography><Typography variant="body2">是</Typography></>}
          {detailTarget?.high_tech_name && <><Typography variant="body2" fontWeight={600}>高新项目名称：</Typography><Typography variant="body2">{detailTarget.high_tech_name}</Typography></>}
          <Typography variant="body2" fontWeight={600}>生效日期：</Typography><Typography variant="body2">{detailTarget?.effective_at}</Typography>
          {detailTarget?.notes && <><Typography variant="body2" fontWeight={600}>备注：</Typography><Typography variant="body2" sx={{ whiteSpace: 'pre-wrap' }}>{detailTarget.notes}</Typography></>}
          {detailTarget?.delivery_status && <><Typography variant="body2" fontWeight={600}>交付状态：</Typography><Typography variant="body2">{detailTarget.delivery_status}</Typography></>}
          <Typography variant="body2" fontWeight={600}>审批状态：</Typography><Typography variant="body2">{detailTarget?.review_status || '待审批'}</Typography>
          {detailTarget?.reviewed_by && <><Typography variant="body2" fontWeight={600}>审批人：</Typography><Typography variant="body2">{detailTarget.reviewed_by}</Typography></>}
          {detailTarget?.reviewed_at && <><Typography variant="body2" fontWeight={600}>审批时间：</Typography><Typography variant="body2">{detailTarget.reviewed_at}</Typography></>}
          {detailTarget?.review_reason && <><Typography variant="body2" fontWeight={600}>审批说明：</Typography><Typography variant="body2" sx={{ whiteSpace: 'pre-wrap' }}>{detailTarget.review_reason}</Typography></>}
          {detailTarget?.updated_at && <><Typography variant="body2" fontWeight={600}>更新时间：</Typography><Typography variant="body2">{detailTarget.updated_at}</Typography></>}
          {detailTarget?.extra_fields && Object.keys(detailTarget.extra_fields).length > 0 && Object.entries(detailTarget.extra_fields).map(([key, value]) => <React.Fragment key={key}><Typography variant="body2" fontWeight={600}>{key}：</Typography><Typography variant="body2">{value}</Typography></React.Fragment>)}
        </Box>
      </DialogContent>
      <DialogActions><Button onClick={() => setDetailTarget(null)}>关闭</Button></DialogActions>
    </Dialog>

    <Dialog open={Boolean(deleteTarget)} onClose={() => !deleting && setDeleteTarget(null)} fullWidth maxWidth="sm">
      <DialogTitle>删除通知</DialogTitle>
      <DialogContent>
        <Typography variant="body2" sx={{ mb: 2 }}>删除后通知状态将变更为"已删除"，不会恢复已同步通知的汇总表变更。</Typography>
        <TextField autoFocus fullWidth multiline minRows={2} label="删除原因" required value={deleteReason} onChange={event => setDeleteReason(event.target.value)} />
      </DialogContent>
      <DialogActions><Button onClick={() => setDeleteTarget(null)} disabled={deleting}>取消</Button><Button variant="contained" color="error" onClick={() => void deleteNotice()} disabled={deleting}>{deleting ? '删除中...' : '确认删除'}</Button></DialogActions>
    </Dialog>
  </Box>;
};

export default PersonnelChangePage;
