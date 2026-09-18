import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import dayjs from 'dayjs';
import {
  Box, Typography, Paper, TextField, Button, Grid, IconButton,
  Chip, Snackbar, Alert, Select, MenuItem, FormControl, InputLabel,
  TablePagination, Collapse, CircularProgress, Checkbox, Table,
  TableBody, TableCell, TableContainer, TableHead, TableRow,
  Switch, LinearProgress, Dialog, DialogContent, DialogTitle,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import AddIcon from '@mui/icons-material/Add';
import DeleteIcon from '@mui/icons-material/Delete';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import CloseIcon from '@mui/icons-material/Close';
import DescriptionIcon from '@mui/icons-material/Description';
import DownloadIcon from '@mui/icons-material/Download';
import ScienceIcon from '@mui/icons-material/Science';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import {
  getSampleInfoRecords, createSampleInfoWithAttachments, updateSampleInfo, sampleSampleInfo, withdrawSampleInfo, completeSampleInfo,
  returnSampleInfo, confirmSampleInfoReturn,
  getSampleInfoTypes, getCurrentRoleDataScopes, getDivisions, getGroups, getActiveSampleInfoColumns,
  getSampleInfoAttachments, uploadSampleInfoAttachment,
  downloadSampleInfoAttachment,
  getSampleInfoAttachmentImagePreview, getSampleInfoAttachmentPreviewPage,
  deleteSampleInfoAttachment, batchGetSampleInfoAttachments,
  getProjects, getSetting, getSampleInfoDrafts,
  createSampleInfoDraft, updateSampleInfoDraft, deleteSampleInfoDraft, getSampleInfoDraftAttachments,
  uploadSampleInfoDraftAttachment, downloadSampleInfoDraftAttachment, deleteSampleInfoDraftAttachment,
  getServerTime, requireApiSuccess, getSampleInfoWorkloadPreview, createSampleInfoWorkload, type SampleInfoAttachmentPreviewStatus,
} from '../api/client';
import TruncatedCell from '../components/TruncatedCell';
import SampleInfoRecordList from '../components/SampleInfoRecordList';
import SampleWorkloadDialog, { type SampleWorkloadPreview } from '../components/SampleWorkloadDialog';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import DateRangePicker from '../components/DateRangePicker';
import type { SampleInfoRecord, SampleInfoType, Division, ProjectGroup, Project, SampleInfoColumn, SampleInfoAttachment, SampleInfoDraft } from '../types';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';


const R = '2px';
const PAGE_SIZE = 20;
const BEIJING_OFFSET_MS = 8 * 60 * 60 * 1000;

const formatBeijingTime = (unixMs: number) =>
  new Date(unixMs + BEIJING_OFFSET_MS).toISOString().slice(0, 19).replace('T', ' ');

const STATUS_OPTIONS = ['全部', '待取样', '待检测', '已检测', '已退回', '已退回已确认', '退回待修改'] as const;
const STATUS_COLORS: Record<string, 'error' | 'warning' | 'success' | 'default'> = {
  '待取样': 'error', '待检测': 'warning', '已检测': 'success', '已退回': 'default', '已退回已确认': 'default', '退回待修改': 'default',
};
const STATUS_CHIP_SX: Record<string, object> = {
  '待取样': { bgcolor: '#d32f2f', color: '#fff' },
  '待检测': { bgcolor: '#f9a825', color: '#1f1f1f' },
  '已检测': { bgcolor: '#2e7d32', color: '#fff' },
  '已退回': { bgcolor: '#7b1fa2', color: '#fff' },
  '已退回已确认': { bgcolor: '#512da8', color: '#fff' },
  '退回待修改': { bgcolor: '#0288d1', color: '#fff' },
};

// 预置字段列表 — 在 extra_fields 中排除
const PREDEFINED_FIELDS = new Set([
  'seq_no', 'user_name', 'division_id', 'lab_name', 'project_name',
  'quantity', 'batch_no', 'main_components', 'notes', 'submitted_at',
  'detection_type', 'detection_date', 'type_key', 'status',
  'sampled_by', 'sampled_at', 'detected_by',
]);

const ACTION_FIELDS = new Set(['action_edit', 'action_record_workload', 'action_return']);

type RowData = Record<string, any>;

const emptyRow = (columns: SampleInfoColumn[], user?: any, defaultDivisionId?: number | null, defaultLabName = '', defaultProjectName = ''): RowData => {
  const row: RowData = { checked: false, _extra: {} };
  for (const col of columns) {
    if (ACTION_FIELDS.has(col.field_key)) continue;
    if (PREDEFINED_FIELDS.has(col.field_key)) {
      if (col.field_key === 'quantity') row[col.field_key] = 1;
      else if (col.field_key === 'division_id') row[col.field_key] = user?.division_id ?? defaultDivisionId ?? '';
      else if (col.field_key === 'user_name') row[col.field_key] = user?.username || '';
      else if (col.field_key === 'lab_name') row[col.field_key] = defaultLabName;
      else if (col.field_key === 'project_name') row[col.field_key] = defaultProjectName;
      else row[col.field_key] = '';
    } else {
      // 自定义字段 → 存到 _extra
      row._extra[col.field_key] = '';
    }
  }
  return row;
};

type AttachmentPreviewDialogState = SampleInfoAttachmentPreviewStatus & {
  attachmentId: number;
  fileName: string;
};

const PREVIEW_STATUS_LABEL: Record<AttachmentPreviewDialogState['status'], string> = {
  queued: '附件预览已进入后台队列',
  generating: '正在生成附件首页预览',
  first_page_ready: '首页已可查看，其他页面将在查看时生成',
  ready: '预览已生成完成',
  failed: '附件预览生成失败',
};

const AttachmentPreviewPage: React.FC<{ attachmentId: number; page: number; eager?: boolean }> = ({ attachmentId, page, eager = false }) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let disposed = false;
    let objectUrl: string | null = null;
    let observer: IntersectionObserver | null = null;
    const load = async () => {
      try {
        const blob = await getSampleInfoAttachmentPreviewPage(attachmentId, page);
        if (disposed) return;
        objectUrl = URL.createObjectURL(blob);
        setUrl(objectUrl);
      } catch {
        if (!disposed) setFailed(true);
      }
    };
    if (eager) {
      void load();
    } else if (typeof IntersectionObserver === 'undefined') {
      void load();
    } else {
      observer = new IntersectionObserver((entries) => {
        if (entries.some(entry => entry.isIntersecting)) {
          observer?.disconnect();
          observer = null;
          void load();
        }
      }, { rootMargin: '640px 0px' });
      if (containerRef.current) observer.observe(containerRef.current);
    }
    return () => {
      disposed = true;
      observer?.disconnect();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [attachmentId, page, eager]);

  return (
    <Box ref={containerRef} sx={{ minHeight: 180, bgcolor: '#fff', border: '1px solid #d9dfe7', boxShadow: 1 }}>
      {url ? (
        <Box component="img" src={url} alt={`第 ${page} 页`} sx={{ display: 'block', width: '100%', height: 'auto' }} />
      ) : (
        <Box sx={{ minHeight: 180, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 1, color: failed ? 'error.main' : 'text.secondary' }}>
          {failed ? '页面加载失败，请关闭后重试' : <><CircularProgress size={20} /><Typography variant="body2">正在加载第 {page} 页</Typography></>}
        </Box>
      )}
    </Box>
  );
};

const SampleInfoEntry: React.FC = () => {
  const { user, hasPermission } = useUser();
  const canCreate = hasPermission('sample-info:create');
  const [sp] = useSearchParams();
  const n = useNavigate();
  const dt = sp.get('type') || '';

  // 列配置
  const [columns, setColumns] = useState<SampleInfoColumn[]>([]);
  const [recordColumns, setRecordColumns] = useState<SampleInfoColumn[]>([]);
  const tableConfig = DEFAULT_TABLE_CONFIG;

  // 多行表格
  const [rows, setRows] = useState<RowData[]>([emptyRow([], user)]);
  const [defaultDivisionId, setDefaultDivisionId] = useState<number | null>(user?.division_id ?? null);
  const [labs, setLabs] = useState<ProjectGroup[]>([]);
  const [availableProjects, setAvailableProjects] = useState<Project[]>([]);
  const [defaultLabName, setDefaultLabName] = useState('');
  const [defaultProjectName, setDefaultProjectName] = useState('');
  const [organizationLoading, setOrganizationLoading] = useState(true);
  const [organizationError, setOrganizationError] = useState('');
  // v0.4.58: useRef 存储待上传文件，彻底避开 React 闭包陈旧问题
  const pendingFilesRef = useRef<Map<number, File[]>>(new Map());
  const [drafts, setDrafts] = useState<SampleInfoDraft[]>([]);
  const [draftId, setDraftId] = useState<number | null>(null);
  const [beijingTime, setBeijingTime] = useState('');
  const [snack, setSnack] = useState<{ open: boolean; msg: string; sev: 'success' | 'error' }>({ open: false, msg: '', sev: 'success' });

  // 列表
  const [records, setRecords] = useState<SampleInfoRecord[]>([]);
  const [sampleWorkloadPreview, setSampleWorkloadPreview] = useState<SampleWorkloadPreview | null>(null);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [statusFilter, setStatusFilter] = useState('全部');
  const [recordTypeFilter, setRecordTypeFilter] = useState(dt);
  const [recordStart, setRecordStart] = useState(() => dayjs().subtract(6, 'day').format('YYYY-MM-DD'));
  const [recordEnd, setRecordEnd] = useState(() => dayjs().format('YYYY-MM-DD'));
  // 排序仅保存在当前页面状态；刷新页面或换用户不会复用其他人的排序。
  const [recordSort, setRecordSort] = useState<{ field: string; direction: 'asc' | 'desc' }>({ field: 'submitted_at', direction: 'desc' });
  const [ld, setLd] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  // 编辑
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<Record<string, string>>({});
  const [returningRecord, setReturningRecord] = useState<SampleInfoRecord | null>(null);
  const [returnReason, setReturnReason] = useState('');

  // v0.4.27-A: 附件
  const [attachments, setAttachments] = useState<Record<number, SampleInfoAttachment[]>>({});
  const [attLoading, setAttLoading] = useState<Record<number, boolean>>({});
  // v0.4.30: 列表批量附件计数
  const [attachmentsByRow, setAttachmentsByRow] = useState<Record<number, SampleInfoAttachment[]>>({});
  const [imagePreview, setImagePreview] = useState<AttachmentPreviewDialogState | null>(null);
  const [downloadingAttachment, setDownloadingAttachment] = useState<number | null>(null);
  const closeImagePreview = useCallback(() => setImagePreview(null), []);
  useEffect(() => {
    if (records.length > 0) {
      const ids = records.map(r => r.id);
      batchGetSampleInfoAttachments(ids).then(r => {
        if (r.code === 0 && r.data) setAttachmentsByRow(r.data);
      }).catch(() => {});
    } else {
      setAttachmentsByRow({});
    }
  }, [records]);
  const loadAttachments = useCallback(async (recordId: number) => {
    setAttLoading(p => ({ ...p, [recordId]: true }));
    try {
      const r = await getSampleInfoAttachments(recordId);
      if (r.code === 0 && r.data) {
        setAttachments(p => ({ ...p, [recordId]: r.data! }));
      }
    } catch {} finally {
      setAttLoading(p => ({ ...p, [recordId]: false }));
    }
  }, []);
  const handleOpenAttachment = useCallback((attachment: SampleInfoAttachment) => {
    closeImagePreview();
    setImagePreview({
      attachmentId: attachment.id,
      fileName: attachment.file_name,
      status: 'queued',
      page_count: 0,
      first_page_ready: false,
      error: null,
    });
  }, [closeImagePreview]);

  const handleDownloadAttachment = useCallback(async (attachmentId: number, fileName: string) => {
    setDownloadingAttachment(attachmentId);
    try {
      const blob = await downloadSampleInfoAttachment(attachmentId);
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = fileName;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
      setSnack({ open: true, msg: '附件下载已开始', sev: 'success' });
    } catch (error: any) {
      setSnack({ open: true, msg: error?.message || '附件下载失败', sev: 'error' });
    } finally {
      setDownloadingAttachment(null);
    }
  }, []);

  useEffect(() => {
    if (!imagePreview) return;
    const attachmentId = imagePreview.attachmentId;
    let disposed = false;
    let timer: number | undefined;
    const refreshPreviewStatus = async () => {
      try {
        const result = await getSampleInfoAttachmentImagePreview(attachmentId);
        requireApiSuccess(result, '附件图片预览生成失败');
        const preview = result.data;
        if (!preview || disposed) return;
        setImagePreview(current => current?.attachmentId === attachmentId
          ? { ...current, ...preview }
          : current);
        if (preview.status !== 'ready' && preview.status !== 'failed' && preview.status !== 'first_page_ready') {
          timer = window.setTimeout(refreshPreviewStatus, 700);
        }
      } catch (error) {
        if (disposed) return;
        const responseMessage = (error as any)?.response?.data?.message;
        setImagePreview(current => current?.attachmentId === attachmentId
          ? { ...current, status: 'failed', error: responseMessage || '附件预览任务请求失败' }
          : current);
      }
    };
    void refreshPreviewStatus();
    return () => {
      disposed = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [imagePreview?.attachmentId]);
  const handleUploadAttachment = useCallback(async (recordId: number, file: File) => {
    try {
      const r = await uploadSampleInfoAttachment(recordId, file);
      if (r.code === 0) {
        setSnack({ open: true, msg: '附件上传成功', sev: 'success' });
        loadAttachments(recordId);
      } else {
        setSnack({ open: true, msg: r.message || '上传失败', sev: 'error' });
      }
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '上传失败', sev: 'error' });
    }
  }, [loadAttachments]);
  const handleDeleteAttachment = useCallback(async (attId: number, recordId: number) => {
    const reason = window.prompt('确认将该附件移入回收站？可填写删除原因。');
    if (reason === null) return;
    try {
      requireApiSuccess(await deleteSampleInfoAttachment(attId, reason), '附件删除失败');
      setSnack({ open: true, msg: '附件已移入回收站', sev: 'success' });
      loadAttachments(recordId);
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '删除失败', sev: 'error' });
    }
  }, [loadAttachments]);

  // 检测类型
  const [types, setTypes] = useState<SampleInfoType[]>([]);
  useEffect(() => {
    Promise.all([getSampleInfoTypes(), getCurrentRoleDataScopes()]).then(([typeResult, scopeResult]) => {
      if (typeResult.code !== 0 || !typeResult.data) return;
      const visible = scopeResult.code === 0 && scopeResult.data?.has_sample_info_type_scope
        ? typeResult.data.filter((item) => scopeResult.data!.sample_info_type_keys.includes(item.type_key))
        : typeResult.data;
      setTypes(visible);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    let disposed = false;
    let serverOffsetMs = 0;
    const updateClock = () => {
      if (!disposed) setBeijingTime(formatBeijingTime(Date.now() + serverOffsetMs));
    };
    const syncClock = async () => {
      try {
        const response = await getServerTime();
        if (response.code === 0 && response.data) {
          serverOffsetMs = response.data.unix_ms - Date.now();
        }
      } catch {
        // 网络短暂异常时继续使用本机时钟显示，提交时间仍以后端记录为准。
      }
      updateClock();
    };
    void syncClock();
    const clockTimer = window.setInterval(updateClock, 1000);
    const syncTimer = window.setInterval(() => { void syncClock(); }, 60_000);
    return () => {
      disposed = true;
      window.clearInterval(clockTimer);
      window.clearInterval(syncTimer);
    };
  }, []);

  // 部门列表
  const [divs, setDivs] = useState<Division[]>([]);
  useEffect(() => {
    let disposed = false;
    setOrganizationLoading(true);
    setOrganizationError('');
    const loadOrganizations = async () => {
      const [divisionResult, groupResult, projectResult] = await Promise.allSettled([
        getDivisions({ portal: 'sample_info' }),
        getGroups({ portal: 'sample_info' }),
        getProjects({ active_only: true, portal: 'sample_info' }),
      ]);
      if (disposed) return;
      const division = divisionResult.status === 'fulfilled' ? divisionResult.value : null;
      const group = groupResult.status === 'fulfilled' ? groupResult.value : null;
      const project = projectResult.status === 'fulfilled' ? projectResult.value : null;
      if (division?.code === 0 && division.data) setDivs(division.data);
      if (group?.code === 0 && group.data) setLabs(group.data);
      if (project?.code === 0 && project.data) setAvailableProjects(project.data);
      const failures = [division, group, project].filter(item => !item || item.code !== 0);
      if (failures.length > 0) setOrganizationError(failures[0]?.message || '组织数据加载失败，请刷新后重试');
      const groupDivisionId = group?.code === 0 && group.data && user?.group_id
        ? group.data.find(item => item.id === user.group_id)?.division_id
        : null;
      const nextDivisionId = user?.division_id ?? groupDivisionId ?? null;
      const nextLabName = user?.group_id && group?.data
        ? group.data.find(item => item.id === user.group_id)?.name || ''
        : '';
      const nextProjects = nextLabName && project?.code === 0 && project.data
        ? project.data.filter(item => (item.lab_ids || []).includes(user?.group_id || 0))
        : [];
      const nextProjectName = nextProjects.length === 1 ? nextProjects[0].name : '';
      setDefaultDivisionId(nextDivisionId);
      setDefaultLabName(nextLabName);
      setDefaultProjectName(nextProjectName);
      setRows(prev => prev.map(row => ({
        ...row,
        division_id: row.division_id || nextDivisionId || '',
        lab_name: row.lab_name || nextLabName,
        project_name: row.project_name || nextProjectName,
      })));
      setOrganizationLoading(false);
    };
    void loadOrganizations().catch(error => {
      if (!disposed) {
        setOrganizationError(error?.message || '组织数据加载失败，请刷新后重试');
        setOrganizationLoading(false);
      }
    });
    return () => { disposed = true; };
  }, [user?.division_id, user?.group_id]);

  useEffect(() => {
    setRecordTypeFilter(dt);
    setPage(0);
  }, [dt]);

  // 加载列配置（v0.4.27-A: 按检测类型过滤）
  useEffect(() => {
    getActiveSampleInfoColumns(dt || undefined).then(r => {
      if (r.code === 0 && r.data) {
        const cols = r.data;
        setColumns(cols);
        // v0.4.55: 保留已有行数据，按新列 field_key 映射
        setRows(prev => {
          if (prev.length === 0 || (prev.length === 1 && !prev[0].user_name && !prev[0].project_name)) {
            return [emptyRow(cols, user, defaultDivisionId, defaultLabName, defaultProjectName)];
          }
          return prev.map(r => {
            const nr: any = { checked: r.checked || false, _extra: r._extra || {} };
            cols.forEach(c => {
              if (c.is_predefined) {
                nr[c.field_key] = r[c.field_key] || '';
              } else {
                nr._extra[c.field_key] = r._extra?.[c.field_key] || '';
              }
            });
            return nr;
          });
        });
      }
    }).catch(() => {});
  }, [dt, defaultDivisionId, defaultLabName, defaultProjectName]);

  useEffect(() => {
    getActiveSampleInfoColumns(recordTypeFilter || undefined).then(response => {
      if (response.code === 0 && response.data) setRecordColumns(response.data);
    }).catch(() => setRecordColumns([]));
  }, [recordTypeFilter]);

  const load = useCallback(async () => {
    setLd(true);
    try {
      const r = await getSampleInfoRecords({
        type_key: recordTypeFilter || undefined,
        status: statusFilter === '全部' ? undefined : statusFilter,
        start: recordStart,
        end: recordEnd,
        page: page + 1,
        page_size: PAGE_SIZE,
        sort_by: recordSort.field,
        sort_dir: recordSort.direction,
      });
      if (r.data) {
        setRecords(r.data.items);
        setTotal(r.data.total);
      }
    } catch (e: any) { setSnack({ open: true, msg: e.message || '加载失败', sev: 'error' }); }
    setLd(false);
  }, [recordTypeFilter, statusFilter, recordStart, recordEnd, page, recordSort]);

  useEffect(() => { load(); }, [load]);

  const loadDrafts = useCallback(async () => {
    try { const r = await getSampleInfoDrafts(); if (r.code === 0 && r.data) setDrafts(r.data.filter(d => d.type_key === dt)); } catch {}
  }, [dt]);
  useEffect(() => { if (canCreate) loadDrafts(); }, [loadDrafts, canCreate]);

  const draftPayload = () => ({ rows, saved_at: new Date().toISOString() });
  const syncDraftAttachments = async (id: number) => {
    const existing = await getSampleInfoDraftAttachments(id);
    if (existing.code !== 0) throw new Error(existing.message || '读取草稿附件失败');
    for (const attachment of existing.data || []) {
      const removed = await deleteSampleInfoDraftAttachment(attachment.id);
      if (removed.code !== 0) throw new Error(removed.message || '清理旧草稿附件失败');
    }
    for (const [rowIndex, files] of pendingFilesRef.current.entries()) {
      for (const file of files) {
        const uploaded = await uploadSampleInfoDraftAttachment(id, rowIndex, file);
        if (uploaded.code !== 0) throw new Error(uploaded.message || `保存附件 ${file.name} 失败`);
      }
    }
  };
  const saveDraft = async () => {
    if (!dt) { setSnack({ open: true, msg: '请先选择检测类型', sev: 'error' }); return; }
    try {
      const input = { type_key: dt, title: types.find(t => t.type_key === dt)?.label || dt, payload: draftPayload() };
      const r = draftId ? await updateSampleInfoDraft(draftId, input) : await createSampleInfoDraft(input);
      if (r.code === 0 && r.data) { setDraftId(r.data.id); setSnack({ open: true, msg: '草稿已保存', sev: 'success' }); loadDrafts(); }
      else setSnack({ open: true, msg: r.message || '草稿保存失败', sev: 'error' });
      if (r.code === 0 && r.data) {
        await syncDraftAttachments(r.data.id);
        setDraftId(r.data.id);
      }
    } catch (e: any) { setSnack({ open: true, msg: e.message || '草稿保存失败', sev: 'error' }); }
  };
  const restoreDraftAttachments = async (draft: SampleInfoDraft, nextRows: RowData[]) => {
    try {
      const response = await getSampleInfoDraftAttachments(draft.id);
      if (response.code !== 0) throw new Error(response.message || '读取草稿附件失败');
      const nextFiles = new Map<number, File[]>();
      for (const attachment of response.data || []) {
        const blob = await downloadSampleInfoDraftAttachment(attachment.id);
        const file = new File([blob], attachment.file_name, { type: attachment.file_type || blob.type });
        const files = nextFiles.get(attachment.row_index) || [];
        files.push(file);
        nextFiles.set(attachment.row_index, files);
      }
      pendingFilesRef.current = nextFiles;
      setRows(nextRows.map((row, index) => ({ ...row, _pendingCount: nextFiles.get(index)?.length || 0 })));
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '恢复草稿附件失败', sev: 'error' });
    }
  };
  const restoreDraft = (draft: SampleInfoDraft) => {
    const nextRows = Array.isArray(draft.payload?.rows) ? draft.payload.rows : [];
    if (!nextRows.length) { setSnack({ open: true, msg: '草稿内容为空', sev: 'error' }); return; }
    void restoreDraftAttachments(draft, nextRows);
    setRows(nextRows); setDraftId(draft.id); setSnack({ open: true, msg: '草稿已恢复，请确认后提交', sev: 'success' });
  };

  // 表单列（show_in_form=true）
  // 操作按钮可在后台的“录入表单”列中配置排序和宽度，但不能成为样品数据输入控件。
  const formColumns = columns.filter((c: SampleInfoColumn) => c.show_in_form && c.data_type !== 'action');
  // 列表列（show_in_list=true）
  const listColumns = columns.filter((c: SampleInfoColumn) => c.show_in_list && c.data_type !== 'action');
  // 退回待修改记录沿用当前检测类型的实际表单列，避免编辑区写死字段。
  const editColumns = recordColumns
    .filter((c: SampleInfoColumn) => c.is_active && c.show_in_form && c.data_type !== 'action')
    .sort((a, b) => a.sort_order - b.sort_order);

  const updateRow = (idx: number, key: string, val: any) => {
    setRows(prev => prev.map((r, i) => {
      if (i !== idx) return r;
      // 内部属性（以 _ 开头）直接存顶层
      if (key === 'checked' || key.startsWith('_') || PREDEFINED_FIELDS.has(key)) return { ...r, [key]: val };
      return { ...r, _extra: { ...r._extra, [key]: val } };
    }));
  };

  const getRowValue = (row: RowData, fieldKey: string) => {
    if (PREDEFINED_FIELDS.has(fieldKey)) return row[fieldKey];
    return row._extra?.[fieldKey] ?? '';
  };

  const getRecordValue = (rec: SampleInfoRecord, fieldKey: string) => {
    if (fieldKey === 'division_id') return rec.division_name || rec.division_id || '';
    if (PREDEFINED_FIELDS.has(fieldKey)) return (rec as any)[fieldKey];
    return rec.extra_fields?.[fieldKey] ?? '';
  };

  const getLabsForDivision = (row: RowData) => {
    const divisionId = Number(row.division_id || defaultDivisionId || 0);
    if (!divisionId) return [];
    return labs.filter(lab => lab.division_id === divisionId);
  };

  const getProjectsForLab = (row: RowData) => {
    const labName = String(row.lab_name || '');
    const lab = labs.find(item => item.name === labName);
    if (!lab) return [];
    return availableProjects.filter(project => (project.lab_ids || []).includes(lab.id));
  };

  const updateLinkedValue = (idx: number, key: string, value: any) => {
    setRows(prev => prev.map((row, rowIndex) => {
      if (rowIndex !== idx) return row;
      if (key === 'division_id') return { ...row, division_id: value ? Number(value) : '', lab_name: '', project_name: '' };
      if (key === 'lab_name') return { ...row, lab_name: value, project_name: '' };
      return { ...row, [key]: value };
    }));
  };

  const fieldWidthBounds = (col: SampleInfoColumn) => {
    if (col.data_type === 'action') {
      const configured = Number(col.width || 100);
      return { min: Math.max(72, Math.min(configured, 220)), max: Math.max(96, Math.min(configured, 260)) };
    }
    if (col.data_type === 'attachment') return { min: 90, max: 140 };
    if (col.data_type === 'number') return { min: 62, max: 82 };
    if (col.data_type === 'date' || col.field_key.includes('date') || col.field_key.includes('time')) {
      return { min: 112, max: 150 };
    }
    if (col.field_key === 'notes' || col.field_key === 'main_components') return { min: 120, max: 220 };
    const configured = Number(col.width || 0);
    return { min: Math.max(70, Math.min(configured || 80, 110)), max: Math.max(120, Math.min(configured || 180, 220)) };
  };

  const formInputWidths = useMemo(() => getAdaptiveColumnWidths(rows, [
    { key: 'checked', header: '', fixed: tableConfig.checkbox_column_width || 44, getValue: () => '' },
    { key: 'seq', header: '序号', fixed: tableConfig.seq_column_width || 54, getValue: () => '' },
    ...formColumns.map(col => {
      const bounds = fieldWidthBounds(col);
      return {
        key: col.field_key,
        header: col.label,
        min: col.data_type === 'attachment' ? 130 : bounds.min,
        max: col.data_type === 'attachment' ? 180 : bounds.max,
        getValue: (row: RowData) => getRowValue(row, col.field_key),
      };
    }),
  ]), [rows, formColumns, tableConfig.checkbox_column_width, tableConfig.seq_column_width]);

  const listRecordWidths = useMemo(() => getAdaptiveColumnWidths(records, [
    { key: 'status', header: '状态', fixed: 76, getValue: r => r.status },
    { key: 'seq_no', header: '序号', fixed: Math.max(tableConfig.seq_column_width || 54, 74), getValue: r => `${r.seq_no} ${r.business_no || ''}` },
    ...listColumns.filter(c => !['status', 'seq_no'].includes(c.field_key)).map(col => {
      const bounds = fieldWidthBounds(col);
      return {
        key: col.field_key,
        header: col.label,
        min: col.data_type === 'attachment' ? 76 : bounds.min,
        max: col.data_type === 'attachment' ? 100 : bounds.max,
        getValue: (rec: SampleInfoRecord) => getRecordValue(rec, col.field_key),
      };
    }),
    { key: '_action', header: '其他操作', fixed: 106, getValue: () => '' },
    { key: '_expand', header: '', fixed: 34, getValue: () => '' },
  ]), [records, listColumns, tableConfig.seq_column_width]);

  // 可见桌面记录表使用独立的宽度预算：短字段保持紧凑，剩余空间优先给长文本和附件。
  const recordDisplayWidths = useMemo(() => getAdaptiveColumnWidths(records, listColumns.map(col => {
    if (col.field_key === 'status') return { key: col.field_key, header: col.label, fixed: 76, getValue: (record: SampleInfoRecord) => record.status };
    if (col.field_key === 'seq_no') return { key: col.field_key, header: col.label, fixed: Math.max(tableConfig.seq_column_width || 54, 74), getValue: (record: SampleInfoRecord) => `${record.seq_no} ${record.business_no || ''}` };

    let bounds: { min: number; max: number };
    if (col.data_type === 'attachment' || col.field_key === 'attachment_files') bounds = { min: 150, max: 240 };
    else if (col.field_key === 'quantity') bounds = { min: 52, max: 72 };
    else if (['user_name', 'division_id'].includes(col.field_key)) bounds = { min: 72, max: 104 };
    else if (col.field_key === 'lab_name') bounds = { min: 88, max: 132 };
    else if (col.field_key === 'project_name') bounds = { min: 82, max: 150 };
    else if (col.field_key === 'batch_no') bounds = { min: 100, max: 160 };
    else bounds = fieldWidthBounds(col);

    return {
      key: col.field_key,
      header: col.label,
      min: bounds.min,
      max: bounds.max,
      getValue: (record: SampleInfoRecord) => {
        if (col.data_type === 'attachment' || col.field_key === 'attachment_files') {
          return attachmentsByRow[record.id]?.map(attachment => attachment.file_name).join(' ') || '';
        }
        return getRecordValue(record, col.field_key);
      },
    };
  }).concat([{ key: '_action', header: '其他操作', fixed: 128, getValue: () => '' }])), [
    attachmentsByRow, listColumns, records, tableConfig.seq_column_width,
  ]);

  const formColumnCellSx = (key: string) => adaptiveCellSx(formInputWidths[key]);
  const listColumnCellSx = (key: string) => adaptiveCellSx(listRecordWidths[key]);

  const addRow = () => setRows(prev => [...prev, emptyRow(columns, user, defaultDivisionId, defaultLabName, defaultProjectName)]);

  const deleteSelected = () => {
    const remaining: RowData[] = [];
    const nextFiles = new Map<number, File[]>();
    rows.forEach((row, oldIndex) => {
      if (row.checked) return;
      const newIndex = remaining.length;
      remaining.push({ ...row, checked: false });
      const files = pendingFilesRef.current.get(oldIndex);
      if (files?.length) nextFiles.set(newIndex, files);
    });
    if (remaining.length === 0) remaining.push(emptyRow(columns, user, defaultDivisionId, defaultLabName, defaultProjectName));
    pendingFilesRef.current = nextFiles;
    setRows(remaining);
  };

  const resetRows = () => {
    pendingFilesRef.current = new Map();
    setRows([emptyRow(columns, user, defaultDivisionId, defaultLabName, defaultProjectName)]);
  };

  const doSubmit = async () => {
    if (!canCreate) {
      setSnack({ open: true, msg: '当前账号没有新建样品信息登记权限', sev: 'error' });
      return;
    }
    const typeLabel = types.find(t => t.type_key === dt)?.label || dt;
    let submitted = 0;
    let errors: string[] = [];
    const successfulRows: number[] = [];

    for (let i = 0; i < rows.length; i++) {
      const row = rows[i];
      const missingFields = formColumns.filter(col => {
        if (!col.is_required) return false;
        if (col.data_type === 'attachment') return !(pendingFilesRef.current.get(i)?.length);
        return String(getRowValue(row, col.field_key) ?? '').trim() === '';
      });
      if (missingFields.length > 0) {
        errors.push(`第 ${i + 1} 行缺少必填项：${missingFields.map(col => col.label).join('、')}`);
        continue;
      }
      try {
        // 分离预置字段和自定义字段
        const extra_fields: Record<string, any> = {};
        const presetData: any = {
          batch_no: row.batch_no,
          user_name: row.user_name || '未知',
          lab_name: row.lab_name || '',
          project_name: row.project_name || '',
          main_components: row.main_components,
          detection_type: typeLabel,
          type_key: dt,
          division_id: row.division_id || null,
          quantity: row.quantity || 1,
          notes: row.notes || undefined,
        };
        // 自定义字段
        if (row._extra) {
          for (const [k, v] of Object.entries(row._extra)) {
            if (v !== '' && v !== null && v !== undefined) {
              extra_fields[k] = v;
            }
          }
        }
        if (Object.keys(extra_fields).length > 0) {
          presetData.extra_fields = extra_fields;
        }
        // The record and its selected attachments are submitted as one request so notification
        // delivery always sees the final attachment count.
        const pendingFiles = pendingFilesRef.current.get(i) || [];
        const response = await createSampleInfoWithAttachments(presetData, pendingFiles);
        if (response.code !== 0 || !response.data?.id) {
          throw new Error(response.message || '提交失败，服务器未返回有效记录');
        }
        submitted++;
        successfulRows.push(i);
      } catch (e: any) {
        errors.push(`第 ${i + 1} 行: ${e.message || '提交失败'}`);
      }
    }

    if (submitted > 0) {
      setSnack({ open: true, msg: `成功登记 ${submitted} 条` + (errors.length > 0 ? `，${errors.length} 条失败` : ''), sev: 'success' });
      const successfulSet = new Set(successfulRows);
      const remainingRows: RowData[] = [];
      const remainingFiles = new Map<number, File[]>();
      rows.forEach((row, oldIndex) => {
        if (successfulSet.has(oldIndex)) return;
        const newIndex = remainingRows.length;
        remainingRows.push({ ...row, checked: false });
        const files = pendingFilesRef.current.get(oldIndex);
        if (files?.length) remainingFiles.set(newIndex, files);
      });
      if (remainingRows.length === 0) {
        remainingRows.push(emptyRow(columns, user, defaultDivisionId, defaultLabName, defaultProjectName));
      }
      pendingFilesRef.current = remainingFiles;
      setRows(remainingRows);
      setPage(0); load();
      if (draftId && errors.length === 0) {
        try {
          await deleteSampleInfoDraft(draftId);
          setDraftId(null);
          loadDrafts();
        } catch {
          // The submitted sample is valid even if a stale local draft could not be removed.
        }
      }
    } else if (errors.length > 0) {
      setSnack({ open: true, msg: errors.join('；'), sev: 'error' });
    } else {
      setSnack({ open: true, msg: '请填写数据', sev: 'error' });
    }
  };

  const doExpand = (id: number) => {
    const next = expandedId === id ? null : id;
    setExpandedId(next);
    if (next && !attachments[next]) {
      loadAttachments(next);
    }
  };

  const doEdit = (rec: SampleInfoRecord) => {
    loadAttachments(rec.id);
    setEditingId(rec.id);
    const initial: Record<string, string> = {};
    for (const column of editColumns) {
      const rawValue = column.field_key === 'division_id'
        ? (rec.division_id ?? '')
        : getRecordValue(rec, column.field_key);
      let value = rawValue === null || rawValue === undefined ? '' : String(rawValue);
      if (column.data_type === 'date' && value) value = value.slice(0, 10);
      initial[column.field_key] = value;
    }
    setEditForm(initial);
  };

  const doCancelEdit = () => { setEditingId(null); setEditForm({}); };

  const doReturn = (record: SampleInfoRecord) => {
    setReturningRecord(record);
    setReturnReason('');
  };

  const submitReturn = async () => {
    const reason = returnReason.trim();
    if (!returningRecord || !reason) {
      setSnack({ open: true, msg: '请填写退回原因', sev: 'error' });
      return;
    }
    try {
      requireApiSuccess(await returnSampleInfo(returningRecord.id, reason), '退回失败');
      setReturningRecord(null);
      setReturnReason('');
      setSnack({ open: true, msg: '样品登记已退回', sev: 'success' });
      await load();
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '退回失败', sev: 'error' });
    }
  };

  const doConfirmReturn = async (record: SampleInfoRecord) => {
    try {
      requireApiSuccess(await confirmSampleInfoReturn(record.id), '确认退回失败');
      setSnack({ open: true, msg: '已确认退回，已生成待修改草稿', sev: 'success' });
      await load();
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '确认退回失败', sev: 'error' });
    }
  };

  const doSaveEdit = async (id: number) => {
    try {
      const record = records.find(item => item.id === id);
      const payload: Record<string, any> = {};
      const extra_fields: Record<string, any> = { ...(record?.extra_fields || {}) };
      for (const column of editColumns) {
        if (column.data_type === 'attachment' || column.field_key === 'attachment_files') continue;
        if (['status', 'seq_no', 'business_no', 'submitted_at', 'type_key', 'detection_type', 'sampled_by', 'sampled_at', 'detected_by'].includes(column.field_key)) continue;
        if (record?.status === '退回待修改' && ['division_id', 'lab_name', 'project_name'].includes(column.field_key)) continue;
        const rawValue = editForm[column.field_key] ?? '';
        const value = column.data_type === 'number'
          ? (String(rawValue).trim() === '' ? null : Number(rawValue))
          : rawValue;
        if (column.field_key === 'division_id') {
          payload.division_id = String(rawValue).trim() === '' ? null : Number(rawValue);
        } else if (PREDEFINED_FIELDS.has(column.field_key)) payload[column.field_key] = value;
        else extra_fields[column.field_key] = value;
      }
      payload.extra_fields = extra_fields;
      requireApiSuccess(await updateSampleInfo(id, payload), '保存失败');
      setSnack({ open: true, msg: '保存成功', sev: 'success' });
      setEditingId(null); setEditForm({}); await load();
    } catch (e: any) { setSnack({ open: true, msg: e.message || '保存失败', sev: 'error' }); }
  };

  const doStatusFlow = async (id: number, curStatus: string) => {
    const scrollTop = window.scrollY;
    try {
      const nxt = curStatus === '待取样' ? '待检测' : '已检测';
      if (curStatus === '待取样') requireApiSuccess(await sampleSampleInfo(id), '取样失败');
      else if (curStatus === '待检测') requireApiSuccess(await completeSampleInfo(id), '完成检测失败');
      else return;
      setSnack({ open: true, msg: `状态已更新：${curStatus} → ${nxt}`, sev: 'success' });
      // 状态操作只更新记录状态，保留当前页、排序和滚动位置。
      await load();
      window.requestAnimationFrame(() => window.scrollTo({ top: scrollTop, behavior: 'auto' }));
    } catch (e: any) { setSnack({ open: true, msg: e.message || '状态流转失败', sev: 'error' }); }
  };

  const doWithdrawSample = async (id: number) => {
    const scrollTop = window.scrollY;
    try {
      requireApiSuccess(await withdrawSampleInfo(id), '撤回取样失败');
      setSnack({ open: true, msg: '已撤回取样，记录恢复为待取样', sev: 'success' });
      await load();
      window.requestAnimationFrame(() => window.scrollTo({ top: scrollTop, behavior: 'auto' }));
    } catch (e: any) { setSnack({ open: true, msg: e.message || '撤回取样失败', sev: 'error' }); }
  };

  const doRecordSampleWorkload = async (id: number) => {
    try {
      const result = requireApiSuccess(await getSampleInfoWorkloadPreview(id), '获取工作量信息失败');
      setSampleWorkloadPreview(result);
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '获取工作量信息失败', sev: 'error' });
    }
  };

  const confirmSampleWorkload = async (data: { quantity: number; multiplier: number; method_id?: number; notes?: string }) => {
    if (!sampleWorkloadPreview || !data.method_id) return;
    try {
      requireApiSuccess(await createSampleInfoWorkload(sampleWorkloadPreview.source_record_id, {
        method_id: data.method_id, quantity: data.quantity, multiplier: data.multiplier, notes: data.notes,
      }), '录入工作量失败');
      setSampleWorkloadPreview(null);
      setSnack({ open: true, msg: '工作量录入成功', sev: 'success' });
      await load();
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '录入工作量失败', sev: 'error' });
      throw e;
    }
  };

  const changeRecordSort = (field: string) => {
    setRecordSort(current => current.field === field
      ? { field, direction: current.direction === 'asc' ? 'desc' : 'asc' }
      : { field, direction: 'asc' });
  };

  const fmtDate = (s: string) => s ? s.slice(0, 16).replace('T', ' ') : '';

  /** 根据 data_type 渲染对应的输入控件 */
  const renderCellInput = (col: SampleInfoColumn, idx: number) => {
    const val = getRowValue(rows[idx], col.field_key);
    if (col.field_key === 'division_id' || col.field_key === 'lab_name' || col.field_key === 'project_name') {
      const options = col.field_key === 'division_id'
        ? divs.map(item => ({ value: item.id, label: item.name }))
        : col.field_key === 'lab_name'
          ? getLabsForDivision(rows[idx]).map(item => ({ value: item.name, label: item.name }))
          : getProjectsForLab(rows[idx]).map(item => ({ value: item.name, label: item.name }));
      const disabled = (col.field_key === 'lab_name' && !rows[idx].division_id)
        || (col.field_key === 'project_name' && !rows[idx].lab_name);
      return <FormControl fullWidth size="small">
        <Select value={val ?? ''} displayEmpty disabled={disabled} onChange={event => updateLinkedValue(idx, col.field_key, event.target.value)} sx={{ fontSize: '0.8rem' }}>
          <MenuItem value=""><em>请选择</em></MenuItem>
          {options.map(option => <MenuItem key={String(option.value)} value={option.value}>{option.label}</MenuItem>)}
        </Select>
      </FormControl>;
    }
    switch (col.data_type) {
      case 'attachment':
        const files = pendingFilesRef.current.get(idx) || [];
        return (
          <Box>
            <Button component="label" size="small" startIcon={<AttachFileIcon />}
              sx={{ fontSize: '0.7rem', borderRadius: R, color: '#2e7d32', textTransform: 'none' }}>
              选择文件
              <input type="file" hidden multiple accept=".pdf,.doc,.docx"
                onChange={e => {
                  const selected = Array.from(e.target.files || []);
                  const existing = pendingFilesRef.current.get(idx) || [];
                  pendingFilesRef.current.set(idx, [...existing, ...selected]);
                  // 强制重渲染显示文件列表
                  setRows(prev => prev.map((r, i) => i === idx ? { ...r, _pendingCount: (r._pendingCount || 0) + selected.length } : r));
                  e.target.value = '';
                }} />
            </Button>
            {files.length > 0 && (
              <Box sx={{ mt: 0.5 }}>
                {files.map((f: File, fi: number) => (
                  <Box key={fi} sx={{ display: 'flex', alignItems: 'center', gap: 0.5, fontSize: '0.7rem', color: '#666' }}>
                    <AttachFileIcon sx={{ fontSize: 14 }} />
                    <Box sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 180 }}>
                      {f.name}
                    </Box>
                    <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: 'nowrap' }}>
                      ({(f.size / 1024).toFixed(0)}KB)
                    </Typography>
                    <IconButton size="small" sx={{ p: 0.3 }} onClick={() => {
                      // v0.4.60: 直接修改 pendingFilesRef.current（之前错写到 React state）
                      const arr = [...(pendingFilesRef.current.get(idx) || [])];
                      arr.splice(fi, 1);
                      pendingFilesRef.current.set(idx, arr);
                      // 触发重渲染（不影响数据本身）
                      setRows(prev => prev.map((r, i) => i === idx ? { ...r, _pendingCount: arr.length } : r));
                    }}><CloseIcon fontSize="small" /></IconButton>
                  </Box>
                ))}
              </Box>
            )}
          </Box>
        );
      case 'select':
        return (
          <FormControl fullWidth size="small">
            <Select
              value={val}
              displayEmpty
              onChange={e => updateRow(idx, col.field_key, e.target.value)}
              sx={{ fontSize: '0.8rem' }}
            >
              <MenuItem value=""><em>请选择</em></MenuItem>
              {!['division_id', 'lab_name', 'project_name'].includes(col.field_key) && col.options?.split(',').map(opt => (
                <MenuItem key={opt} value={opt}>{opt}</MenuItem>
              ))}
            </Select>
          </FormControl>
        );

      case 'number':
        return (
          <TextField
            size="small" type="number"
            value={val}
            onChange={e => updateRow(idx, col.field_key, Math.max(1, Number(e.target.value) || 1))}
            inputProps={{ min: 1 }}
            sx={{ width: '100%', '& .MuiOutlinedInput-root': { fontSize: '0.8rem' } }}
          />
        );
      case 'date':
        return (
          <TextField
            size="small" type="date"
            value={val}
            onChange={e => updateRow(idx, col.field_key, e.target.value)}
            InputLabelProps={{ shrink: true }}
            sx={{ width: '100%', '& .MuiOutlinedInput-root': { fontSize: '0.8rem' } }}
          />
        );
      default:
        return (
          <TextField
            size="small" fullWidth
            value={val}
            onChange={e => updateRow(idx, col.field_key, e.target.value)}
            placeholder={col.label}
            required={col.is_required}
            sx={{ '& .MuiOutlinedInput-root': { fontSize: '0.8rem' } }}
          />
        );
    }
  };

  /** 根据 data_type 渲染只读显示值 */
  const renderCellValue = (col: SampleInfoColumn, rec: SampleInfoRecord) => {
    if (col.data_type === 'attachment') {
      return <Chip icon={<AttachFileIcon />} label={attachmentsByRow?.[rec.id]?.length || 0} size="small" />;
    }
    let val: any;
    if (PREDEFINED_FIELDS.has(col.field_key)) {
      val = (rec as any)[col.field_key];
    } else if (rec.extra_fields) {
      val = rec.extra_fields[col.field_key];
    }
    if (val === null || val === undefined || val === '') return '-';
    if (col.data_type === 'date') return String(val).slice(0, 10);
    return String(val);
  };

  return (
    
    <Box sx={{ width: '100%', maxWidth: 1536, mx: 'auto', mt: { xs: 1, md: 2 }, px: { xs: 0, sm: 1 } }}>
      {/* 顶部 */}
      
      <Box sx={{ display: 'flex', alignItems: 'center', mb: 2, gap: 1 }}>
        <IconButton onClick={() => n('/sample-info')} size="small"><ArrowBackIcon /></IconButton>
        <Typography variant="h5" fontWeight={700} color="#2e7d32">样品信息登记</Typography>
      </Box>
      

      {/* === 部分 A：登记表单 === */}
      
      {canCreate && <Paper elevation={0} sx={{ p: { xs: 1.25, md: 2 }, mb: 2, borderRadius: '6px', border: '1px solid #d9e2dc', borderTop: '3px solid #2e7d32', bgcolor: '#fff' }}>
        {organizationError && <Alert severity="error" sx={{ mb: 2 }}>{organizationError}</Alert>}
        {!organizationLoading && !organizationError && divs.length === 0 && <Alert severity="warning" sx={{ mb: 2 }}>当前账号无可用部门，请联系管理员配置样品信息登记权限。</Alert>}
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2, flexWrap: 'wrap', gap: 1 }}>
          <Box>
            <Typography variant="subtitle1" fontWeight={700} color="#2e7d32">登记信息</Typography>
            <Typography variant="body2" color="text.secondary">
              
                <Box component="span" sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.5 }}>
                  <Chip label={types.find(t => t.type_key === dt)?.label || dt || '全部'} size="small" color="success" variant="outlined" />
                </Box>
              
              ·
              
                <Box component="span" sx={{ color: '#999' }}>序号: 自动生成</Box>
              
            </Typography>
          </Box>
          <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap' }}>
            <Button size="small" variant="outlined" onClick={() => void saveDraft()}>保存草稿</Button>
            {drafts.map(draft => <Button key={draft.id} size="small" variant="text" onClick={() => restoreDraft(draft)}>恢复草稿 {draft.id}</Button>)}
            <Chip label="待取样" color="error" size="small" sx={STATUS_CHIP_SX['待取样']} />
          </Box>
        </Box>

        {/* 公共时间 */}
        
        <Box sx={{ mb: 1.5 }}>
          <TextField
            label="送样时间（整单公共）"
            type="text"
            required
            size="small"
            value={beijingTime || '正在同步服务器北京时间...'}
            InputProps={{ readOnly: true }}
            InputLabelProps={{ shrink: true }}
            helperText="提交时由服务器再次校准并写入，不可修改"
            sx={{ width: { xs: '100%', sm: 280 } }}
          />
        </Box>
        

        {/* 动态多行录入：宽屏五列，默认字段自然分为两行 */}
        <Box sx={{ display: 'grid', gap: 1.25, mb: 2 }}>
          {rows.map((row, idx) => (
            <Paper key={idx} variant="outlined" sx={{ p: { xs: 1, md: 1.5 }, borderRadius: '6px', bgcolor: row.checked ? '#f1f8f2' : '#fff' }}>
              <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
                  <Checkbox size="small" checked={Boolean(row.checked)} onChange={(_, checked) => updateRow(idx, 'checked', checked)} />
                  <Typography variant="subtitle2" fontWeight={700}>第 {idx + 1} 条样品</Typography>
                </Box>
                <Typography variant="caption" color="text.secondary">必填字段标有 *</Typography>
              </Box>
              <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2,minmax(0,1fr))', sm: 'repeat(2,minmax(0,1fr))', lg: 'repeat(4,minmax(0,1fr))', xl: 'repeat(5,minmax(0,1fr))' }, gap: { xs: 1, sm: 1.25 }, alignItems: 'start' }}>
                {formColumns.map(col => (
                  <Box key={col.field_key} sx={{
                    minWidth: 0,
                    gridColumn: {
                      xs: ['main_components', 'notes'].includes(col.field_key) || col.data_type === 'attachment' ? '1 / -1' : 'auto',
                      sm: 'auto',
                    },
                  }}>
                    <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.35, fontWeight: 600 }}>
                      {col.label}{col.is_required ? ' *' : ''}
                    </Typography>
                    {renderCellInput(col, idx)}
                  </Box>
                ))}
              </Box>
            </Paper>
          ))}
        </Box>

        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '1fr auto' }, gap: 1 }}>
          
          <Box sx={{ display: 'flex', gap: { xs: 0.75, sm: 1 }, flexWrap: 'nowrap', width: { xs: '100%', sm: 'auto' }, '& .MuiButton-root': { flex: { xs: 1, sm: 'initial' }, minWidth: 0, px: { xs: 1, sm: 1.5 } } }}>
            <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={addRow} sx={{ borderRadius: R }}>
              添加行
            </Button>
            <Button variant="outlined" size="small" color="error" startIcon={<DeleteIcon />} onClick={deleteSelected} disabled={!rows.some(r => r.checked)} sx={{ borderRadius: R }}>
              删除选中行
            </Button>
            <Button variant="outlined" size="small" onClick={resetRows} sx={{ borderRadius: R }}>
              重置
            </Button>
          </Box>
          
          
          <Button variant="contained" onClick={doSubmit} sx={{ width: { xs: '100%', sm: 'auto' }, borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>
            提交登记（{rows.length} 行）
          </Button>
          
        </Box>
      </Paper>}
      

      {/* === 部分 B：记录列表 === */}
      
      <SampleInfoRecordList
        records={records}
        total={total}
        page={page}
        pageSize={PAGE_SIZE}
        loading={ld}
        statusFilter={statusFilter}
        statusOptions={STATUS_OPTIONS}
        typeFilter={recordTypeFilter}
        typeOptions={types.map(type => ({ value: type.type_key, label: type.label }))}
        startDate={recordStart}
        endDate={recordEnd}
        columns={recordColumns.filter(column => column.show_in_list)}
        editColumns={editColumns}
        columnWidths={recordDisplayWidths}
        attachmentsByRow={attachmentsByRow}
        attachments={attachments}
        attachmentLoading={attLoading}
        editingId={editingId}
        editForm={editForm}
        hasPermission={hasPermission}
        divisionOptions={divs.map(item => ({ value: item.id, label: item.name }))}
        onPageChange={setPage}
        onStatusChange={status => { setStatusFilter(status); setPage(0); }}
        onTypeChange={typeKey => { setRecordTypeFilter(typeKey); setPage(0); }}
        onStartChange={date => { setRecordStart(date); setPage(0); }}
        onEndChange={date => { setRecordEnd(date); setPage(0); }}
        onResetFilters={() => {
          setRecordStart(dayjs().subtract(6, 'day').format('YYYY-MM-DD'));
          setRecordEnd(dayjs().format('YYYY-MM-DD'));
          setRecordTypeFilter(dt);
          setStatusFilter('全部');
          setPage(0);
        }}
        onEdit={doEdit}
        onCancelEdit={doCancelEdit}
        onSaveEdit={doSaveEdit}
        onEditFormChange={(field, value) => setEditForm(prev => ({ ...prev, [field]: value }))}
        onStatusFlow={doStatusFlow}
        onRecordWorkload={doRecordSampleWorkload}
        onWithdrawSample={doWithdrawSample}
        onReturn={doReturn}
        onConfirmReturn={doConfirmReturn}
        currentUserId={user?.id}
        isSystemAdmin={Boolean(user?.is_admin)}
        sortBy={recordSort.field}
        sortDir={recordSort.direction}
        onSortChange={changeRecordSort}
        onLoadAttachments={loadAttachments}
        onUploadAttachment={handleUploadAttachment}
        onDeleteAttachment={handleDeleteAttachment}
        onOpenAttachment={attachment => handleOpenAttachment(attachment)}
        getRecordValue={getRecordValue}
        formatDate={fmtDate}
      />
      <SampleWorkloadDialog preview={sampleWorkloadPreview} onClose={() => setSampleWorkloadPreview(null)} onConfirm={confirmSampleWorkload} />

      <Dialog open={Boolean(returningRecord)} onClose={() => setReturningRecord(null)} fullWidth maxWidth="sm">
        <DialogTitle>退回样品登记</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
            {returningRecord ? `记录 ${returningRecord.business_no || `#${returningRecord.seq_no}`} 将退回给提交账号处理。` : ''}
          </Typography>
          <TextField
            autoFocus
            fullWidth
            multiline
            minRows={3}
            label="退回原因"
            value={returnReason}
            onChange={event => setReturnReason(event.target.value)}
            placeholder="请说明需要补充或修改的内容"
          />
          <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, mt: 2 }}>
            <Button variant="outlined" onClick={() => setReturningRecord(null)}>取消</Button>
            <Button variant="contained" color="secondary" onClick={() => void submitReturn()}>确认退回</Button>
          </Box>
        </DialogContent>
      </Dialog>

      <Paper elevation={0} sx={{ display: 'none', p: { xs: 1, md: 2 }, borderRadius: R, border: '1px solid #e0e0e0' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2, flexWrap: 'wrap', gap: 1 }}>
          <Typography variant="h6" fontWeight={700}>登记记录</Typography>
          <FormControl size="small" sx={{ minWidth: 120 }}>
            <InputLabel>状态</InputLabel>
            <Select value={statusFilter} label="状态" onChange={e => { setStatusFilter(e.target.value); setPage(0); }}>
              {STATUS_OPTIONS.map(s => <MenuItem key={s} value={s}>{s}</MenuItem>)}
            </Select>
          </FormControl>
        </Box>

        {ld ? <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={32} /></Box> : (
          <>
            {/* 动态列表头 — v0.4.34: 改为 MUI Table 布局 */}
            <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, mb: 1, overflowX: 'auto' }}>
              <Table size="small" sx={adaptiveTableSx}>
                <TableHead>
                  <TableRow sx={{ bgcolor: '#f5f5f5' }}>
                    <TableCell sx={{ ...listColumnCellSx('status'), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>状态</TableCell>
                    <TableCell sx={{ ...listColumnCellSx('seq_no'), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>序号</TableCell>
                    {listColumns.filter(c => !['status', 'seq_no'].includes(c.field_key)).map(col => (
                      <TableCell key={col.field_key} sx={{ ...listColumnCellSx(col.field_key), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>
                        {col.label}
                      </TableCell>
                    ))}
                    <TableCell sx={{ ...listColumnCellSx('_action'), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>操作</TableCell>
                    <TableCell sx={{ ...listColumnCellSx('_expand'), borderColor: '#e0e0e0' }} />
                  </TableRow>
                </TableHead>
                <TableBody>
                  {records.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={listColumns.length + 4} align="center" sx={{ py: 4, color: '#999', fontSize: '0.875rem' }}>暂无登记记录</TableCell>
                    </TableRow>
                  ) : records.map(r => {
                    const isDone = r.status === '已检测';
                    const canModifyAttachments = r.status !== '已退回' && r.status !== '已退回已确认' && (
                      Boolean(user?.is_admin)
                      || (r.sampled_at == null && (r.created_by_user_id === user?.id || r.business_user_id === user?.id) && hasPermission('sample-info:edit-own'))
                      || hasPermission('sample-info:collect')
                      || hasPermission('sample-info:complete')
                    );
                    return (
                      <React.Fragment key={r.id}>
                        <TableRow
                          hover
                          onClick={() => doExpand(r.id)}
                          sx={{ cursor: 'pointer', opacity: isDone ? 0.5 : 1, '&:hover': { bgcolor: '#fafafa' } }}
                        >
                          <TableCell sx={{ ...listColumnCellSx('status'), fontSize: '0.875rem', borderColor: '#e0e0e0' }}>
                            <Chip label={r.status} color={STATUS_COLORS[r.status] || 'default'} size="small" sx={STATUS_CHIP_SX[r.status]} />
                          </TableCell>
                          <TableCell sx={{ ...listColumnCellSx('seq_no'), fontSize: '0.875rem', color: '#999', borderColor: '#e0e0e0' }}>#{r.seq_no}{r.business_no && <Box sx={{ mt: 0.25, fontFamily: 'monospace', fontSize: '0.62rem', lineHeight: 1.2, overflowWrap: 'anywhere' }}>{r.business_no}</Box>}</TableCell>
                          {listColumns.filter(c => !['status', 'seq_no'].includes(c.field_key)).map(col => (
                            <TableCell key={col.field_key} sx={{ ...listColumnCellSx(col.field_key), fontSize: '0.875rem', borderColor: '#e0e0e0' }}>
                              {renderCellValue(col, r)}
                            </TableCell>
                          ))}
                          <TableCell sx={{ ...listColumnCellSx('_action'), borderColor: '#e0e0e0' }}>
                            {r.status === '待取样' && hasPermission('sample-info:collect') && (
                              <Button size="small" variant="contained" color="error" startIcon={<ScienceIcon />}
                                onClick={e => { e.stopPropagation(); doStatusFlow(r.id, r.status); }} sx={{ borderRadius: R, whiteSpace: 'nowrap' }}>
                                取样
                              </Button>
                            )}
                            {r.status === '待检测' && hasPermission('sample-info:complete') && (
                              <Button size="small" variant="contained" color="success" startIcon={<CheckCircleIcon />}
                                onClick={e => { e.stopPropagation(); doStatusFlow(r.id, r.status); }} sx={{ borderRadius: R, whiteSpace: 'nowrap' }}>
                                完成检测
                              </Button>
                            )}
                            {isDone && '-'}
                          </TableCell>
                          <TableCell sx={{ ...listColumnCellSx('_expand'), borderColor: '#e0e0e0' }}>{expandedId === r.id ? <ExpandLessIcon fontSize="small" /> : <ExpandMoreIcon fontSize="small" />}</TableCell>
                        </TableRow>
                        {expandedId === r.id && (
                          <TableRow>
                            <TableCell colSpan={listColumns.length + 4} sx={{ p: 0, border: 'none' }}>
                              <Collapse in={expandedId === r.id}>
                                <Paper elevation={0} sx={{ mx: 2, mb: 1, p: 2, border: '1px solid #e8e8e8', borderRadius: R, bgcolor: '#fafafa' }}>
                                  <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
                                    <Typography variant="subtitle2" fontWeight={700} color="#2e7d32">登记详情 · 序号 #{r.seq_no}</Typography>
                                    <Chip label={r.status} color={STATUS_COLORS[r.status] || 'default'} size="small" sx={STATUS_CHIP_SX[r.status]} />
                                  </Box>
                                  {editingId === r.id ? (
                                    <Grid container spacing={2}>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="样品批号" fullWidth size="small" value={editForm.batch_no || ''} onChange={e => setEditForm(p => ({ ...p, batch_no: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="送样人" fullWidth size="small" value={editForm.user_name || ''} onChange={e => setEditForm(p => ({ ...p, user_name: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="实验室/车间" fullWidth size="small" value={editForm.lab_name || ''} onChange={e => setEditForm(p => ({ ...p, lab_name: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="所属项目" fullWidth size="small" value={editForm.project_name || ''} onChange={e => setEditForm(p => ({ ...p, project_name: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <Box><Typography variant="caption" color="text.secondary">送样时间</Typography><Typography variant="body2">{fmtDate(r.submitted_at)}</Typography></Box>
                                      </Grid>
                                      <Grid item xs={12}>
                                        <TextField label="样品主要成分" fullWidth size="small" value={editForm.main_components || ''} onChange={e => setEditForm(p => ({ ...p, main_components: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12}>
                                        <TextField label="注意事项" fullWidth size="small" multiline rows={2} value={editForm.notes || ''} onChange={e => setEditForm(p => ({ ...p, notes: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12}>
                                        <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end' }}>
                                          <Button variant="outlined" size="small" onClick={doCancelEdit} sx={{ borderRadius: R }}>取消</Button>
                                          <Button variant="contained" size="small" onClick={() => doSaveEdit(r.id)} sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>保存</Button>
                                        </Box>
                                      </Grid>
                                    </Grid>
                                  ) : (
                                    <>
                                      <Grid container spacing={2} sx={{ mb: 2 }}>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">样品批号</Typography><Typography variant="body2">{r.batch_no}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">送样人</Typography><Typography variant="body2">{r.user_name}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">实验室/车间</Typography><Typography variant="body2">{r.lab_name}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">所属项目</Typography><Typography variant="body2">{r.project_name}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">送样时间</Typography><Typography variant="body2">{fmtDate(r.submitted_at)}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">检测时间</Typography><Typography variant="body2">{fmtDate(r.detection_date)}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">取样人 / 取样时间</Typography><Typography variant="body2">{r.sampled_by ? `${r.sampled_by} / ${fmtDate(r.sampled_at || '')}` : '-'}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">检测完成人</Typography><Typography variant="body2">{r.detected_by || '-'}</Typography></Grid>
                                        <Grid item xs={12}><Typography variant="caption" color="text.secondary">样品主要成分</Typography><Typography variant="body2">{r.main_components}</Typography></Grid>
                                        {r.notes && <Grid item xs={12}><Typography variant="caption" color="text.secondary">注意事项</Typography><Typography variant="body2">{r.notes}</Typography></Grid>}
                                      </Grid>
                                      <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end' }}>
                                        {!isDone && <Button variant="outlined" size="small" onClick={() => doEdit(r)} sx={{ borderRadius: R }}>编辑</Button>}
                                      </Box>
                                      <Box sx={{ mt: 2, pt: 2, borderTop: '1px solid #e0e0e0' }}>
                                        <Typography variant="caption" fontWeight={600} color="text.secondary" sx={{ mb: 1, display: 'flex', alignItems: 'center', gap: 0.5 }}>
                                          <AttachFileIcon fontSize="inherit" /> 附件
                                        </Typography>
                                        <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center', mt: 0.5 }}>
                                          {canModifyAttachments && <Button component="label" variant="outlined" size="small" startIcon={<AddIcon />} sx={{ borderRadius: R, fontSize: '0.75rem' }}>
                                              上传附件
                                              <input type="file" hidden accept=".pdf,.doc,.docx" onChange={(e) => {
                                                const file = (e.target as HTMLInputElement).files?.[0];
                                                if (file) {
                                                  if (file.size > 100 * 1024 * 1024) {
                                                    setSnack({ open: true, msg: '文件大小不能超过100MB', sev: 'error' });
                                                    return;
                                                  }
                                                  handleUploadAttachment(r.id, file);
                                                }
                                                (e.target as HTMLInputElement).value = '';
                                              }} />
                                            </Button>}
                                          {attLoading[r.id] && <CircularProgress size={16} />}
                                          {(attachments[r.id] || []).map(att => (
                                            <Chip key={att.id} icon={<DescriptionIcon />} label={att.file_name} size="small"
                                              onClick={() => handleOpenAttachment(att)}
                                              onDelete={canModifyAttachments ? () => handleDeleteAttachment(att.id, r.id) : undefined}
                                              sx={{ borderRadius: R, cursor: 'pointer', fontSize: '0.75rem' }} />
                                          ))}
                                        </Box>
                                      </Box>
                                    </>
                                  )}
                                </Paper>
                              </Collapse>
                            </TableCell>
                          </TableRow>
                        )}
                      </React.Fragment>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
            <TablePagination
              component="div" count={total} page={page} onPageChange={(_, p) => setPage(p)}
              rowsPerPage={PAGE_SIZE} rowsPerPageOptions={[PAGE_SIZE]}
              onRowsPerPageChange={e => { setPage(0); }}
              labelRowsPerPage="每页"
            />
          </>
        )}
      </Paper>
      

      <Dialog fullScreen open={Boolean(imagePreview)} onClose={closeImagePreview}>
        <DialogTitle sx={{ pr: 6, overflowWrap: 'anywhere' }}>附件预览：{imagePreview?.fileName}</DialogTitle>
        <IconButton aria-label="关闭预览" onClick={closeImagePreview} sx={{ position: 'absolute', right: 10, top: 10 }}><CloseIcon /></IconButton>
        <DialogContent dividers sx={{ bgcolor: '#f3f5f7', p: { xs: 1, sm: 2 } }}>
          <Box sx={{ width: '100%', maxWidth: 1180, mx: 'auto', display: 'grid', gap: { xs: 1, sm: 2 } }}>
            {imagePreview && (
              <>
                <Paper variant="outlined" sx={{ px: 1.5, py: 1, display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, flexWrap: 'wrap', borderRadius: R }}>
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, minWidth: 0, flex: '1 1 240px' }}>
                    {imagePreview.status !== 'ready' && imagePreview.status !== 'failed' && imagePreview.status !== 'first_page_ready' && <CircularProgress size={18} />}
                    <Typography variant="body2" color={imagePreview.status === 'failed' ? 'error.main' : 'text.secondary'} sx={{ overflowWrap: 'anywhere' }}>
                      {imagePreview.error || PREVIEW_STATUS_LABEL[imagePreview.status]}
                    </Typography>
                  </Box>
                  <Button
                    size="small"
                    variant="outlined"
                    startIcon={downloadingAttachment === imagePreview.attachmentId ? <CircularProgress size={16} /> : <DownloadIcon />}
                    onClick={() => void handleDownloadAttachment(imagePreview.attachmentId, imagePreview.fileName)}
                    disabled={downloadingAttachment === imagePreview.attachmentId}
                    sx={{ borderRadius: R, flexShrink: 0 }}
                  >下载</Button>
                </Paper>
                {imagePreview.first_page_ready && (
                  <AttachmentPreviewPage attachmentId={imagePreview.attachmentId} page={1} eager />
                )}
                {(imagePreview.status === 'ready' || imagePreview.status === 'first_page_ready') && Array.from(
                  { length: Math.max(0, imagePreview.page_count - 1) },
                  (_, index) => <AttachmentPreviewPage key={index + 2} attachmentId={imagePreview.attachmentId} page={index + 2} />,
                )}
              </>
            )}
          </Box>
        </DialogContent>
      </Dialog>

      <Snackbar open={snack.open} autoHideDuration={3000} onClose={() => setSnack(s => ({ ...s, open: false }))} anchorOrigin={{ vertical: 'top', horizontal: 'center' }}>
        <Alert severity={snack.sev} sx={{ width: '100%' }} onClose={() => setSnack(s => ({ ...s, open: false }))}>{snack.msg}</Alert>
      </Snackbar>
    </Box>
    
  );
};
export default SampleInfoEntry;
