import { useEffect, useState } from 'react';
import {
  Alert, Autocomplete, Box, Button, Checkbox, CircularProgress, Dialog, DialogActions,
  DialogContent, DialogTitle, FormControl, InputLabel, MenuItem, Paper, Select, Stack,
  Tab, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Tabs, TextField, Typography,
} from '@mui/material';
import {
  getDivisions, getGroups, getInstruments, getMethods, getMethodTypes, getProjects, getSampleInfoColumns,
  getSampleInfoTypesAll, getSettings, requireApiSuccess, updateSetting,
} from '../api/client';
import { useUser } from '../UserContext';
import type { Method, MethodType, Project, ProjectGroup, SampleInfoColumn, SampleInfoType } from '../types';

const SETTING_KEY = 'sample_info_workload_mapping';
type TypeMapping = { type_key: string; method_type_ids: number[] };
type FieldMapping = { type_key: string; field_key: string; value: string; method_id: number };
type ProjectAlias = { alias: string; project_id: number; group_id?: number | null };
type Mapping = { type_mappings: TypeMapping[]; field_mappings: FieldMapping[]; project_aliases: ProjectAlias[] };
type Section = keyof Mapping;
type Editor = { section: Section; index: number; type: TypeMapping; field: FieldMapping; project: ProjectAlias };
const emptyMapping = (): Mapping => ({ type_mappings: [], field_mappings: [], project_aliases: [] });
const sections: Array<{ key: Section; label: string }> = [
  { key: 'type_mappings', label: '登记类型映射' },
  { key: 'field_mappings', label: '字段值映射' },
  { key: 'project_aliases', label: '项目别名' },
];

function readMapping(value: string): Mapping {
  const parsed = JSON.parse(value);
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)
    || Object.keys(parsed).some(key => !sections.some(section => section.key === key))) throw new Error('映射格式无效');
  const mapping: Mapping = { ...emptyMapping(), ...parsed };
  if (!Array.isArray(mapping.type_mappings) || !Array.isArray(mapping.field_mappings) || !Array.isArray(mapping.project_aliases)
    || mapping.type_mappings.some(item => !item || typeof item.type_key !== 'string' || !Array.isArray(item.method_type_ids) || item.method_type_ids.some(id => !Number.isSafeInteger(id) || id <= 0))
    || mapping.field_mappings.some(item => !item || typeof item.type_key !== 'string' || typeof item.field_key !== 'string' || typeof item.value !== 'string' || !Number.isSafeInteger(item.method_id) || item.method_id <= 0)
    || mapping.project_aliases.some(item => !item || typeof item.alias !== 'string' || !Number.isSafeInteger(item.project_id) || item.project_id <= 0 || (item.group_id != null && (!Number.isSafeInteger(item.group_id) || item.group_id <= 0)))) throw new Error('映射格式无效');
  return mapping;
}

export default function SampleWorkloadMappingSettings() {
  const { user } = useUser();
  const [mapping, setMapping] = useState<Mapping | null>(null);
  const [types, setTypes] = useState<SampleInfoType[]>([]);
  const [methodTypes, setMethodTypes] = useState<MethodType[]>([]);
  const [methods, setMethods] = useState<Method[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [columns, setColumns] = useState<SampleInfoColumn[]>([]);
  const [section, setSection] = useState<Section>('type_mappings');
  const [editor, setEditor] = useState<Editor | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [invalidConfig, setInvalidConfig] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  useEffect(() => {
    if (!user?.is_admin) { setLoading(false); return; }
    let cancelled = false;
    void Promise.all([getSettings(), getSampleInfoTypesAll(), getMethodTypes(), getMethods(), getProjects({ status: 'ongoing', active_only: true }), getGroups(), getSampleInfoColumns(), getInstruments(), getDivisions()])
      .then(responses => {
        if (cancelled) return;
        const [settings, typeRows, methodTypeRows, methodRows, projectRows, groupRows, columnRows, instrumentRows, divisionRows] = responses;
        const saved = (requireApiSuccess(settings, '读取映射配置失败') ?? []).find(item => item.key === SETTING_KEY);
        setTypes((requireApiSuccess(typeRows) ?? []).filter(item => Boolean(item.is_active)));
        setMethodTypes(requireApiSuccess(methodTypeRows) ?? []);
        const activeInstruments = new Set((requireApiSuccess(instrumentRows) ?? []).filter(item => item.is_active).map(item => item.id));
        setMethods((requireApiSuccess(methodRows) ?? []).filter(item => item.is_active && (!item.instrument_id || activeInstruments.has(item.instrument_id))));
        const activeDivisions = new Set((requireApiSuccess(divisionRows) ?? []).filter(item => item.is_active !== false).map(item => item.id));
        const availableGroups = (requireApiSuccess(groupRows) ?? []).filter(item => item.division_id != null && activeDivisions.has(item.division_id));
        setGroups(availableGroups);
        setProjects((requireApiSuccess(projectRows) ?? []).filter(item => item.is_active !== false && item.project_status !== 'archived' && availableGroups.some(group =>
          item.lab_ids.includes(group.id) && (item.project_division_id == null || item.project_division_id === group.division_id || (item.collaboration_division_ids ?? []).includes(group.division_id!)))));
        setColumns((requireApiSuccess(columnRows) ?? []).filter(item => item.is_active && ['text', 'select', 'multi_select', 'multiselect', 'textarea'].includes(item.data_type)));
        try { setMapping(saved ? readMapping(saved.value) : emptyMapping()); }
        catch { setInvalidConfig(true); setError('已保存的映射配置格式损坏。原值仍保留，可由管理员重新配置后保存替换。'); }
      }).catch(reason => { if (!cancelled) setError(reason instanceof Error ? reason.message : '映射配置加载失败'); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [user?.is_admin]);

  if (!user?.is_admin) return <Alert severity="warning">仅系统管理员可维护工作量映射。</Alert>;

  const typeLabel = (key: string) => types.find(item => item.type_key === key)?.label || `${key}（不可用）`;
  const methodLabel = (id: number) => {
    const method = methods.find(item => item.id === id);
    return method ? `${method.full_name || method.name} · ${method.instrument_code || method.instrument_name || '无仪器'}` : `方法 #${id}（不可用）`;
  };
  const projectLabel = (id: number) => projects.find(item => item.id === id)?.name || `项目 #${id}（不可用）`;
  const groupLabel = (id?: number | null) => id ? groups.find(item => item.id === id)?.name || `实验室 #${id}（不可用）` : '全部关联实验室';
  const fieldLabel = (key: string) => columns.find(item => item.field_key === key)?.label || `${key}（不可用）`;
  const openEditor = (index: number) => {
    setEditor({ section, index,
      type: section === 'type_mappings' && index >= 0 ? mapping!.type_mappings[index] : { type_key: '', method_type_ids: [] },
      field: section === 'field_mappings' && index >= 0 ? mapping!.field_mappings[index] : { type_key: '', field_key: '', value: '', method_id: 0 },
      project: section === 'project_aliases' && index >= 0 ? mapping!.project_aliases[index] : { alias: '', project_id: 0 },
    });
  };
  const acceptEditor = () => {
    if (!editor || !mapping) return;
    const updated = { ...mapping };
    if (editor.section === 'type_mappings') {
      updated.type_mappings = editor.index < 0 ? [...mapping.type_mappings, editor.type] : mapping.type_mappings.map((item, index) => index === editor.index ? editor.type : item);
    } else if (editor.section === 'field_mappings') {
      const field = { ...editor.field, value: editor.field.value.trim() };
      updated.field_mappings = editor.index < 0 ? [...mapping.field_mappings, field] : mapping.field_mappings.map((item, index) => index === editor.index ? field : item);
    } else {
      const project = { ...editor.project, alias: editor.project.alias.trim() };
      updated.project_aliases = editor.index < 0 ? [...mapping.project_aliases, project] : mapping.project_aliases.map((item, index) => index === editor.index ? project : item);
    }
    setMapping(updated); setDirty(true); setMessage(''); setEditor(null);
  };
  const remove = (index: number) => {
    if (!mapping) return;
    setMapping({ ...mapping, [section]: mapping[section].filter((_, itemIndex) => itemIndex !== index) });
    setDirty(true); setMessage('');
  };
  const save = async () => {
    if (!mapping || saving) return;
    setSaving(true); setError(''); setMessage('');
    try { requireApiSuccess(await updateSetting(SETTING_KEY, mapping), '保存映射失败'); setDirty(false); setMessage('映射已保存，对所有用户的新录入生效；历史工作量保持不变。'); }
    catch (reason) { setError(reason instanceof Error ? reason.message : '保存失败，编辑内容已保留。'); }
    finally { setSaving(false); }
  };
  const validEditor = editor && (editor.section === 'type_mappings' ? Boolean(editor.type.type_key && editor.type.method_type_ids.length)
    : editor.section === 'field_mappings' ? Boolean(editor.field.type_key && editor.field.field_key && editor.field.value.trim() && editor.field.method_id)
      : Boolean(editor.project.alias.trim() && editor.project.project_id));
  const editableColumns = columns.filter(item => editor && ((item.visible_types ?? []).includes(editor.field.type_key)));
  const selectedProject = editor && projects.find(item => item.id === editor.project.project_id);
  const rowStyle = { overflowWrap: 'anywhere', minWidth: 0 } as const;

  return <Stack spacing={1.5}>
    <Typography variant="subtitle2" fontWeight={700}>样品信息登记 · 工作量映射</Typography>
    <Alert severity="info">映射用于推荐标准项目、方法和检测类型，录入人员仍可在授权范围内确认选择。登记表单、自定义列选项和已确认的历史工作量不会被改写。</Alert>
    {error && <Alert severity="error" onClose={() => setError('')}>{error}</Alert>}
    {message && <Alert severity="success" onClose={() => setMessage('')}>{message}</Alert>}
    {loading ? <CircularProgress size={24} /> : invalidConfig ? <Button variant="outlined" onClick={() => { setMapping(emptyMapping()); setInvalidConfig(false); setDirty(true); setError(''); }}>重新配置（保存后替换损坏配置）</Button> : !mapping ? <Typography color="text.secondary">读取失败，请重新进入此页面后重试。</Typography> : <>
      <Paper variant="outlined" sx={{ minWidth: 0 }}>
        <Tabs value={section} onChange={(_, value: Section) => setSection(value)} variant="scrollable" scrollButtons="auto">{sections.map(item => <Tab key={item.key} value={item.key} label={item.label} />)}</Tabs>
        <Box sx={{ p: 1.5 }}>
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1 }}>{section === 'type_mappings' ? '建立登记类型与方法库类型的对应关系，匹配仅决定推荐顺序。' : section === 'field_mappings' ? '明确指定某个登记字段及填写值对应的标准方法；不会扫描备注等无关文本。' : '将项目代号或别名关联到标准项目，可限定实验室；同一范围不允许冲突。'}</Typography>
          <TableContainer sx={{ overflowX: 'hidden' }}><Table size="small" sx={{ tableLayout: 'fixed', width: '100%' }}>
            <TableHead><TableRow><TableCell sx={rowStyle}>{section === 'project_aliases' ? '登记项目别名' : '登记类型'}</TableCell><TableCell sx={rowStyle}>{section === 'field_mappings' ? '字段 / 填写值' : section === 'project_aliases' ? '标准项目 / 实验室' : '方法库类型'}</TableCell>{section === 'field_mappings' && <TableCell sx={rowStyle}>标准方法 / 仪器</TableCell>}<TableCell align="right" sx={{ width: 112 }}>操作</TableCell></TableRow></TableHead>
            <TableBody>
              {mapping[section].length === 0 && <TableRow><TableCell colSpan={section === 'field_mappings' ? 4 : 3}>尚未配置，录入时仍可手动选择授权范围内的标准项目和方法。</TableCell></TableRow>}
              {section === 'type_mappings' && mapping.type_mappings.map((item, index) => <TableRow key={index}><TableCell sx={rowStyle}>{typeLabel(item.type_key)}</TableCell><TableCell sx={rowStyle}>{item.method_type_ids.map(id => methodTypes.find(type => type.id === id)?.name || `类型 #${id}（不可用）`).join('、')}</TableCell><TableCell align="right"><Button size="small" disabled={saving} onClick={() => openEditor(index)}>编辑</Button><Button size="small" color="error" disabled={saving} onClick={() => remove(index)}>删除</Button></TableCell></TableRow>)}
              {section === 'field_mappings' && mapping.field_mappings.map((item, index) => <TableRow key={index}><TableCell sx={rowStyle}>{typeLabel(item.type_key)}</TableCell><TableCell sx={rowStyle}>{fieldLabel(item.field_key)} / {item.value}</TableCell><TableCell sx={rowStyle}>{methodLabel(item.method_id)}</TableCell><TableCell align="right"><Button size="small" disabled={saving} onClick={() => openEditor(index)}>编辑</Button><Button size="small" color="error" disabled={saving} onClick={() => remove(index)}>删除</Button></TableCell></TableRow>)}
              {section === 'project_aliases' && mapping.project_aliases.map((item, index) => <TableRow key={index}><TableCell sx={rowStyle}>{item.alias}</TableCell><TableCell sx={rowStyle}>{projectLabel(item.project_id)} / {groupLabel(item.group_id)}</TableCell><TableCell align="right"><Button size="small" disabled={saving} onClick={() => openEditor(index)}>编辑</Button><Button size="small" color="error" disabled={saving} onClick={() => remove(index)}>删除</Button></TableCell></TableRow>)}
            </TableBody>
          </Table></TableContainer>
          <Button size="small" variant="outlined" sx={{ mt: 1 }} disabled={saving} onClick={() => openEditor(-1)}>新增映射</Button>
        </Box>
      </Paper>
      <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, alignItems: 'center' }}><Typography variant="caption" color="text.secondary">{dirty ? '有未保存的映射更改' : '配置已同步'}</Typography><Button variant="contained" disabled={saving || !dirty} onClick={() => void save()}>{saving ? '保存中…' : '保存映射'}</Button></Box>
    </>}
    <Dialog open={Boolean(editor)} onClose={() => setEditor(null)} fullWidth maxWidth="sm">
      <DialogTitle>{editor?.index === -1 ? '新增' : '编辑'}{sections.find(item => item.key === editor?.section)?.label}</DialogTitle>
      <DialogContent><Stack spacing={2} sx={{ pt: 1 }}>
        {editor && editor.section !== 'project_aliases' && <TextField select size="small" label="登记检测类型" value={editor.section === 'type_mappings' ? editor.type.type_key : editor.field.type_key} onChange={event => setEditor({ ...editor, type: { ...editor.type, type_key: event.target.value }, field: { ...editor.field, type_key: event.target.value, field_key: '' } })}>{types.map(item => <MenuItem key={item.type_key} value={item.type_key}>{item.label}</MenuItem>)}</TextField>}
        {editor?.section === 'type_mappings' && <FormControl size="small"><InputLabel>方法库检测类型</InputLabel><Select multiple label="方法库检测类型" value={editor.type.method_type_ids} onChange={event => setEditor({ ...editor, type: { ...editor.type, method_type_ids: (event.target.value as number[]).map(Number) } })} renderValue={ids => ids.map(id => methodTypes.find(item => item.id === id)?.name || `#${id}`).join('、')}>{methodTypes.map(item => <MenuItem key={item.id} value={item.id}><Checkbox checked={editor.type.method_type_ids.includes(item.id)} />{item.name}</MenuItem>)}</Select></FormControl>}
        {editor?.section === 'field_mappings' && <>
          <TextField select size="small" label="登记字段" value={editor.field.field_key} disabled={!editor.field.type_key} onChange={event => setEditor({ ...editor, field: { ...editor.field, field_key: event.target.value } })} helperText="仅列出对此检测类型可见的文本或选项列，不改动列配置">{editableColumns.map(item => <MenuItem key={item.id} value={item.field_key}>{item.label}（{item.field_key}）</MenuItem>)}</TextField>
          <TextField size="small" label="登记填写值或选项" value={editor.field.value} onChange={event => setEditor({ ...editor, field: { ...editor.field, value: event.target.value } })} />
          <Autocomplete size="small" options={methods} value={methods.find(item => item.id === editor.field.method_id) ?? null} getOptionLabel={item => methodLabel(item.id)} isOptionEqualToValue={(a, b) => a.id === b.id} onChange={(_, item) => setEditor({ ...editor, field: { ...editor.field, method_id: item?.id ?? 0 } })} renderInput={params => <TextField {...params} label="标准方法 / 仪器" />} />
        </>}
        {editor?.section === 'project_aliases' && <>
          <TextField size="small" label="登记项目别名或代号" value={editor.project.alias} onChange={event => setEditor({ ...editor, project: { ...editor.project, alias: event.target.value } })} />
          <Autocomplete size="small" options={projects} value={selectedProject || null} getOptionLabel={item => item.name} isOptionEqualToValue={(a, b) => a.id === b.id} onChange={(_, item) => setEditor({ ...editor, project: { ...editor.project, project_id: item?.id ?? 0, group_id: undefined } })} renderInput={params => <TextField {...params} label="标准项目" />} />
          <TextField select size="small" label="限定实验室" value={editor.project.group_id ?? ''} disabled={!selectedProject} onChange={event => setEditor({ ...editor, project: { ...editor.project, group_id: event.target.value ? Number(event.target.value) : undefined } })} helperText="仅有效执行关联可保存；历史项目缺少所属部门时由保存校验核对"><MenuItem value="">全部有效关联实验室</MenuItem>{groups.filter(item => selectedProject?.lab_ids.includes(item.id) && (selectedProject.project_division_id == null || selectedProject.project_division_id === item.division_id || (selectedProject.collaboration_division_ids ?? []).includes(item.division_id!))).map(item => <MenuItem key={item.id} value={item.id}>{item.name}</MenuItem>)}</TextField>
        </>}
      </Stack></DialogContent>
      <DialogActions><Button onClick={() => setEditor(null)}>取消</Button><Button variant="contained" disabled={!validEditor} onClick={acceptEditor}>应用到待保存配置</Button></DialogActions>
    </Dialog>
  </Stack>;
}
