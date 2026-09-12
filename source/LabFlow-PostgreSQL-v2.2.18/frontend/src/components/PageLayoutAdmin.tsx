import React, { useState, useEffect, useCallback } from 'react';
import {
  Box, Paper, Typography, List, ListItemButton, ListItemIcon, ListItemText,
  Switch, TextField, Button, Snackbar, Alert, Tooltip, IconButton, Divider,
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
import { getSetting, updateSetting } from '../api/client';

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
  const [sel, setSel] = useState<string>('');
  const [layout, setLayout] = useState<PageLayout | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [fields, setFields] = useState<FieldDef[]>([]);
  const [fieldDirty, setFieldDirty] = useState(false);
  const fieldsKey = sel ? PAGE_FIELDS[sel] : undefined;

  const pageDef = PAGES.find(p => p.key === sel);

  const loadLayout = useCallback(async (pageKey: string) => {
    if (!pageKey) return;
    setLoading(true);
    try {
      const r = await getSetting(`layout_${pageKey}`);
      if (r.code === 0 && r.data) {
        const parsed = JSON.parse(r.data.value) as PageLayout;
        if (parsed?.sections) setLayout(parsed);
        else setLayout(null);
      } else setLayout(null);
      // v0.4.44: 同时加载字段布局
      const fk = PAGE_FIELDS[pageKey];
      if (fk) {
        const fr = await getSetting(fk).catch(() => ({} as any));
        if (fr.code === 0 && fr.data) {
          const fp = JSON.parse(fr.data.value) as FieldDef[];
          if (Array.isArray(fp) && fp.length > 0) { setFields(fp); return; }
        }
        setFields(DEFAULT_FIELDS[fk] || []);
      } else setFields([]);
    } catch { setLayout(null); setFields([]); }
    finally { setLoading(false); setDirty(false); setFieldDirty(false); }
  }, []);

  useEffect(() => { if (sel) loadLayout(sel); }, [sel, loadLayout]);

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
    if (!sel || !layout) return;
    setSaving(true);
    try {
      const r = await updateSetting(`layout_${sel}`, layout);
      if (r.code !== 0) throw new Error(r.message || '保存失败');
      setSnackMsg('✅ 布局已保存');
      setSnackErr(false);
      setDirty(false);
    } catch (e: any) {
      setSnackMsg('❌ 保存失败: ' + (e.message || ''));
      setSnackErr(true);
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
    if (!fieldsKey || fields.length === 0) return;
    setSaving(true);
    try {
      const r = await updateSetting(fieldsKey, fields);
      if (r.code !== 0) throw new Error(r.message || '保存失败');
      setSnackMsg('✅ 字段布局已保存');
      setSnackErr(false);
      setFieldDirty(false);
    } catch (e: any) {
      setSnackMsg('❌ 保存字段失败: ' + (e.message || ''));
      setSnackErr(true);
    } finally { setSaving(false); }
  };
  const resetFields = () => {
    if (!fieldsKey) return;
    const def = DEFAULT_FIELDS[fieldsKey];
    if (def) { setFields([...def]); setFieldDirty(true); }
  };

  const resetDefault = () => {
    if (!sel || !layout) return;
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
  };

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
                sx={{ minWidth: 200, '& .MuiOutlinedInput-root': { borderRadius: R } }}
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
    <Box sx={{ display: 'flex', gap: 2, height: 'calc(100vh - 200px)', minHeight: 500 }}>
      {/* 左侧：页面列表 */}
      <Paper elevation={0} sx={{ width: 220, flexShrink: 0, border: '1px solid #e0e0e0', borderRadius: R, overflow: 'auto' }}>
        <Typography variant="subtitle2" sx={{ p: 1.5, fontWeight: 700, bgcolor: '#f5f5f5', borderBottom: '1px solid #e0e0e0' }}>
          选择页面
        </Typography>
        <List dense disablePadding>
          {PAGES.map(p => (
            <ListItemButton
              key={p.key}
              selected={sel === p.key}
              onClick={() => setSel(p.key)}
              sx={{ borderRadius: 0, '&.Mui-selected': { bgcolor: 'rgba(25,118,210,0.08)', '&:hover': { bgcolor: 'rgba(25,118,210,0.12)' } } }}
            >
              <ListItemIcon sx={{ minWidth: 36 }}>{p.icon}</ListItemIcon>
              <ListItemText primary={p.name} primaryTypographyProps={{ variant: 'body2', noWrap: true }} />
            </ListItemButton>
          ))}
        </List>
      </Paper>

      {/* 右侧：编辑区 */}
      <Paper elevation={0} sx={{ flex: 1, border: '1px solid #e0e0e0', borderRadius: R, p: 2, overflow: 'auto' }}>
        {!sel && (
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: '#999', flexDirection: 'column', gap: 1 }}>
            <SettingsIcon sx={{ fontSize: 48, opacity: 0.3 }} />
            <Typography>← 选择左侧页面开始编辑布局</Typography>
          </Box>
        )}

        {sel && loading && (
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', py: 4, color: '#999' }}>
            <Typography>加载中...</Typography>
          </Box>
        )}

        {sel && !loading && (
          <>
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
              <Typography variant="h6" fontWeight={700}>{pageDef?.name || sel}</Typography>
              <Box sx={{ display: 'flex', gap: 1 }}>
                <Tooltip title="重置为默认">
                  <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={resetDefault} sx={{ borderRadius: R }}>
                    重置默认
                  </Button>
                </Tooltip>
                <Tooltip title={dirty ? '有未保存的更改' : ''}>
                  <Button size="small" variant="contained" startIcon={<SaveIcon />} onClick={saveLayout}
                    disabled={saving || !dirty} sx={{ borderRadius: R, bgcolor: dirty ? '#1976d2' : '#ccc' }}>
                    {saving ? '保存中...' : '保存布局'}
                  </Button>
                </Tooltip>
              </Box>
            </Box>
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
                      disabled={!fieldDirty || saving} sx={{ borderRadius: R, bgcolor: fieldDirty ? '#1976d2' : '#ccc' }}>
                      保存字段
                    </Button>
                  </Box>
                </Box>
                <Paper elevation={0} sx={{ border: '1px solid #e0e0e0', borderRadius: R, overflow: 'hidden' }}>
                  <Box sx={{ display: 'flex', bgcolor: '#f5f5f5', px: 1.5, py: 0.5, borderBottom: '1px solid #e0e0e0', fontSize: '0.75rem', fontWeight: 700, color: '#666' }}>
                    <Box sx={{ width: 32 }}></Box>
                    <Box sx={{ flex: 1, minWidth: 80 }}>字段标签</Box>
                    <Box sx={{ width: 80, textAlign: 'center' }}>列宽(px)</Box>
                    <Box sx={{ width: 60, textAlign: 'center' }}>可见</Box>
                    <Box sx={{ width: 40 }}></Box>
                  </Box>
                  {fields.map((field, idx) => (
                    <Box key={field.key || idx} sx={{ display: 'flex', alignItems: 'center', px: 1.5, py: 0.5, borderBottom: '1px solid #f0f0f0', '&:hover': { bgcolor: '#fafafa' } }}>
                      <Box sx={{ width: 32, display: 'flex', gap: 0.5 }}>
                        <IconButton size="small" sx={{ p: 0, cursor: 'grab' }} disabled={idx === 0}
                          onClick={() => moveField(idx, idx - 1)}>
                          <Typography variant="caption">▲</Typography>
                        </IconButton>
                        <IconButton size="small" sx={{ p: 0, cursor: 'grab' }} disabled={idx === fields.length - 1}
                          onClick={() => moveField(idx, idx + 1)}>
                          <Typography variant="caption">▼</Typography>
                        </IconButton>
                      </Box>
                      <TextField size="small" value={field.label}
                        onChange={e => updateField(idx, { label: e.target.value })}
                        sx={{ flex: 1, minWidth: 80, '& .MuiOutlinedInput-root': { borderRadius: R }, '& input': { fontSize: '0.8rem', py: 0.5 } }} />
                      <TextField size="small" type="number" value={field.width}
                        onChange={e => updateField(idx, { width: Math.max(40, Number(e.target.value) || 80) })}
                        sx={{ width: 72, mx: 0.5, '& .MuiOutlinedInput-root': { borderRadius: R }, '& input': { fontSize: '0.8rem', py: 0.5, textAlign: 'center' } }}
                        inputProps={{ min: 40, max: 500 }} />
                      <Switch size="small" checked={field.visible !== false}
                        onChange={(_, v) => updateField(idx, { visible: v })}
                        sx={{ width: 48 }} />
                      <IconButton size="small" onClick={() => removeField(idx)} sx={{ width: 32, color: '#ccc', '&:hover': { color: 'error.main' } }}>
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
