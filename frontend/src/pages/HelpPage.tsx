import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert, Autocomplete, Box, Button, Card, CardContent, Checkbox, Chip, CircularProgress, Divider, FormControl,
  InputLabel, MenuItem, Paper, Select, Stack, Tab, Table, TableBody, TableCell,
  TableContainer, TableHead, TableRow, Tabs, TextField, Tooltip, Typography,
  useMediaQuery, useTheme,
} from '@mui/material';
import MenuBookOutlinedIcon from '@mui/icons-material/MenuBookOutlined';
import PeopleAltOutlinedIcon from '@mui/icons-material/PeopleAltOutlined';
import SearchIcon from '@mui/icons-material/Search';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import UndoOutlinedIcon from '@mui/icons-material/UndoOutlined';
import AddOutlinedIcon from '@mui/icons-material/AddOutlined';
import SaveOutlinedIcon from '@mui/icons-material/SaveOutlined';
import CloseOutlinedIcon from '@mui/icons-material/CloseOutlined';
import { useUser } from '../UserContext';
import {
  createPersonnelFeedback, getGroups, getHelpArticleImageBlob, getHelpArticles,
  getPersonnelFeedback, getProjects, updatePersonnelFeedback, withdrawPersonnelFeedback,
  type PersonnelFeedback,
} from '../api/client';
import type { HelpArticle, Project, ProjectGroup } from '../types';

type FeedbackForm = {
  lab_id: number;
  change_type: string;
  person_name: string;
  effective_at: string;
  notes: string;
  project_ids: number[];
};

const TYPES = ['新人员加入', '人员离开', '岗位调整', '信息更新'];
const RADIUS = '2px';
const defaultForm = (labId = 0): FeedbackForm => ({
  lab_id: labId,
  change_type: TYPES[0],
  person_name: '',
  effective_at: new Date().toISOString().slice(0, 10),
  notes: '',
  project_ids: [],
});
const cleanHtml = (html: string) => html
  .replace(/<\/?(script|style|iframe|object|embed)[^>]*>/gi, '')
  .replace(/\son\w+\s*=\s*(['"]).*?\1/gi, '')
  .replace(/javascript:/gi, '');
const statusColor = (status: string) => status === '有效'
  ? { bgcolor: '#2e7d32', color: '#fff' }
  : { bgcolor: '#eceff1', color: '#455a64' };

const HelpPage: React.FC = () => {
  const { user } = useUser();
  const theme = useTheme();
  const compact = useMediaQuery(theme.breakpoints.down('md'));
  const [tab, setTab] = useState(0);
  const [articles, setArticles] = useState<HelpArticle[]>([]);
  const [selected, setSelected] = useState<HelpArticle | null>(null);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const [feedback, setFeedback] = useState<PersonnelFeedback[]>([]);
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [editing, setEditing] = useState<PersonnelFeedback | null>(null);
  const [form, setForm] = useState<FeedbackForm>(defaultForm());
  const [statusFilter, setStatusFilter] = useState('全部');
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  const articleBodyRef = useRef<HTMLDivElement>(null);
  const canEditFeedback = (user?.role_names || []).includes('研发送样组长');

  const load = async () => {
    setLoading(true);
    const [articleResult, feedbackResult, groupResult, projectResult] = await Promise.all([
      getHelpArticles(true),
      getPersonnelFeedback(),
      getGroups(),
      getProjects({ active_only: true, status: 'ongoing' }),
    ]);
    setArticles(articleResult.code === 0 && articleResult.data ? articleResult.data : []);
    setFeedback(feedbackResult.code === 0 && feedbackResult.data ? feedbackResult.data : []);
    setGroups(groupResult.code === 0 && groupResult.data ? groupResult.data : []);
    setProjects(projectResult.code === 0 && projectResult.data ? projectResult.data : []);
    setLoading(false);
  };

  useEffect(() => { void load(); }, []);
  useEffect(() => {
    let cancelled = false;
    const objectUrls: string[] = [];
    const loadArticleImages = async () => {
      const container = articleBodyRef.current;
      if (!container || !selected) return;
      const images = Array.from(container.querySelectorAll<HTMLImageElement>('img[data-help-image]'));
      await Promise.all(images.map(async image => {
        const source = image.dataset.helpImage;
        if (!source) return;
        try {
          const blob = await getHelpArticleImageBlob(source);
          if (cancelled) return;
          const objectUrl = URL.createObjectURL(blob);
          objectUrls.push(objectUrl);
          image.src = objectUrl;
        } catch {
          image.alt = '教程图片加载失败';
        }
      }));
    };
    void loadArticleImages();
    return () => {
      cancelled = true;
      objectUrls.forEach(URL.revokeObjectURL);
    };
  }, [selected]);

  const filteredArticles = useMemo(
    () => articles.filter(item => `${item.title} ${item.content_html}`.toLowerCase().includes(search.toLowerCase())),
    [articles, search],
  );
  const visibleFeedback = useMemo(
    () => statusFilter === '全部' ? feedback : feedback.filter(item => item.status === statusFilter),
    [feedback, statusFilter],
  );
  const editableGroups = useMemo(
    () => user?.group_id ? groups.filter(group => group.id === user.group_id) : groups,
    [groups, user?.group_id],
  );
  const availableProjects = useMemo(
    () => projects.filter(project => !form.lab_id || project.lab_ids.includes(form.lab_id)),
    [projects, form.lab_id],
  );

  const resetForm = () => {
    setEditing(null);
    setForm(defaultForm(user?.group_id || 0));
    setMessage('');
  };
  const openEdit = (item: PersonnelFeedback) => {
    setEditing(item);
    setForm({
      lab_id: item.lab_id,
      change_type: item.change_type,
      person_name: item.person_name,
      effective_at: item.effective_at.slice(0, 10),
      notes: item.notes,
      project_ids: item.project_ids,
    });
    setMessage('');
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };
  const save = async () => {
    if (!form.lab_id || !form.person_name.trim() || !form.effective_at) {
      setMessage('请完整填写实验室、人员姓名和生效日期');
      return;
    }
    setSaving(true);
    setMessage('');
    const payload = {
      change_type: form.change_type,
      person_name: form.person_name,
      effective_at: form.effective_at,
      notes: form.notes,
      project_ids: form.project_ids,
    };
    const result = editing
      ? await updatePersonnelFeedback(editing.id, payload)
      : await createPersonnelFeedback({ ...payload, lab_id: form.lab_id });
    setSaving(false);
    if (result.code !== 0) {
      setMessage(result.message || '保存失败');
      return;
    }
    resetForm();
    await load();
  };
  const withdraw = async (item: PersonnelFeedback) => {
    if (!window.confirm(`确认撤回 ${item.notice_no} 吗？撤回后记录仍会保留。`)) return;
    const result = await withdrawPersonnelFeedback(item.id);
    if (result.code !== 0) setMessage(result.message || '撤回失败');
    else await load();
  };

  if (loading) return <Box sx={{ minHeight: 320, display: 'grid', placeItems: 'center' }}><CircularProgress /></Box>;

  if (selected) return (
    <Box sx={{ display: 'grid', gridTemplateColumns: compact ? '1fr' : '220px minmax(0,1fr) 180px', gap: 2 }}>
      <Paper variant="outlined" sx={{ p: 1.25, height: 'fit-content', position: compact ? 'static' : 'sticky', top: 82, borderRadius: RADIUS }}>
        <Button startIcon={<ArrowBackIcon />} onClick={() => setSelected(null)} sx={{ mb: 1 }}>返回指南</Button>
        {articles.map(item => <Button key={item.id} onClick={() => setSelected(item)} fullWidth sx={{ justifyContent: 'flex-start', textAlign: 'left', py: 1, color: item.id === selected.id ? 'primary.main' : 'text.primary' }}>{item.title}</Button>)}
      </Paper>
      <Box sx={{ p: { xs: 1.5, md: 3 }, minWidth: 0, border: '1px solid', borderColor: 'divider' }}>
        <Typography variant="h4" sx={{ fontSize: 26, fontWeight: 700, mb: 1 }}>{selected.title}</Typography>
        <Divider sx={{ mb: 3 }} />
        <Box ref={articleBodyRef} className="help-article-body" sx={{ '& img': { maxWidth: '100%', height: 'auto', cursor: 'zoom-in' }, '& img:not([src])': { display: 'none' }, '& .help-article-image': { m: '20px 0', textAlign: 'center' }, '& .help-article-image figcaption': { color: 'text.secondary', fontSize: 13, mt: .5 }, '& table': { width: '100%', borderCollapse: 'collapse', mb: 2 }, '& td, & th': { border: '1px solid #d9dee7', p: 1 }, '& p': { lineHeight: 1.9 }, '& h1,h2,h3': { mt: 3 } }} dangerouslySetInnerHTML={{ __html: cleanHtml(selected.content_html) }} />
      </Box>
      {!compact && <Box sx={{ p: 2, height: 'fit-content', position: 'sticky', top: 82, border: '1px solid', borderColor: 'divider' }}><Typography fontWeight={700} sx={{ mb: 1 }}>本文指南</Typography><Typography variant="body2" color="text.secondary">使用左侧列表查阅其他操作说明。</Typography></Box>}
    </Box>
  );

  return (
    <Box>
      <Box sx={{ mb: 2.5 }}>
        <Typography variant="h4" sx={{ fontWeight: 700, fontSize: { xs: 26, md: 30 } }}>帮助与反馈</Typography>
        <Typography color="text.secondary" sx={{ mt: 0.5 }}>查阅操作指南，提交实验室人员变动通知。</Typography>
      </Box>
      {message && <Alert severity="error" onClose={() => setMessage('')} sx={{ mb: 2 }}>{message}</Alert>}
      <Box sx={{ border: '1px solid', borderColor: 'divider' }}>
        <Tabs value={tab} onChange={(_event, value) => setTab(value)} sx={{ px: 1, borderBottom: '1px solid', borderColor: 'divider' }}>
          <Tab icon={<MenuBookOutlinedIcon />} iconPosition="start" label="使用指南" />
          <Tab icon={<PeopleAltOutlinedIcon />} iconPosition="start" label="人员变动反馈" />
        </Tabs>

        {tab === 0 && <Box sx={{ p: { xs: 1.5, md: 3 } }}>
          <TextField value={search} onChange={event => setSearch(event.target.value)} placeholder="搜索操作指南" fullWidth InputProps={{ startAdornment: <SearchIcon sx={{ mr: 1, color: 'text.secondary' }} /> }} sx={{ mb: 2.5 }} />
          {filteredArticles.length === 0 ? <Alert severity="info">暂无可查看的指南。</Alert> : <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', gap: 1.5 }}>{filteredArticles.map(item => <Box key={item.id} onClick={() => setSelected(item)} sx={{ p: 2, cursor: 'pointer', border: '1px solid', borderColor: 'divider', '&:hover': { borderColor: 'primary.main', bgcolor: '#f7fbff' } }}><MenuBookOutlinedIcon color="primary" /><Typography fontWeight={700} sx={{ mt: 1 }}>{item.title}</Typography><Typography variant="body2" color="text.secondary" sx={{ mt: .75, display: '-webkit-box', overflow: 'hidden', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>{item.source_file || '图文操作指南'}</Typography></Box>)}</Box>}
        </Box>}

        {tab === 1 && <Box sx={{ p: { xs: 1.5, md: 2.5 } }}>
          <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end', gap: 1, flexWrap: 'wrap', mb: 2 }}>
            <Box><Typography fontWeight={700} sx={{ fontSize: '1.15rem' }}>人员变动通知</Typography><Typography variant="body2" color="text.secondary">仅用于通知，不会创建用户、分配角色或进入工作量统计。</Typography></Box>
            <FormControl size="small" sx={{ minWidth: 132 }}><InputLabel>记录状态</InputLabel><Select label="记录状态" value={statusFilter} onChange={event => setStatusFilter(String(event.target.value))}><MenuItem value="全部">全部记录</MenuItem><MenuItem value="有效">有效</MenuItem><MenuItem value="已撤回">已撤回</MenuItem></Select></FormControl>
          </Box>

          {canEditFeedback ? <Box sx={{ mb: 2.25, px: { xs: 1, md: 1.5 }, py: 1.5, bgcolor: '#f7fbff', border: '1px solid #d7e8f7', borderLeft: '4px solid #1976d2' }}>
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, flexWrap: 'wrap', mb: 1.5 }}><Box><Typography fontWeight={700}>{editing ? `编辑通知 ${editing.notice_no}` : '人员变动登记'}</Typography><Typography variant="caption" color="text.secondary">保存后作为通知记录保留，可在下方查看、编辑或撤回。</Typography></Box>{editing && <Button size="small" startIcon={<CloseOutlinedIcon />} onClick={resetForm}>取消编辑</Button>}</Box>
            <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, minmax(0, 1fr))' }, gap: 1.25 }}>
              <FormControl fullWidth size="small" disabled={!!editing || editableGroups.length <= 1}><InputLabel>实验室</InputLabel><Select label="实验室" value={form.lab_id || ''} onChange={event => setForm({ ...form, lab_id: Number(event.target.value), project_ids: [] })}>{editableGroups.map(group => <MenuItem key={group.id} value={group.id}>{group.name}</MenuItem>)}</Select></FormControl>
              <FormControl fullWidth size="small"><InputLabel>变动类型</InputLabel><Select label="变动类型" value={form.change_type} onChange={event => setForm({ ...form, change_type: String(event.target.value) })}>{TYPES.map(type => <MenuItem key={type} value={type}>{type}</MenuItem>)}</Select></FormControl>
              <TextField size="small" label="人员姓名" value={form.person_name} onChange={event => setForm({ ...form, person_name: event.target.value })} fullWidth />
              <TextField size="small" label="生效日期" type="date" value={form.effective_at} onChange={event => setForm({ ...form, effective_at: event.target.value })} fullWidth InputLabelProps={{ shrink: true }} />
              <Autocomplete
                multiple
                disableCloseOnSelect
                options={availableProjects}
                value={availableProjects.filter(project => form.project_ids.includes(project.id))}
                onChange={(_event, selectedProjects) => setForm(current => ({
                  ...current,
                  project_ids: selectedProjects.map(project => project.id),
                }))}
                getOptionLabel={project => project.name}
                isOptionEqualToValue={(option, value) => option.id === value.id}
                renderOption={(props, project, { selected }) => (
                  <li {...props} key={project.id}>
                    <Checkbox checked={selected} size="small" sx={{ mr: 0.5 }} />
                    {project.name}
                  </li>
                )}
                renderInput={params => <TextField {...params} label="关联项目" size="small" placeholder="搜索或选择项目" />}
                noOptionsText="当前实验室没有可关联项目"
                sx={{ gridColumn: { xs: 'auto', sm: 'span 2' } }}
              />
              <TextField size="small" label="说明" value={form.notes} onChange={event => setForm({ ...form, notes: event.target.value })} fullWidth multiline minRows={2} sx={{ gridColumn: { xs: 'auto', sm: 'span 2' } }} />
              <Box sx={{ display: 'flex', justifyContent: 'flex-end', gridColumn: { xs: 'auto', sm: 'span 2' } }}><Button variant="contained" startIcon={editing ? <SaveOutlinedIcon /> : <AddOutlinedIcon />} disabled={saving} onClick={() => void save()}>{saving ? '保存中...' : editing ? '保存修改' : '提交通知'}</Button></Box>
            </Box>
          </Box> : <Alert severity="info" sx={{ mb: 2 }}>当前账号仅可查看。请使用所属实验室的研发送样组长账号登记、编辑或撤回人员变动通知。</Alert>}

          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}><Typography fontWeight={700}>通知记录</Typography><Typography variant="caption" color="text.secondary">共 {visibleFeedback.length} 条</Typography></Box>
          <TableContainer sx={{ display: { xs: 'none', md: 'block' }, border: '1px solid #e0e0e0' }}>
            <Table size="small" sx={{ tableLayout: 'fixed', width: '100%' }}><colgroup><col style={{ width: '9%' }} /><col style={{ width: '12%' }} /><col style={{ width: '18%' }} /><col style={{ width: '23%' }} /><col style={{ width: '17%' }} /><col style={{ width: '21%' }} /></colgroup><TableHead><TableRow sx={{ bgcolor: '#f5f7f5' }}>{['状态', '通知编号', '人员 / 变动类型', '实验室 / 关联项目', '生效日期 / 提交人', '说明 / 操作'].map(title => <TableCell key={title} sx={{ fontWeight: 700, py: 1, px: 1, whiteSpace: 'normal' }}>{title}</TableCell>)}</TableRow></TableHead><TableBody>{visibleFeedback.length === 0 ? <TableRow><TableCell colSpan={6} align="center" sx={{ py: 4, color: 'text.secondary' }}>暂无人员变动通知</TableCell></TableRow> : visibleFeedback.map(item => <TableRow key={item.id} hover sx={{ verticalAlign: 'top', '& td': { px: 1, py: 1.1 } }}><TableCell><Chip label={item.status} size="small" sx={{ ...statusColor(item.status), borderRadius: RADIUS, fontWeight: 700 }} /></TableCell><TableCell sx={{ fontWeight: 700, overflowWrap: 'anywhere' }}>{item.notice_no}</TableCell><TableCell><Typography fontWeight={700} sx={{ fontSize: '0.86rem', overflowWrap: 'anywhere' }}>{item.person_name}</Typography><Chip label={item.change_type} size="small" variant="outlined" sx={{ mt: .55, borderRadius: RADIUS }} /></TableCell><TableCell><Typography variant="body2">{item.lab_name}</Typography><Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: .4, overflowWrap: 'anywhere' }}>{item.project_names.join('、') || '未关联项目'}</Typography></TableCell><TableCell><Typography variant="body2">{item.effective_at}</Typography><Typography variant="caption" color="text.secondary">提交人：{item.created_by_username}</Typography></TableCell><TableCell><Typography variant="body2" sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{item.notes || '-'}</Typography>{canEditFeedback && item.status === '有效' && <Stack direction="row" spacing={.5} sx={{ mt: .8 }}><Tooltip title="编辑通知"><Button size="small" startIcon={<EditOutlinedIcon />} onClick={() => openEdit(item)}>编辑</Button></Tooltip><Button size="small" color="warning" startIcon={<UndoOutlinedIcon />} onClick={() => void withdraw(item)}>撤回</Button></Stack>}</TableCell></TableRow>)}</TableBody></Table>
          </TableContainer>
          <Box sx={{ display: { xs: 'grid', md: 'none' }, gap: 1 }}>{visibleFeedback.length === 0 ? <Box sx={{ py: 4, textAlign: 'center', color: 'text.secondary' }}>暂无人员变动通知</Box> : visibleFeedback.map(item => <Card key={item.id} variant="outlined" sx={{ borderRadius: RADIUS, borderLeft: `4px solid ${item.status === '有效' ? '#2e7d32' : '#90a4ae'}` }}><CardContent sx={{ p: 1.25, '&:last-child': { pb: 1.25 } }}><Box sx={{ display: 'flex', justifyContent: 'space-between', gap: 1, alignItems: 'flex-start' }}><Box><Typography fontWeight={700}>{item.person_name}</Typography><Typography variant="caption" color="text.secondary">{item.notice_no}</Typography></Box><Chip label={item.status} size="small" sx={{ ...statusColor(item.status), borderRadius: RADIUS, fontWeight: 700 }} /></Box><Box sx={{ mt: 1, display: 'grid', gap: .55, fontSize: '0.86rem' }}><Box><Typography component="span" color="text.secondary">变动类型：</Typography>{item.change_type}</Box><Box><Typography component="span" color="text.secondary">实验室：</Typography>{item.lab_name}</Box><Box><Typography component="span" color="text.secondary">关联项目：</Typography>{item.project_names.join('、') || '未关联项目'}</Box><Box><Typography component="span" color="text.secondary">生效日期：</Typography>{item.effective_at}</Box>{item.notes && <Box><Typography component="span" color="text.secondary">说明：</Typography>{item.notes}</Box>}</Box>{canEditFeedback && item.status === '有效' && <Stack direction="row" spacing={.5} sx={{ mt: 1 }}><Button size="small" startIcon={<EditOutlinedIcon />} onClick={() => openEdit(item)}>编辑</Button><Button size="small" color="warning" startIcon={<UndoOutlinedIcon />} onClick={() => void withdraw(item)}>撤回</Button></Stack>}</CardContent></Card>)}</Box>
        </Box>}
      </Box>
    </Box>
  );
};

export default HelpPage;
