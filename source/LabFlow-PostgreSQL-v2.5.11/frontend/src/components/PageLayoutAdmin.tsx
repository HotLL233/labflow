import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  Box, Paper, Typography, List, ListItemButton, ListItemIcon, ListItemText,
  Switch, TextField, Button, Snackbar, Alert, Tooltip, IconButton, Divider,
  FormControlLabel, MenuItem,
} from '@mui/material';
import HomeIcon from '@mui/icons-material/Home';
import AssessmentIcon from '@mui/icons-material/Assessment';
import ScienceIcon from '@mui/icons-material/Science';
import EditNoteIcon from '@mui/icons-material/EditNote';
import ListAltIcon from '@mui/icons-material/ListAlt';
import FolderIcon from '@mui/icons-material/Folder';
import WorkIcon from '@mui/icons-material/Work';
import DashboardIcon from '@mui/icons-material/Dashboard';
import SettingsIcon from '@mui/icons-material/Settings';
import SaveIcon from '@mui/icons-material/Save';
import RefreshIcon from '@mui/icons-material/Refresh';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import DeleteIcon from '@mui/icons-material/Delete';
import AddIcon from '@mui/icons-material/Add';
import StarIcon from '@mui/icons-material/Star';
import type { PageLayout, SectionConfig } from './PageSectionEditor';
import type { FieldDef } from '../types/layout';
import { useUiDisplay } from '../UiDisplayContext';
import { getSetting, updateSetting } from '../api/client';
import { FONT } from '../constants/designTokens';
import { useManagement, useUnsavedManagement } from './ManagementContext';

const R = '2px';

// 哪些页面包含可编辑的字段布局（pageKey → settings key）
const PAGE_FIELDS: Record<string, string> = {
  'sample_entry': 'layout_sample_entry_fields',
};

// 字段默认值（用于重置）
const DEFAULT_FIELDS: Record<string, FieldDef[]> = {
  'layout_sample_entry_fields': [
    { key: 'user_name', type: 'text', label: '送样人', width: 120, required: false, visible: true, sort_order: 1, placeholder: '' },
    { key: 'division_id', type: 'select', label: '部门', width: 140, required: false, visible: true, sort_order: 2, options: '从用户分组读取' },
    { key: 'lab_name', type: 'text', label: '实验室', width: 150, required: false, visible: true, sort_order: 3, placeholder: '' },
    { key: 'project_name', type: 'text', label: '项目', width: 160, required: false, visible: true, sort_order: 4, placeholder: '' },
    { key: 'detection_type', type: 'select', label: '检测类型', width: 120, required: false, visible: true, sort_order: 5, options: '从检测类型表读取' },
    { key: 'method_name', type: 'text', label: '方法', width: 200, required: false, visible: true, sort_order: 6, placeholder: '' },
    { key: 'quantity', type: 'number', label: '数量', width: 80, required: false, visible: true, sort_order: 7 },
    { key: 'batch_no', type: 'text', label: '批号', width: 100, required: false, visible: true, sort_order: 8, placeholder: '' },
    { key: 'notes', type: 'text', label: '注意事项', width: 150, required: false, visible: true, sort_order: 9, placeholder: '' },
  ],
};

interface PageDef {
  key: string;
  name: string;
  icon: React.ReactNode;
  sections: {
    key: string;
    label: string;
    hasLabel?: boolean;
    hasColor?: boolean;
    hasChipLabel?: boolean;
    hasButtonLabel?: boolean;
    hasActionBtns?: boolean;
  }[];
}

const PAGES: PageDef[] = [
  {
    key: 'home', name: '家页（知微）', icon: <HomeIcon />,
    sections: [
      { key: 'brand-title', label: '品牌标题', hasLabel: true, hasColor: true },
      { key: 'entry-cards', label: '入口卡片区', hasButtonLabel: true },
      { key: 'footer-text', label: '底部文字', hasLabel: true },
    ],
  },
  {
    key: 'stats', name: '分析检测统计', icon: <AssessmentIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'stat-cards', label: '统计卡片', hasButtonLabel: true },
      { key: 'charts', label: '图表区', hasButtonLabel: true },
    ],
  },
  {
    key: 'sample_portal', name: '研发送样入口', icon: <ScienceIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'division-chips', label: '部门筛选 Chip', hasChipLabel: true },
      { key: 'group-cards', label: '实验室卡片', hasButtonLabel: true },
    ],
  },
  {
    key: 'sample_entry', name: '研发送样录入', icon: <EditNoteIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'sample-time', label: '送样时间', hasLabel: true },
      { key: 'action-btns', label: '操作按钮栏', hasActionBtns: true },
      { key: 'entry-table', label: '录入表格', hasButtonLabel: true },
      { key: 'submit-btn', label: '提交按钮', hasLabel: true },
      { key: 'today-records', label: '今日记录', hasButtonLabel: true },
    ],
  },
  {
    key: 'rd_records', name: '研发送样记录', icon: <ListAltIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'table-columns', label: '表格列配置', hasButtonLabel: true },
      { key: 'filters', label: '筛选区', hasButtonLabel: true },
    ],
  },
  {
    key: 'sample_info_home', name: '样品信息入口', icon: <FolderIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'type-cards', label: '类型卡片', hasButtonLabel: true },
    ],
  },
  {
    key: 'sample_info_entry', name: '样品信息登记', icon: <EditNoteIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'form-fields', label: '表单字段区', hasButtonLabel: true },
      { key: 'detection-type-chip', label: '检测类型 Chip', hasChipLabel: true },
      { key: 'seq-no-chip', label: '序号 Chip', hasLabel: true },
      { key: 'sample-time', label: '送样时间', hasLabel: true },
      { key: 'action-btns', label: '操作按钮栏', hasActionBtns: true },
      { key: 'submit-btn', label: '提交按钮', hasLabel: true },
      { key: 'records-table', label: '记录表格', hasButtonLabel: true },
    ],
  },
  {
    key: 'sample_info_stats', name: '样品信息统计', icon: <AssessmentIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'stat-cards', label: '统计卡片', hasButtonLabel: true },
      { key: 'charts', label: '图表区', hasButtonLabel: true },
    ],
  },
  {
    key: 'workload_portal', name: '分析检测入口', icon: <WorkIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'division-chips', label: '部门筛选 Chip', hasChipLabel: true },
      { key: 'group-cards', label: '实验室卡片', hasButtonLabel: true },
    ],
  },
  {
    key: 'sample_stats', name: '研发送样统计', icon: <AssessmentIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'stat-cards', label: '统计卡片', hasButtonLabel: true },
      { key: 'charts', label: '图表区', hasButtonLabel: true },
    ],
  },
  {
    key: 'manage', name: '管理页', icon: <SettingsIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'tabs', label: '标签页区域', hasButtonLabel: true },
    ],
  },
  {
    key: 'entry', name: '分析检测录入', icon: <DashboardIcon />,
    sections: [
      { key: 'page-title', label: '页面标题', hasLabel: true },
      { key: 'form', label: '表单区域', hasButtonLabel: true },
      { key: 'action-btns', label: '操作按钮栏', hasActionBtns: true },
      { key: 'submit-btn', label: '提交按钮', hasLabel: true },
    ],
  },
];

const PageLayoutAdmin: React.FC = () => {
  const management = useManagement();
  const [sel, setSel] = useState<string>(() => management?.readDraft<string>('layout-selection') || '');
  const [layout, setLayout] = useState<PageLayout | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [fields, setFields] = useState<FieldDef[]>([]);
  const [fieldDirty, setFieldDirty] = useState(false);
  const [loadError, setLoadError] = useState('');
  const [restored, setRestored] = useState(false);
  const loadRequest = useRef(0);
  const fieldsKey = sel ? PAGE_FIELDS[sel] : undefined;
  const currentDraft = useRef({ layout, fields, dirty, fieldDirty });
  currentDraft.current = { layout, fields, dirty, fieldDirty };
  // v2.3.18: 业务编号在业务列表里普遍占一行，过于拥挤；这里提供全局显示 / 隐藏开关。
  const { showBusinessNo, setShowBusinessNo } = useUiDisplay();
  const [businessNoSaving, setBusinessNoSaving] = useState(false);

  const toggleBusinessNo = async (value: boolean) => {
    setBusinessNoSaving(true);
    try {
      await setShowBusinessNo(value, true);
      setSnackMsg(value ? '业务编号已设为显示' : '业务编号已设为隐藏');
      setSnackErr(false);
    } catch (e: any) {
      setSnackMsg('保存失败: ' + (e?.message || ''));
      setSnackErr(true);
    } finally {
      setBusinessNoSaving(false);
    }
  };

  const pageDef = PAGES.find(p => p.key === sel);
  const chartHeight = layout?.sections.charts?.chartHeight;
  const invalidChartHeight = sel === 'stats' && chartHeight !== undefined && (!Number.isFinite(chartHeight) || chartHeight < 200 || chartHeight > 600);

  const loadLayout = useCallback(async (pageKey: string) => {
    if (!pageKey) return;
    const request = ++loadRequest.current;
    const cached = management?.readDraft<{ layout: PageLayout; fields: FieldDef[]; dirty: boolean; fieldDirty: boolean }>(`layout:${pageKey}`);
    if (cached) {
      setLayout(cached.layout); setFields(cached.fields); setDirty(cached.dirty); setFieldDirty(cached.fieldDirty);
      setRestored(true); setLoadError(''); setLoading(false); return;
    }
    setLoading(true);
    setLoadError(''); setRestored(false);
    try {
      const r = await getSetting(`layout_${pageKey}`);
      if (request !== loadRequest.current) return;
      if (r.code !== 0) throw new Error(r.message || '页面布局读取失败');
      const parsed = r.data?.value ? JSON.parse(r.data.value) as PageLayout : { sections: {} };
      if (!parsed?.sections) throw new Error('布局配置格式不正确，请核对或明确使用默认配置');
      setLayout(parsed);
      // v0.4.44: 同时加载字段布局
      const fk = PAGE_FIELDS[pageKey];
      if (fk) {
        const fr = await getSetting(fk);
        if (request !== loadRequest.current) return;
        if (fr.code !== 0) throw new Error(fr.message || '字段布局读取失败');
        if (fr.data?.value) {
          const fp = JSON.parse(fr.data.value) as FieldDef[];
          if (!Array.isArray(fp)) throw new Error('字段布局格式不正确');
          setFields(fp); return;
        }
        setFields(DEFAULT_FIELDS[fk] || []);
      } else setFields([]);
    } catch (error: any) { if (request === loadRequest.current) { setLayout(null); setFields([]); setLoadError(error?.message || '页面布局读取失败'); } }
    finally { if (request === loadRequest.current) { setLoading(false); setDirty(false); setFieldDirty(false); } }
  }, [management?.readDraft]);

  useEffect(() => { if (sel) void loadLayout(sel); return () => { ++loadRequest.current; }; }, [sel, loadLayout]);
  useEffect(() => {
    if (sel) management?.writeDraft('layout-selection', sel);
    if (sel && layout && (dirty || fieldDirty)) management?.writeDraft(`layout:${sel}`, { layout, fields, dirty, fieldDirty });
  }, [sel, layout, fields, dirty, fieldDirty, management?.writeDraft]);
  const selectPage = (key: string) => {
    if (key === sel) return;
    const change = () => { setLayout(null); setFields([]); setDirty(false); setFieldDirty(false); setSel(key); };
    if (management) management.guard(change); else if (!(dirty || fieldDirty) || window.confirm('当前布局尚未保存，确认放弃并切换页面吗？')) change();
  };

  const updateSection = (sectionKey: string, patch: Partial<SectionConfig>) => {
    if (!layout) return;
    const updated = {
      ...layout,
      sections: {
        ...layout.sections,
        [sectionKey]: { ...(layout.sections[sectionKey] || { visible: true }), ...patch },
      },
    };
    setLayout(updated);
    setDirty(true);
  };

  const saveLayout = async () => {
    if (!sel || !layout) return false;
    if (invalidChartHeight) { setSnackMsg('图表高度须在 200–600 px 之间'); setSnackErr(true); return false; }
    const submitted = JSON.stringify(layout);
    setSaving(true);
    try {
      const r = await updateSetting(`layout_${sel}`, layout);
      if (r.code !== 0) throw new Error(r.message || '保存失败');
      const unchanged = JSON.stringify(currentDraft.current.layout) === submitted;
      setSnackMsg(unchanged ? '布局已保存' : '本次提交已保存，后续编辑仍待保存');
      setSnackErr(false);
      setDirty(!unchanged);
      if (unchanged && !currentDraft.current.fieldDirty) management?.clearDraft(`layout:${sel}`);
      return unchanged;
    } catch (e: any) {
      setSnackMsg('保存失败: ' + (e.message || ''));
      setSnackErr(true);
      return false;
    } finally { setSaving(false); }
  };

  // 字段编辑
  const updateField = (idx: number, patch: Partial<FieldDef>) => {
    setFields(prev => prev.map((f, i) => i === idx ? { ...f, ...patch } : f));
    setFieldDirty(true);
  };
  const removeField = (idx: number) => {
    setFields(prev => prev.filter((_, i) => i !== idx).map((f, i) => ({ ...f, sort_order: i + 1 })));
    setFieldDirty(true);
  };
  const addField = () => {
    const newField: FieldDef = { key: `field_${Date.now()}`, type: 'text', label: '新字段', width: 120, required: false, visible: true, sort_order: fields.length + 1, placeholder: '' };
    setFields(prev => [...prev, newField]);
    setFieldDirty(true);
  };
  const moveField = (from: number, to: number) => {
    if (from === to || to < 0 || to >= fields.length) return;
    setFields(prev => {
      const arr = [...prev];
      const [item] = arr.splice(from, 1);
      arr.splice(to, 0, item);
      return arr.map((f, i) => ({ ...f, sort_order: i + 1 }));
    });
    setFieldDirty(true);
  };
  const saveFields = async () => {
    if (!fieldsKey || fields.length === 0) return false;
    const submitted = JSON.stringify(fields);
    setSaving(true);
    try {
      const r = await updateSetting(fieldsKey, fields);
      if (r.code !== 0) throw new Error(r.message || '保存失败');
      const unchanged = JSON.stringify(currentDraft.current.fields) === submitted;
      setSnackMsg(unchanged ? '字段布局已保存' : '本次字段已保存，后续编辑仍待保存');
      setSnackErr(false);
      setFieldDirty(!unchanged);
      if (unchanged && !currentDraft.current.dirty) management?.clearDraft(`layout:${sel}`);
      return unchanged;
    } catch (e: any) {
      setSnackMsg('保存字段失败: ' + (e.message || ''));
      setSnackErr(true);
      return false;
    } finally { setSaving(false); }
  };
  const resetFields = () => {
    if (!fieldsKey) return;
    const def = DEFAULT_FIELDS[fieldsKey];
    if (def) { setFields([...def]); setFieldDirty(true); }
  };

  const resetDefault = () => {
    if (!sel) return;
    const reset: PageLayout = { sections: {} };
    const def = PAGES.find(p => p.key === sel);
    if (def) {
      def.sections.forEach(s => {
        reset.sections[s.key] = { visible: true };
        if (s.hasLabel) reset.sections[s.key].label = s.label;
      });
    }
    setLayout(reset);
    setDirty(true);
    setLoadError('');
  };
  useUnsavedManagement(`layout:${sel}`, {
    label: `${pageDef?.name || '页面'}布局与字段`, dirty: dirty || fieldDirty,
    save: async () => {
      if (dirty && !await saveLayout()) return false;
      if (fieldDirty && !await saveFields()) return false;
      return true;
    },
    discard: () => { setDirty(false); setFieldDirty(false); },
  });

  const renderSectionEditor = (sec: PageDef['sections'][0]) => {
    if (!layout) return null;
    const cfg = layout.sections[sec.key] || { visible: true };
    return (
      <Paper
        key={sec.key}
        elevation={0}
        sx={{
          p: 2, mb: 1.5, border: '1px solid #e0e0e0', borderRadius: R,
          ...((!cfg.visible) && { opacity: 0.5, bgcolor: '#fafafa' }),
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1, flexWrap: 'wrap' }}>
          <Switch
            size="small"
            checked={cfg.visible !== false}
            onChange={(_, v) => updateSection(sec.key, { visible: v })}
          />
          <Typography variant="subtitle2" sx={{ fontWeight: 700, flex: 1 }}>
            {cfg.label || sec.label}
          </Typography>
          <Typography variant="caption" color="text.secondary">
            section: {sec.key}
          </Typography>
        </Box>

        {(sec.hasLabel || sec.hasChipLabel || sec.hasButtonLabel) && (
          <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mb: 1 }}>
            {sec.hasLabel && (
              <TextField
                size="small" label="显示文本"
                value={cfg.label ?? ''}
                onChange={e => updateSection(sec.key, { label: e.target.value || undefined })}
                sx={{ minWidth: { xs: 0, sm: 200 }, width: { xs: '100%', sm: 'auto' }, '& .MuiOutlinedInput-root': { borderRadius: R } }}
              />
            )}
            {sec.hasColor && (
              <TextField
                size="small" label="颜色" type="color"
                value={cfg.color ?? '#1976d2'}
                onChange={e => updateSection(sec.key, { color: e.target.value })}
                sx={{ width: 80, '& .MuiOutlinedInput-root': { borderRadius: R } }}
                inputProps={{ style: { padding: '2px 4px', height: 32 } }}
              />
            )}
            {sec.hasChipLabel && (
              <TextField
                size="small" label="Chip 文字"
                value={cfg.chipLabel ?? ''}
                onChange={e => updateSection(sec.key, { chipLabel: e.target.value || undefined })}
                sx={{ minWidth: 150, '& .MuiOutlinedInput-root': { borderRadius: R } }}
              />
            )}
            {sec.hasButtonLabel && (
              <TextField
                size="small" label="按钮文字"
                value={cfg.buttonLabel ?? ''}
                onChange={e => updateSection(sec.key, { buttonLabel: e.target.value || undefined })}
                sx={{ minWidth: 150, '& .MuiOutlinedInput-root': { borderRadius: R } }}
              />
            )}
          </Box>
        )}

        {sel === 'stats' && sec.key === 'charts' && (
          <TextField
            size="small" type="number" label="图表高度（px）"
            value={cfg.chartHeight ?? 240}
            onChange={e => updateSection(sec.key, { chartHeight: e.target.value === '' ? undefined : Number(e.target.value) })}
            inputProps={{ min: 200, max: 600, step: 10 }}
            error={invalidChartHeight}
            helperText="200–600 px；未设置时使用 240 px"
            sx={{ width: { xs: '100%', sm: 240 }, mb: 1, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
        )}

        {sec.hasActionBtns && (
          <Box>
            <Typography variant="caption" color="text.secondary" sx={{ mb: 0.5, display: 'block' }}>
              操作按钮文字
            </Typography>
            <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
              <TextField
                size="small" label="添加行按钮"
                value={cfg.actionButtons?.addLabel ?? ''}
                onChange={e => updateSection(sec.key, {
                  actionButtons: { ...(cfg.actionButtons || {}), addLabel: e.target.value || undefined },
                })}
                sx={{ minWidth: 150, '& .MuiOutlinedInput-root': { borderRadius: R } }}
              />
              <TextField
                size="small" label="删除按钮"
                value={cfg.actionButtons?.deleteLabel ?? ''}
                onChange={e => updateSection(sec.key, {
                  actionButtons: { ...(cfg.actionButtons || {}), deleteLabel: e.target.value || undefined },
                })}
                sx={{ minWidth: 150, '& .MuiOutlinedInput-root': { borderRadius: R } }}
              />
              <TextField
                size="small" label="重置按钮"
                value={cfg.actionButtons?.resetLabel ?? ''}
                onChange={e => updateSection(sec.key, {
                  actionButtons: { ...(cfg.actionButtons || {}), resetLabel: e.target.value || undefined },
                })}
                sx={{ minWidth: 150, '& .MuiOutlinedInput-root': { borderRadius: R } }}
              />
            </Box>
          </Box>
        )}
      </Paper>
    );
  };

  return (
    <Box sx={{ display: 'grid', gap: 2, gridTemplateColumns: { xs: 'minmax(0,1fr)', lg: '200px minmax(0,1fr)' }, alignItems: 'start', minWidth: 0 }}>
      <TextField select size="small" label="配置页面" value={sel} onChange={event => selectPage(event.target.value)} sx={{ display: { xs: 'block', lg: 'none' } }} fullWidth>
        <MenuItem value="">选择页面</MenuItem>{PAGES.map(page => <MenuItem key={page.key} value={page.key}>{page.name}</MenuItem>)}
      </TextField>
      {/* 左侧：页面列表 */}
      <Paper elevation={0} sx={{ display: { xs: 'none', lg: 'block' }, minWidth: 0, border: '1px solid', borderColor: 'divider', borderRadius: R }}>
        <Typography variant="subtitle2" sx={{ p: 1.5, fontWeight: 700, bgcolor: '#f5f5f5', borderBottom: '1px solid #e0e0e0' }}>
          选择页面
        </Typography>
        <List dense disablePadding>
          {PAGES.map(p => (
            <ListItemButton
              key={p.key}
              selected={sel === p.key}
              onClick={() => selectPage(p.key)}
              sx={{ borderRadius: 0, '&.Mui-selected': { bgcolor: 'rgba(25,118,210,0.08)', '&:hover': { bgcolor: 'rgba(25,118,210,0.12)' } } }}
            >
              <ListItemIcon sx={{ minWidth: 36 }}>{p.icon}</ListItemIcon>
              <ListItemText primary={p.name} primaryTypographyProps={{ variant: 'body2', noWrap: true }} />
            </ListItemButton>
          ))}
        </List>
      </Paper>

      {/* 右侧：编辑区 */}
      <Paper elevation={0} sx={{ minWidth: 0, border: '1px solid', borderColor: 'divider', borderRadius: R, p: { xs: 1.5, sm: 2 } }}>
        {loadError && <Alert severity="error" sx={{ mb: 1.5 }} action={<Button onClick={() => void loadLayout(sel)}>重试</Button>}>{loadError}<Button size="small" onClick={resetDefault}>使用默认配置（待保存）</Button></Alert>}
        {restored && <Alert severity="info" sx={{ mb: 1.5 }}>已恢复本次会话未保存草稿，尚未写入系统。</Alert>}
        <Paper elevation={0} sx={{ p: 1.5, mb: 2, border: '1px solid #bbdefb', borderRadius: R, bgcolor: '#f7fbff' }}>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1.5, flexWrap: 'wrap' }}>
            <Box sx={{ minWidth: 0, flex: '1 1 200px' }}>
              <Typography variant="subtitle2" fontWeight={700}>业务编号显示</Typography>
              <Typography variant="caption" color="text.secondary">
                全局开关：关闭后所有业务列表不再在序号 / 时间下方显示业务编号，列表更紧凑；审计日志的编号列不受影响。
              </Typography>
            </Box>
            <FormControlLabel
              sx={{ mr: 0 }}
              control={<Switch checked={showBusinessNo} disabled={businessNoSaving} onChange={event => void toggleBusinessNo(event.target.checked)} />}
              label={showBusinessNo ? '显示业务编号' : '隐藏业务编号'}
            />
          </Box>
        </Paper>
        {!sel && (
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', py: 4, color: 'text.secondary', flexDirection: 'column', gap: 1 }}>
            <SettingsIcon sx={{ fontSize: 48, opacity: 0.3 }} />
            <Typography>选择配置页面后编辑区块与文案</Typography>
          </Box>
        )}

        {sel && loading && (
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', py: 4, color: '#757575' }}>
            <Typography>加载中...</Typography>
          </Box>
        )}

        {sel && !loading && (
          <>
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2, gap: 1, flexWrap: 'wrap' }}>
              <Typography variant="h6" fontWeight={700}>{pageDef?.name || sel}</Typography>
              <Box sx={{ display: 'flex', gap: 1 }}>
                <Tooltip title="重置为默认">
                  <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={resetDefault} sx={{ borderRadius: R }}>
                    重置默认
                  </Button>
                </Tooltip>
                <Tooltip title={dirty ? '有未保存的更改' : ''}>
                  <Button size="small" variant="contained" startIcon={<SaveIcon />} onClick={saveLayout}
                    disabled={saving || !dirty} sx={{ borderRadius: R }}>
                    {saving ? '保存中...' : '保存布局'}
                  </Button>
                </Tooltip>
              </Box>
            </Box>
            <Typography variant="caption" color={dirty || fieldDirty ? 'warning.main' : 'text.secondary'} sx={{ display: 'block', mb: 1.5 }}>作用于当前页面；区块布局与录入字段分别保存。{dirty || fieldDirty ? ' 有未保存修改' : ' 当前无未保存修改'}</Typography>
            <Divider sx={{ mb: 2 }} />
            {pageDef?.sections.map(renderSectionEditor)}

            {/* 字段编辑（仅对有字段的页面显示） */}
            {fieldsKey && fields.length > 0 && (
              <>
                <Divider sx={{ my: 2, borderStyle: 'dashed' }} />
                <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
                  <Typography variant="subtitle2" fontWeight={700} color="text.secondary">
                    录入表格字段布局
                  </Typography>
                  <Box sx={{ display: 'flex', gap: 1 }}>
                    <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={resetFields} sx={{ borderRadius: R }}>
                      重置字段
                    </Button>
                    <Button size="small" variant="contained" startIcon={<SaveIcon />} onClick={saveFields}
                      disabled={!fieldDirty || saving} sx={{ borderRadius: R }}>
                      保存字段
                    </Button>
                  </Box>
                </Box>
                <Paper elevation={0} sx={{ border: '1px solid #e0e0e0', borderRadius: R, overflow: 'hidden' }}>
                  <Box sx={{ display: 'flex', bgcolor: '#f5f5f5', px: 1.5, py: 0.5, borderBottom: '1px solid #e0e0e0', fontSize: FONT.xs, fontWeight: 700, color: '#666' }}>
                    <Box sx={{ width: 32 }}></Box>
                    <Box sx={{ flex: 1, minWidth: 80 }}>字段标签</Box>
                    <Box sx={{ width: 80, textAlign: 'center' }}>列宽(px)</Box>
                    <Box sx={{ width: 60, textAlign: 'center' }}>可见</Box>
                    <Box sx={{ width: 40 }}></Box>
                  </Box>
                  {fields.map((field, idx) => (
                    <Box key={field.key || idx} sx={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 0.5, px: 1.5, py: 1, borderBottom: '1px solid', borderColor: 'divider', '&:hover': { bgcolor: 'action.hover' } }}>
                      <Box sx={{ width: 32, display: 'flex', gap: 0.5 }}>
                        <IconButton size="small" aria-label={`上移${field.label}`} sx={{ p: 0, cursor: 'grab' }} disabled={idx === 0}
                          onClick={() => moveField(idx, idx - 1)}>
                          <Typography variant="caption">▲</Typography>
                        </IconButton>
                        <IconButton size="small" aria-label={`下移${field.label}`} sx={{ p: 0, cursor: 'grab' }} disabled={idx === fields.length - 1}
                          onClick={() => moveField(idx, idx + 1)}>
                          <Typography variant="caption">▼</Typography>
                        </IconButton>
                      </Box>
                      <TextField size="small" value={field.label}
                        onChange={e => updateField(idx, { label: e.target.value })}
                        label="字段标签" sx={{ flex: '1 1 120px', minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: R }, '& input': { fontSize: FONT.sm, py: 0.5 } }} />
                      <TextField size="small" type="number" value={field.width}
                        onChange={e => updateField(idx, { width: Math.max(40, Number(e.target.value) || 80) })}
                        label="列宽(px)" sx={{ width: 84, mx: 0.5, '& .MuiOutlinedInput-root': { borderRadius: R }, '& input': { fontSize: FONT.sm, py: 0.5, textAlign: 'center' } }}
                        inputProps={{ min: 40, max: 500 }} />
                      <Switch size="small" inputProps={{ 'aria-label': `${field.label}可见` }} checked={field.visible !== false}
                        onChange={(_, v) => updateField(idx, { visible: v })}
                        sx={{ width: 48 }} />
                      <IconButton size="small" aria-label={`删除${field.label}`} onClick={() => removeField(idx)} sx={{ width: 32, color: 'text.secondary', '&:hover': { color: 'error.main' } }}>
                        <DeleteIcon fontSize="small" />
                      </IconButton>
                    </Box>
                  ))}
                </Paper>
                <Button size="small" variant="outlined" startIcon={<AddIcon />} onClick={addField}
                  sx={{ mt: 1, borderRadius: R, borderStyle: 'dashed' }}>
                  添加字段
                </Button>
              </>
            )}
          </>
        )}
      </Paper>

      <Snackbar open={!!snackMsg} autoHideDuration={4000} onClose={() => setSnackMsg('')}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snackErr ? 'error' : 'success'} onClose={() => setSnackMsg('')} sx={{ borderRadius: R }}>
          {snackMsg}
        </Alert>
      </Snackbar>
    </Box>
  );
};

export default PageLayoutAdmin;
