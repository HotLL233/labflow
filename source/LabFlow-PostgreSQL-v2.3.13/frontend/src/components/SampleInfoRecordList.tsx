import React from 'react';
import {
  Box, Button, Card, CardContent, Chip, CircularProgress, IconButton,
  FormControl, InputLabel, MenuItem, Paper, Select, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination,
  TableRow, TableSortLabel, TextField, Tooltip, Typography,
} from '@mui/material';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import DescriptionIcon from '@mui/icons-material/Description';
import EditIcon from '@mui/icons-material/Edit';
import ScienceIcon from '@mui/icons-material/Science';
import SaveIcon from '@mui/icons-material/Save';
import CloseIcon from '@mui/icons-material/Close';
import UndoIcon from '@mui/icons-material/Undo';
import AddIcon from '@mui/icons-material/Add';
import type { SampleInfoColumn, SampleInfoRecord, SampleInfoAttachment } from '../types';
import type { AdaptiveColumnWidth } from '../utils/adaptiveColumns';
import DateRangePicker from './DateRangePicker';
import { recordCellSx } from '../utils/recordCellStyles';

const R = '2px';
const STATUS_WAIT_SAMPLE = '待取样';
const STATUS_WAIT_TEST = '待检测';
const STATUS_DONE = '已检测';
const STATUS_RETURNED = '已退回';
const STATUS_RETURN_CONFIRMED = '已退回已确认';
const STATUS_RETURN_DRAFT = '退回待修改';

type EditForm = Record<string, string>;

export interface SampleInfoRecordListProps {
  records: SampleInfoRecord[];
  total: number;
  page: number;
  pageSize: number;
  loading: boolean;
  statusFilter: string;
  statusOptions: readonly string[];
  typeFilter: string;
  typeOptions: { value: string; label: string }[];
  startDate: string;
  endDate: string;
  columns: SampleInfoColumn[];
  editColumns: SampleInfoColumn[];
  columnWidths: Record<string, AdaptiveColumnWidth>;
  attachmentsByRow: Record<number, SampleInfoAttachment[]>;
  attachments: Record<number, SampleInfoAttachment[]>;
  attachmentLoading: Record<number, boolean>;
  editingId: number | null;
  editForm: EditForm;
  hasPermission: (permission: string) => boolean;
  divisionOptions: { value: number; label: string }[];
  onPageChange: (page: number) => void;
  onStatusChange: (status: string) => void;
  onTypeChange: (typeKey: string) => void;
  onStartChange: (date: string) => void;
  onEndChange: (date: string) => void;
  onResetFilters: () => void;
  onEdit: (record: SampleInfoRecord) => void;
  onCancelEdit: () => void;
  onSaveEdit: (id: number) => void;
  onEditFormChange: (field: string, value: string) => void;
  onStatusFlow: (id: number, status: string) => void;
  onRecordWorkload: (id: number) => void;
  onWithdrawSample: (id: number) => void;
  onReturn: (record: SampleInfoRecord) => void;
  onConfirmReturn: (record: SampleInfoRecord) => void;
  currentUserId?: number;
  isSystemAdmin?: boolean;
  onLoadAttachments: (id: number) => void;
  onUploadAttachment: (id: number, file: File) => void;
  onDeleteAttachment: (attachmentId: number, recordId: number) => void;
  onOpenAttachment: (attachment: SampleInfoAttachment) => void;
  getRecordValue: (record: SampleInfoRecord, field: string) => any;
  formatDate: (value: string) => string;
  sortBy: string;
  sortDir: 'asc' | 'desc';
  onSortChange: (field: string) => void;
}

const valueText = (value: any) => {
  if (value === null || value === undefined || value === '') return '-';
  return String(value);
};

const statusSx = (status: string) => {
  if (status === STATUS_WAIT_SAMPLE) return { bgcolor: '#d32f2f', color: '#fff' };
  if (status === STATUS_WAIT_TEST) return { bgcolor: '#f9a825', color: '#1f1f1f' };
  if (status === STATUS_DONE) return { bgcolor: '#2e7d32', color: '#fff' };
  if (status === STATUS_RETURNED) return { bgcolor: '#7b1fa2', color: '#fff' };
  if (status === STATUS_RETURN_CONFIRMED) return { bgcolor: '#512da8', color: '#fff' };
  if (status === STATUS_RETURN_DRAFT) return { bgcolor: '#0288d1', color: '#fff' };
  return {};
};

const StatusChip = ({ status }: { status: string }) => (
  <Chip label={status || '-'} size="small" sx={{ ...statusSx(status), borderRadius: R, fontWeight: 700 }} />
);

const Wrap = ({ children, muted = false }: { children: React.ReactNode; muted?: boolean }) => (
  <Box sx={{ minWidth: 0, boxSizing: 'border-box', border: '1px solid #d9dfe7', borderRadius: R, bgcolor: '#fff', maxHeight: 180, overflow: 'auto', whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word', color: muted ? 'text.secondary' : 'text.primary', lineHeight: 1.45, px: 0.7, py: 0.45 }}>
    {children}
  </Box>
);

const Label = ({ children }: { children: React.ReactNode }) => (
  <Box component="span" sx={{ color: 'text.secondary', mr: 0.5 }}>{children}:</Box>
);

const SORTABLE_FIELDS = new Set([
  'submitted_at', 'created_at', 'status', 'seq_no', 'business_no', 'batch_no',
  'user_name', 'division_id', 'lab_name', 'project_name', 'detection_type',
  'detection_date', 'quantity', 'sampled_at', 'detected_by', 'main_components', 'notes',
]);

const EditField = ({ label, value, onChange, multiline = false }: { label: string; value: string; onChange: (value: string) => void; multiline?: boolean }) => (
  <TextField
    label={label}
    value={value || ''}
    onChange={e => onChange(e.target.value)}
    size="small"
    fullWidth
    multiline={multiline}
    minRows={multiline ? 2 : undefined}
    sx={{ '& .MuiInputBase-root': { fontSize: '0.78rem' }, '& .MuiInputLabel-root': { fontSize: '0.78rem' } }}
  />
);

const READONLY_EDIT_FIELDS = new Set([
  'status', 'seq_no', 'business_no', 'submitted_at', 'type_key', 'detection_type',
  'sampled_by', 'sampled_at', 'detected_by', 'return_reason', 'returned_by', 'returned_at',
  'return_confirmed_by', 'return_confirmed_at',
]);

const parseColumnOptions = (options: string | null): string[] => {
  if (!options) return [];
  try {
    const parsed = JSON.parse(options);
    if (Array.isArray(parsed)) return parsed.map(item => String(item)).filter(Boolean);
  } catch {
    // Older column configurations use a comma-separated option string.
  }
  return options.split(',').map(item => item.trim()).filter(Boolean);
};

const DynamicEditField = ({
  column, value, formatDate, onChange, readOnly = false, divisionOptions = [],
}: {
  column: SampleInfoColumn;
  value: string;
  formatDate: (value: string) => string;
  onChange: (value: string) => void;
  readOnly?: boolean;
  divisionOptions?: { value: number; label: string }[];
}) => {
  if (readOnly || READONLY_EDIT_FIELDS.has(column.field_key)) {
    return <Box sx={{ display: 'grid', gap: 0.25, minWidth: 0 }}>
      <Typography variant="caption" color="text.secondary">{column.label}</Typography>
      <Typography variant="body2" sx={{ overflowWrap: 'break-word', wordBreak: 'normal' }}>{column.data_type === 'date' ? formatDate(value) : valueText(value)}</Typography>
    </Box>;
  }

  const options = column.field_key === 'division_id'
    ? divisionOptions.map(item => ({ value: String(item.value), label: item.label }))
    : column.data_type === 'select'
      ? parseColumnOptions(column.options).map(item => ({ value: item, label: item }))
      : [];
  if (options.length > 0) {
    return <FormControl fullWidth size="small">
      <InputLabel>{column.label}</InputLabel>
      <Select value={value} label={column.label} onChange={event => onChange(String(event.target.value))}>
        <MenuItem value=""><em>请选择</em></MenuItem>
        {options.map(option => <MenuItem key={option.value} value={option.value}>{option.label}</MenuItem>)}
      </Select>
    </FormControl>;
  }

  const multiline = column.field_key === 'notes'
    || column.field_key === 'main_components'
    || /备注|注意|成分|描述/.test(column.label);
  return <TextField
    label={column.label}
    value={value}
    onChange={event => onChange(event.target.value)}
    type={column.data_type === 'date' ? 'date' : column.data_type === 'number' ? 'number' : 'text'}
    size="small"
    fullWidth
    multiline={multiline}
    minRows={multiline ? 2 : undefined}
    InputLabelProps={column.data_type === 'date' ? { shrink: true } : undefined}
    sx={{ '& .MuiInputBase-root': { fontSize: '0.78rem' }, '& .MuiInputLabel-root': { fontSize: '0.78rem' } }}
  />;
};

interface SampleInfoEditActionsProps {
  recordId: number;
  onCancel: () => void;
  onSave: (id: number) => void;
}

const SampleInfoEditActions = ({ recordId, onCancel, onSave }: SampleInfoEditActionsProps) => (
  <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end', flexWrap: 'wrap' }}>
    <Button size="small" variant="outlined" startIcon={<CloseIcon />} onClick={event => { event.stopPropagation(); onCancel(); }} sx={{ borderRadius: R }}>取消</Button>
    <Button size="small" variant="contained" startIcon={<SaveIcon />} onClick={event => { event.stopPropagation(); onSave(recordId); }} sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>保存</Button>
  </Box>
);

interface SampleInfoEditRowProps {
  record: SampleInfoRecord;
  columnCount: number;
  editColumns: SampleInfoColumn[];
  editForm: EditForm;
  formatDate: (value: string) => string;
  onEditFormChange: (field: string, value: string) => void;
  onCancel: () => void;
  onSave: (id: number) => void;
  divisionOptions: { value: number; label: string }[];
}

// This must remain a module-level component. Defining it inside the list would
// recreate its component type for every keystroke and make the input lose focus.
const SampleInfoEditRow = ({
  record, columnCount, editColumns, editForm, formatDate, onEditFormChange, onCancel, onSave, divisionOptions,
}: SampleInfoEditRowProps) => (
  <TableRow sx={{ bgcolor: '#f5fbf5' }}>
    <TableCell colSpan={Math.max(2, columnCount + 1)} sx={{ borderColor: '#d8e7d9', p: 1 }}>
      <Box
        onClick={event => event.stopPropagation()}
        onMouseDown={event => event.stopPropagation()}
        sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,minmax(0,1fr))', lg: 'repeat(4,minmax(0,1fr))' }, gap: 1 }}
      >
        {editColumns.filter(column => column.data_type !== 'attachment' && column.field_key !== 'attachment_files').map(column => {
          const longField = column.field_key === 'notes' || column.field_key === 'main_components' || /备注|注意|成分|描述/.test(column.label);
          return <Box key={column.field_key} sx={{ minWidth: 0, gridColumn: longField ? { xs: 'auto', lg: 'span 2' } : undefined }}>
            <DynamicEditField column={column} value={editForm[column.field_key] || ''} formatDate={formatDate} onChange={value => onEditFormChange(column.field_key, value)} readOnly={record.status === STATUS_RETURN_DRAFT && ['division_id', 'lab_name', 'project_name'].includes(column.field_key)} divisionOptions={divisionOptions} />
          </Box>;
        })}
        {editColumns.length === 0 && <Typography variant="body2" color="text.secondary">当前检测类型没有配置可编辑字段。</Typography>}
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'flex-end' }}><SampleInfoEditActions recordId={record.id} onCancel={onCancel} onSave={onSave} /></Box>
      </Box>
    </TableCell>
  </TableRow>
);

const AttachmentSummary = ({
  record,
  attachmentsByRow,
  attachments,
  loading,
  onLoad,
  onUpload,
  onDelete,
  onOpenAttachment,
  hasPermission,
  currentUserId,
  isSystemAdmin,
}: {
  record: SampleInfoRecord;
  attachmentsByRow: Record<number, SampleInfoAttachment[]>;
  attachments: Record<number, SampleInfoAttachment[]>;
  loading: boolean;
  onLoad: () => void;
  onUpload: (file: File) => void;
  onDelete: (id: number) => void;
  onOpenAttachment: (attachment: SampleInfoAttachment) => void;
  hasPermission: (permission: string) => boolean;
  currentUserId?: number;
  isSystemAdmin?: boolean;
}) => {
  const loaded = attachments[record.id] ?? attachmentsByRow[record.id] ?? [];
  const count = loaded.length;
  const canModify = record.status !== STATUS_RETURNED && record.status !== STATUS_RETURN_CONFIRMED && (
    Boolean(isSystemAdmin)
    || (record.sampled_at == null && (record.created_by_user_id === currentUserId || record.business_user_id === currentUserId) && hasPermission('sample-info:edit-own'))
    || hasPermission('sample-info:collect')
    || hasPermission('sample-info:complete')
  );
  return (
    <Box sx={{
      minWidth: 0,
      p: 1,
      border: '1px solid #d9dfe7',
      borderRadius: R,
      bgcolor: '#fbfcfe',
    }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 0.5, flexWrap: 'wrap' }}>
        <Chip icon={<AttachFileIcon />} label={`${count} 个`} size="small" sx={{ borderRadius: R }} onClick={onLoad} />
        {canModify && <Tooltip title="上传附件">
          <IconButton component="label" size="small" sx={{ p: 0.35, width: 32, height: 32, border: '1px solid #b8c2cc', borderRadius: R, bgcolor: '#fff' }}>
            <AddIcon fontSize="small" />
            <input type="file" hidden accept=".pdf,.doc,.docx" onChange={e => {
              const file = e.target.files?.[0];
              if (file) onUpload(file);
              e.currentTarget.value = '';
            }} />
          </IconButton>
        </Tooltip>}
        {loading && <CircularProgress size={15} />}
      </Box>
      {loaded.length > 0 && (
        <Box sx={{ mt: 0.65, display: 'flex', flexDirection: 'column', gap: 0.5 }}>
          {loaded.map(att => (
              <Box key={att.id} sx={{ display: 'grid', gridTemplateColumns: '16px minmax(0,1fr) 24px', alignItems: 'start', gap: 0.35, minWidth: 0 }}>
                <DescriptionIcon sx={{ fontSize: 15, mt: 0.15 }} />
              <Box sx={{ minWidth: 0, whiteSpace: 'normal', overflowWrap: 'break-word', wordBreak: 'normal', fontSize: '0.72rem', lineHeight: 1.4, cursor: 'pointer' }} onClick={() => onOpenAttachment(att)}>{att.file_name}</Box>
              {canModify ? <IconButton size="small" onClick={() => onDelete(att.id)} sx={{ p: 0.2, mt: -0.2 }} aria-label="删除附件"><DeleteOutlineIcon sx={{ fontSize: 15 }} /></IconButton> : <Box />}
            </Box>
          ))}
        </Box>
      )}
    </Box>
  );
};

const TimeSummary = ({ record, formatDate, showType = true }: { record: SampleInfoRecord; formatDate: (value: string) => string; showType?: boolean }) => (
  <Box sx={{ display: 'grid', gap: 0.35 }}>
    <Wrap><Label>送样</Label>{formatDate(record.submitted_at)}</Wrap>
    <Wrap><Label>取样</Label>{record.sampled_at ? `${valueText(record.sampled_by)} / ${formatDate(record.sampled_at)}` : '-'}</Wrap>
    <Wrap><Label>检测</Label>{record.detection_date ? `${valueText(record.detected_by)} / ${formatDate(record.detection_date)}` : '-'}</Wrap>
    {showType && <Wrap><Label>类型</Label>{valueText(record.detection_type)}</Wrap>}
  </Box>
);

const DetailSummary = ({ record, columns, getRecordValue }: { record: SampleInfoRecord; columns: SampleInfoColumn[]; getRecordValue: (record: SampleInfoRecord, field: string) => any }) => {
  const extra = columns.filter(c => c.show_in_list && !['status', 'seq_no', 'user_name', 'division_id', 'lab_name', 'project_name', 'quantity', 'batch_no', 'main_components', 'notes', 'submitted_at', 'detection_type', 'detection_date', 'type_key', 'sampled_by', 'sampled_at', 'detected_by'].includes(c.field_key) && c.data_type !== 'attachment');
  const detectionElements = extra.filter(col => col.label.includes('检测元素') || col.field_key.includes('element'));
  const otherExtra = extra.filter(col => !detectionElements.includes(col));
  return (
    <Box sx={{ display: 'grid', gap: 0.4 }}>
      <Wrap><Label>主要成分</Label><Box component="span" sx={{ fontWeight: 700 }}>{valueText(record.main_components)}</Box></Wrap>
      {detectionElements.map(col => <Wrap key={col.field_key}><Label>{col.label}</Label><Box component="span" sx={{ fontWeight: 600 }}>{valueText(getRecordValue(record, col.field_key))}</Box></Wrap>)}
      <Wrap><Label>备注</Label>{valueText(record.notes)}</Wrap>
      {otherExtra.map(col => <Wrap key={col.field_key}><Label>{col.label}</Label>{valueText(getRecordValue(record, col.field_key))}</Wrap>)}
    </Box>
  );
};

export default function SampleInfoRecordList(props: SampleInfoRecordListProps) {
  const {
    records, total, page, pageSize, loading, statusFilter, statusOptions, typeFilter, typeOptions, startDate, endDate, columns, editColumns, columnWidths, divisionOptions,
    attachmentsByRow, attachments, attachmentLoading, editingId, editForm,
    hasPermission, onPageChange, onStatusChange, onTypeChange, onStartChange, onEndChange, onResetFilters, onEdit, onCancelEdit, onSaveEdit,
    onEditFormChange, onStatusFlow, onRecordWorkload, onWithdrawSample, onReturn, onConfirmReturn, currentUserId, isSystemAdmin, onLoadAttachments, onUploadAttachment, onDeleteAttachment,
    getRecordValue, formatDate, onOpenAttachment, sortBy, sortDir, onSortChange,
  } = props;

  const editable = (record: SampleInfoRecord) => editingId === record.id;
  const extraClass = 'sample-info-desktop-list';
  // 操作字段参与按钮顺序和权限控制，但统一渲染到一个“操作”列，避免
  // 编辑、录入工作量、退回分别占用表格列宽。
  const dataColumns = columns.filter(column => column.data_type !== 'action');

  const valueBoxSx = {
    minWidth: 0,
    maxWidth: '100%',
    width: '100%',
    maxHeight: 180,
    overflowX: 'auto' as const,
    overflowY: 'auto' as const,
    whiteSpace: 'pre-wrap' as const,
    overflowWrap: 'anywhere' as const,
    wordBreak: 'break-word' as const,
  };

  // v2.3.13: 操作按钮宽度在所有断点都使用后台配置值，卡片视图同样可自定义按钮大小。
  const actionButtonSx = (fieldKey: string) => {
    const width = columnWidths[fieldKey]?.mobile;
    return {
      borderRadius: R,
      width: width ? `${width}px` : 'auto',
      minWidth: width ? `${width}px` : undefined,
      flex: width ? `0 0 ${width}px` : '0 0 auto',
      whiteSpace: 'normal' as const,
    };
  };

  const renderConfiguredAction = (record: SampleInfoRecord, fieldKey: string) => {
    if (fieldKey === 'action_edit') {
      const allowed = !editable(record)
        && hasPermission('sample-info:edit-own')
        && (record.status === STATUS_WAIT_SAMPLE || record.status === STATUS_RETURN_DRAFT)
        && (isSystemAdmin || record.created_by_user_id === currentUserId || record.business_user_id === currentUserId);
      return allowed ? <Button size="small" variant="outlined" startIcon={<EditIcon />} onClick={event => { event.stopPropagation(); onEdit(record); }} sx={actionButtonSx(fieldKey)}>编辑</Button> : null;
    }
    if (fieldKey === 'action_record_workload') {
      if (record.workload_recorded) {
        return <Button size="small" variant="outlined" color="success" disabled sx={actionButtonSx(fieldKey)}>已录入</Button>;
      }
      return record.status === STATUS_WAIT_TEST && hasPermission('sample-info:record-workload')
        ? <Button size="small" variant="outlined" color="primary" onClick={event => { event.stopPropagation(); onRecordWorkload(record.id); }} sx={actionButtonSx(fieldKey)}>录入工作量</Button>
        : null;
    }
    if (fieldKey === 'action_return') {
      if (record.status === STATUS_WAIT_SAMPLE && hasPermission('sample-info:return')) {
        return <Button size="small" variant="outlined" color="secondary" startIcon={<UndoIcon />} onClick={event => { event.stopPropagation(); onReturn(record); }} sx={actionButtonSx(fieldKey)}>退回</Button>;
      }
      if (record.status === STATUS_RETURNED && hasPermission('sample-info:return-confirm') && (isSystemAdmin || record.created_by_user_id === currentUserId)) {
        return <Button size="small" variant="outlined" color="primary" startIcon={<UndoIcon />} onClick={event => { event.stopPropagation(); onConfirmReturn(record); }} sx={{ ...actionButtonSx(fieldKey), whiteSpace: 'normal' }}>确认退回</Button>;
      }
    }
    if (fieldKey === 'action_collect') {
      return record.status === STATUS_WAIT_SAMPLE && hasPermission('sample-info:collect')
        ? <Button size="small" variant="contained" color="error" startIcon={<ScienceIcon />} onClick={event => { event.stopPropagation(); onStatusFlow(record.id, record.status); }} sx={actionButtonSx(fieldKey)}>取样</Button>
        : null;
    }
    if (fieldKey === 'action_withdraw') {
      return record.status === STATUS_WAIT_TEST && hasPermission('sample-info:withdraw')
        ? <Button size="small" variant="outlined" color="warning" startIcon={<UndoIcon />} onClick={event => { event.stopPropagation(); onWithdrawSample(record.id); }} sx={actionButtonSx(fieldKey)}>撤回取样</Button>
        : null;
    }
    if (fieldKey === 'action_complete') {
      return record.status === STATUS_WAIT_TEST && hasPermission('sample-info:complete')
        ? <Button size="small" variant="contained" color="success" startIcon={<CheckCircleIcon />} onClick={event => { event.stopPropagation(); onStatusFlow(record.id, record.status); }} sx={actionButtonSx(fieldKey)}>完成检测</Button>
        : null;
    }
    return null;
  };

  // v2.3.13: 这里必须是普通渲染函数，不能定义成组件。
  // 在组件内部定义子组件会让每次父组件渲染都产生新的组件类型，React 会卸载并重建整棵
  // 子树，造成退回原因等长内容区域滚动后被弹回顶部（附件回填、筛选变化都会触发）。
  const renderConfiguredActions = (record: SampleInfoRecord) => (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5, flexWrap: 'wrap', width: '100%', '& .MuiButton-root': { minHeight: 32, whiteSpace: 'normal', lineHeight: 1.25 } }}>
      {columns.filter(column => column.data_type === 'action').map(column => <React.Fragment key={column.field_key}>{renderConfiguredAction(record, column.field_key)}</React.Fragment>)}
    </Box>
  );

  const renderColumnValue = (record: SampleInfoRecord, column: SampleInfoColumn) => {
    // 退回原因只保留外层一个滚动容器，避免嵌套滚动导致位置错乱。
    if (column.field_key === 'status') return <Box sx={valueBoxSx}><Box sx={{ display: 'grid', gap: 0.35 }}><StatusChip status={record.status} />{record.return_reason && <Box sx={{ border: '1px solid #d9dfe7', borderRadius: R, bgcolor: '#fff', px: 0.7, py: 0.45, color: 'text.secondary', lineHeight: 1.45, whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word' }}>原因：{record.return_reason}</Box>}</Box></Box>;
    if (column.data_type === 'attachment' || column.field_key === 'attachment_files') {
      return <Box sx={valueBoxSx}><AttachmentSummary record={record} attachmentsByRow={attachmentsByRow} attachments={attachments} loading={!!attachmentLoading[record.id]} onLoad={() => onLoadAttachments(record.id)} onUpload={file => onUploadAttachment(record.id, file)} onDelete={id => onDeleteAttachment(id, record.id)} onOpenAttachment={onOpenAttachment} hasPermission={hasPermission} currentUserId={currentUserId} isSystemAdmin={isSystemAdmin} /></Box>;
    }
    if (column.field_key === 'seq_no') return <Box sx={valueBoxSx}><Box sx={{ fontWeight: 700 }}>#{record.seq_no ?? '—'}</Box><Box sx={{ color: 'text.secondary', fontSize: '0.68rem', overflowWrap: 'anywhere' }}>{valueText(record.business_no)}</Box></Box>;
    if (['submitted_at', 'detection_date', 'sampled_at'].includes(column.field_key)) {
      return <Box sx={valueBoxSx}>{formatDate(String(getRecordValue(record, column.field_key) || ''))}</Box>;
    }
    return <Box sx={valueBoxSx}>{valueText(getRecordValue(record, column.field_key))}</Box>;
  };

  return (
    <Paper elevation={0} sx={{ p: { xs: 1, sm: 1.5, lg: 2 }, borderRadius: R, border: '1px solid #e0e0e0' }}>
      <Box sx={{ mb: 1.5 }}>
        <Box>
          <Box component="h2" sx={{ m: 0, fontSize: '1.15rem', fontWeight: 700 }}>登记记录</Box>
          <Box sx={{ mt: 0.35, color: 'text.secondary', fontSize: '0.78rem' }}>记录状态、送样信息和处理时间</Box>
        </Box>
      </Box>

      <Paper variant="outlined" sx={{ p: 1.25, mb: 1.5, borderRadius: R, bgcolor: '#fbfcfe' }}>
        <DateRangePicker startDate={startDate} endDate={endDate} onStartChange={onStartChange} onEndChange={onEndChange}>
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>检测类型</InputLabel>
            <Select value={typeFilter} label="检测类型" onChange={event => onTypeChange(event.target.value)}>
              <MenuItem value="">全部检测类型</MenuItem>
              {typeOptions.map(type => <MenuItem key={type.value} value={type.value}>{type.label}</MenuItem>)}
            </Select>
          </FormControl>
          <FormControl size="small" sx={{ minWidth: 120 }}>
            <InputLabel>状态</InputLabel>
            <Select value={statusFilter} label="状态" onChange={event => onStatusChange(event.target.value)}>
              {statusOptions.map(status => <MenuItem key={status} value={status}>{status}</MenuItem>)}
            </Select>
          </FormControl>
          <Button size="small" variant="outlined" onClick={onResetFilters} sx={{ borderRadius: R }}>重置</Button>
        </DateRangePicker>
      </Paper>

      {loading ? <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={32} /></Box> : (
        <>
          <TableContainer className={extraClass} sx={{ display: { xs: 'none', lg: 'block' }, width: '100%', overflow: 'hidden', border: '1px solid #e5e5e5', borderRadius: R }}>
            <Table size="small" sx={{ width: '100%', minWidth: 0, tableLayout: 'fixed' }}>
              <colgroup>
                {dataColumns.map(column => <col key={column.field_key} style={{ width: columnWidths[column.field_key]?.desktop }} />)}
                <col style={{ width: columnWidths._action?.desktop }} />
              </colgroup>
              <TableHead><TableRow sx={{ bgcolor: '#f5f7f5' }}>
                {dataColumns.map(column => {
                  const sortable = SORTABLE_FIELDS.has(column.field_key);
                  return <TableCell key={column.field_key} sx={{ fontWeight: 700, whiteSpace: 'normal', overflowWrap: 'break-word', wordBreak: 'normal', px: 0.8, py: 1, borderColor: '#e0e0e0' }}>
                    {sortable ? <TableSortLabel
                      active={sortBy === column.field_key}
                      direction={sortBy === column.field_key ? sortDir : 'asc'}
                      onClick={event => { event.stopPropagation(); onSortChange(column.field_key); }}
                      sx={{ '& .MuiTableSortLabel-icon': { fontSize: '1rem' } }}
                    >{column.label}</TableSortLabel> : column.label}
                  </TableCell>;
                })}
                <TableCell sx={{ fontWeight: 700, whiteSpace: 'normal', overflowWrap: 'break-word', wordBreak: 'normal', px: 0.8, py: 1, borderColor: '#e0e0e0', minWidth: 0 }}>操作</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {records.length === 0 ? <TableRow><TableCell colSpan={Math.max(2, dataColumns.length + 1)} align="center" sx={{ py: 4, color: 'text.secondary' }}>暂无登记记录</TableCell></TableRow> : records.map(record => (
                  <React.Fragment key={record.id}>
                    <TableRow hover sx={{ bgcolor: record.status === STATUS_DONE ? '#fbfbfb' : '#fff', verticalAlign: 'top', '& td': { px: 0.8, py: 1 } }}>
                      {dataColumns.map(column => <TableCell key={column.field_key} sx={{ ...recordCellSx, width: columnWidths[column.field_key]?.desktop, overflow: 'hidden' }}>{renderColumnValue(record, column)}</TableCell>)}
                      <TableCell sx={{ ...recordCellSx, width: columnWidths._action?.desktop, px: 0.5, textAlign: 'center', overflow: 'hidden' }}>{renderConfiguredActions(record)}</TableCell>
                    </TableRow>
                    {editable(record) && <SampleInfoEditRow record={record} columnCount={dataColumns.length + 1} editColumns={editColumns} editForm={editForm} formatDate={formatDate} onEditFormChange={onEditFormChange} onCancel={onCancelEdit} onSave={onSaveEdit} divisionOptions={divisionOptions} />}
                  </React.Fragment>
                ))}
              </TableBody>
            </Table>
          </TableContainer>

          <Box sx={{ display: { xs: 'grid', lg: 'none' }, gap: 1 }}>
            {records.length === 0 ? <Box sx={{ py: 4, textAlign: 'center', color: 'text.secondary' }}>暂无登记记录</Box> : records.map(record => (
              <Card key={record.id} variant="outlined" sx={{ borderRadius: R, borderLeft: `4px solid ${record.status === STATUS_WAIT_SAMPLE ? '#d32f2f' : record.status === STATUS_WAIT_TEST ? '#f9a825' : '#2e7d32'}` }}>
                <CardContent sx={{ p: 1.25, '&:last-child': { pb: 1.25 } }}>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 1, mb: 1 }}><Box><Box sx={{ fontWeight: 700 }}>#{record.seq_no ?? '—'}</Box><Box sx={{ color: 'text.secondary', fontSize: '0.7rem', overflowWrap: 'anywhere' }}>{valueText(record.business_no)}</Box></Box><StatusChip status={record.status} /></Box>
                  {editable(record) ? <Box onClick={event => event.stopPropagation()} onMouseDown={event => event.stopPropagation()} sx={{ display: 'grid', gap: 1 }}>
                    {editColumns.filter(column => column.data_type !== 'attachment' && column.field_key !== 'attachment_files').map(column => <DynamicEditField key={column.field_key} column={column} value={editForm[column.field_key] || ''} formatDate={formatDate} onChange={value => onEditFormChange(column.field_key, value)} readOnly={record.status === STATUS_RETURN_DRAFT && ['division_id', 'lab_name', 'project_name'].includes(column.field_key)} divisionOptions={divisionOptions} />)}
                    {editColumns.length === 0 && <Typography variant="body2" color="text.secondary">当前检测类型没有配置可编辑字段。</Typography>}
                    <SampleInfoEditActions recordId={record.id} onCancel={onCancelEdit} onSave={onSaveEdit} />
                  </Box> : <>
                    <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1, mb: 1 }}>
                      {columns.filter(column => column.field_key !== 'status' && column.data_type !== 'attachment' && column.data_type !== 'action' && column.field_key !== 'attachment_files').map(column => <Wrap key={column.field_key}><Label>{column.label}</Label>{column.field_key === 'submitted_at' ? formatDate(record.submitted_at) : valueText(getRecordValue(record, column.field_key))}</Wrap>)}
                    </Box>
                    {record.return_reason && <Box sx={{ mb: 1 }}><Wrap muted>退回原因：{record.return_reason}</Wrap></Box>}
                    {columns.some(column => column.data_type === 'attachment' || column.field_key === 'attachment_files') && <Box sx={{ mt: 1.5 }}><AttachmentSummary record={record} attachmentsByRow={attachmentsByRow} attachments={attachments} loading={!!attachmentLoading[record.id]} onLoad={() => onLoadAttachments(record.id)} onUpload={file => onUploadAttachment(record.id, file)} onDelete={id => onDeleteAttachment(id, record.id)} onOpenAttachment={onOpenAttachment} hasPermission={hasPermission} currentUserId={currentUserId} isSystemAdmin={isSystemAdmin} /></Box>}
                    <Box sx={{ mt: { xs: 1.75, lg: 1 }, display: 'grid', gap: 0.75 }}>{renderConfiguredActions(record)}</Box>
                  </>}
                </CardContent>
              </Card>
            ))}
          </Box>

          <TablePagination component="div" count={total} page={page} onPageChange={(_, nextPage) => onPageChange(nextPage)} rowsPerPage={pageSize} rowsPerPageOptions={[pageSize]} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { minHeight: 52, flexWrap: 'wrap', justifyContent: { xs: 'center', sm: 'flex-end' }, py: 0.5 }, '& .MuiTablePagination-actions': { ml: { xs: 0.5, sm: 2 } } }} />
        </>
      )}
    </Paper>
  );
}
