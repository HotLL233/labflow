import React from 'react';
import {
  Box, Button, Card, CardContent, Chip, CircularProgress, IconButton,
  Paper, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination,
  TableRow, TextField, Tooltip,
} from '@mui/material';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import DescriptionIcon from '@mui/icons-material/Description';
import EditIcon from '@mui/icons-material/Edit';
import ScienceIcon from '@mui/icons-material/Science';
import SaveIcon from '@mui/icons-material/Save';
import CloseIcon from '@mui/icons-material/Close';
import type { SampleInfoColumn, SampleInfoRecord, SampleInfoAttachment } from '../types';

const R = '2px';
const STATUS_WAIT_SAMPLE = '待取样';
const STATUS_WAIT_TEST = '待检测';
const STATUS_DONE = '已检测';

type EditForm = Record<string, string>;

export interface SampleInfoRecordListProps {
  records: SampleInfoRecord[];
  total: number;
  page: number;
  pageSize: number;
  loading: boolean;
  statusFilter: string;
  statusOptions: readonly string[];
  columns: SampleInfoColumn[];
  attachmentsByRow: Record<number, SampleInfoAttachment[]>;
  attachments: Record<number, SampleInfoAttachment[]>;
  attachmentLoading: Record<number, boolean>;
  editingId: number | null;
  editForm: EditForm;
  hasPermission: (permission: string) => boolean;
  onPageChange: (page: number) => void;
  onStatusChange: (status: string) => void;
  onEdit: (record: SampleInfoRecord) => void;
  onCancelEdit: () => void;
  onSaveEdit: (id: number) => void;
  onEditFormChange: (field: string, value: string) => void;
  onStatusFlow: (id: number, status: string) => void;
  onLoadAttachments: (id: number) => void;
  onUploadAttachment: (id: number, file: File) => void;
  onDeleteAttachment: (attachmentId: number, recordId: number) => void;
  getAttachmentUrl: (attachmentId: number) => string;
  getRecordValue: (record: SampleInfoRecord, field: string) => any;
  formatDate: (value: string) => string;
}

const valueText = (value: any) => {
  if (value === null || value === undefined || value === '') return '-';
  return String(value);
};

const statusSx = (status: string) => {
  if (status === STATUS_WAIT_SAMPLE) return { bgcolor: '#d32f2f', color: '#fff' };
  if (status === STATUS_WAIT_TEST) return { bgcolor: '#f9a825', color: '#1f1f1f' };
  if (status === STATUS_DONE) return { bgcolor: '#2e7d32', color: '#fff' };
  return {};
};

const StatusChip = ({ status }: { status: string }) => (
  <Chip label={status || '-'} size="small" sx={{ ...statusSx(status), borderRadius: R, fontWeight: 700 }} />
);

const Wrap = ({ children, muted = false }: { children: React.ReactNode; muted?: boolean }) => (
  <Box sx={{ minWidth: 0, whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word', color: muted ? 'text.secondary' : 'text.primary', lineHeight: 1.45 }}>
    {children}
  </Box>
);

const Label = ({ children }: { children: React.ReactNode }) => (
  <Box component="span" sx={{ color: 'text.secondary', mr: 0.5 }}>{children}:</Box>
);

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

const AttachmentSummary = ({
  record,
  attachmentsByRow,
  attachments,
  loading,
  onLoad,
  onUpload,
  onDelete,
  getAttachmentUrl,
}: {
  record: SampleInfoRecord;
  attachmentsByRow: Record<number, SampleInfoAttachment[]>;
  attachments: Record<number, SampleInfoAttachment[]>;
  loading: boolean;
  onLoad: () => void;
  onUpload: (file: File) => void;
  onDelete: (id: number) => void;
  getAttachmentUrl: (id: number) => string;
}) => {
  const loaded = attachments[record.id] ?? attachmentsByRow[record.id] ?? [];
  const count = loaded.length;
  return (
    <Box sx={{ minWidth: 0 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 0.5, flexWrap: 'wrap' }}>
        <Chip icon={<AttachFileIcon />} label={`${count} 个`} size="small" sx={{ borderRadius: R }} onClick={onLoad} />
        <Tooltip title="上传附件">
          <IconButton component="label" size="small" sx={{ p: 0.35 }}>
            <AttachFileIcon fontSize="small" />
            <input type="file" hidden accept=".pdf,.doc,.docx" onChange={e => {
              const file = e.target.files?.[0];
              if (file) onUpload(file);
              e.currentTarget.value = '';
            }} />
          </IconButton>
        </Tooltip>
        {loading && <CircularProgress size={15} />}
      </Box>
      {loaded.length > 0 && (
        <Box sx={{ mt: 0.65, display: 'flex', flexDirection: 'column', gap: 0.5 }}>
          {loaded.map(att => (
            <Box key={att.id} sx={{ display: 'grid', gridTemplateColumns: '16px minmax(0,1fr) 24px', alignItems: 'start', gap: 0.35, minWidth: 0 }}>
              <DescriptionIcon sx={{ fontSize: 15, mt: 0.15 }} />
              <Tooltip title={att.file_name}>
                <Box sx={{ minWidth: 0, whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word', fontSize: '0.72rem', lineHeight: 1.4, cursor: 'pointer' }} onClick={() => window.open(getAttachmentUrl(att.id), '_blank')}>{att.file_name}</Box>
              </Tooltip>
              <IconButton size="small" onClick={() => onDelete(att.id)} sx={{ p: 0.2, mt: -0.2 }} aria-label="删除附件"><DeleteOutlineIcon sx={{ fontSize: 15 }} /></IconButton>
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
    records, total, page, pageSize, loading, statusFilter, statusOptions, columns,
    attachmentsByRow, attachments, attachmentLoading, editingId, editForm,
    hasPermission, onPageChange, onStatusChange, onEdit, onCancelEdit, onSaveEdit,
    onEditFormChange, onStatusFlow, onLoadAttachments, onUploadAttachment, onDeleteAttachment,
    getRecordValue, formatDate, getAttachmentUrl,
  } = props;

  const editable = (record: SampleInfoRecord) => editingId === record.id;
  const extraClass = 'sample-info-desktop-list';

  const EditActions = ({ record }: { record: SampleInfoRecord }) => (
    <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end', flexWrap: 'wrap' }}>
      <Button size="small" variant="outlined" startIcon={<CloseIcon />} onClick={onCancelEdit} sx={{ borderRadius: R }}>取消</Button>
      <Button size="small" variant="contained" startIcon={<SaveIcon />} onClick={() => onSaveEdit(record.id)} sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>保存</Button>
    </Box>
  );

  const EditRow = ({ record }: { record: SampleInfoRecord }) => (
    <TableRow sx={{ bgcolor: '#f5fbf5' }}>
      <TableCell colSpan={9} sx={{ borderColor: '#d8e7d9', p: 1 }}>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,minmax(0,1fr))', lg: 'repeat(4,minmax(0,1fr))' }, gap: 1 }}>
          <EditField label="送样人" value={editForm.user_name || ''} onChange={v => onEditFormChange('user_name', v)} />
          <EditField label="样品批号" value={editForm.batch_no || ''} onChange={v => onEditFormChange('batch_no', v)} />
          <EditField label="实验室/车间" value={editForm.lab_name || ''} onChange={v => onEditFormChange('lab_name', v)} />
          <EditField label="项目名称" value={editForm.project_name || ''} onChange={v => onEditFormChange('project_name', v)} />
          <EditField label="送样时间" value={editForm.submitted_at || ''} onChange={v => onEditFormChange('submitted_at', v)} />
          <Box sx={{ gridColumn: { xs: 'auto', lg: 'span 2' } }}><EditField label="样品主要成分" value={editForm.main_components || ''} onChange={v => onEditFormChange('main_components', v)} multiline /></Box>
          <Box sx={{ gridColumn: { xs: 'auto', lg: 'span 2' } }}><EditField label="备注" value={editForm.notes || ''} onChange={v => onEditFormChange('notes', v)} multiline /></Box>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'flex-end' }}><EditActions record={record} /></Box>
        </Box>
      </TableCell>
    </TableRow>
  );

  const RecordActions = ({ record }: { record: SampleInfoRecord }) => (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.25, justifyContent: 'flex-end', flexWrap: 'wrap' }}>
      {!editable(record) && !record.status.includes('已检测') && <Tooltip title="编辑记录"><IconButton size="small" onClick={() => onEdit(record)} aria-label="编辑记录"><EditIcon fontSize="small" /></IconButton></Tooltip>}
      {record.status === STATUS_WAIT_SAMPLE && hasPermission('sample-info:collect') && <Button size="small" variant="contained" color="error" startIcon={<ScienceIcon />} onClick={() => onStatusFlow(record.id, record.status)} sx={{ borderRadius: R, whiteSpace: 'nowrap' }}>取样</Button>}
      {record.status === STATUS_WAIT_TEST && hasPermission('sample-info:complete') && <Button size="small" variant="contained" color="success" startIcon={<CheckCircleIcon />} onClick={() => onStatusFlow(record.id, record.status)} sx={{ borderRadius: R, whiteSpace: 'nowrap' }}>完成检测</Button>}
    </Box>
  );

  return (
    <Paper elevation={0} sx={{ p: { xs: 1, sm: 1.5, lg: 2 }, borderRadius: R, border: '1px solid #e0e0e0' }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1.5, gap: 1, flexWrap: 'wrap' }}>
        <Box>
          <Box component="h2" sx={{ m: 0, fontSize: '1.15rem', fontWeight: 700 }}>登记记录</Box>
          <Box sx={{ mt: 0.35, color: 'text.secondary', fontSize: '0.78rem' }}>记录状态、送样信息和处理时间</Box>
        </Box>
        <select value={statusFilter} onChange={e => onStatusChange(e.target.value)} style={{ height: 40, minWidth: 130, border: '1px solid #c7c7c7', borderRadius: 2, padding: '0 8px', background: '#fff', fontSize: 14 }} aria-label="状态筛选">
          {statusOptions.map(status => <option key={status} value={status}>{status}</option>)}
        </select>
      </Box>

      {loading ? <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={32} /></Box> : (
        <>
          <TableContainer className={extraClass} sx={{ display: { xs: 'none', lg: 'block' }, width: '100%', overflow: 'hidden', border: '1px solid #e5e5e5', borderRadius: R }}>
            <Table size="small" sx={{ width: '100%', tableLayout: 'fixed' }}>
              <colgroup>
                <col style={{ width: '6%' }} /><col style={{ width: '7%' }} /><col style={{ width: '11%' }} /><col style={{ width: '23%' }} />
                <col style={{ width: '9%' }} /><col style={{ width: '8%' }} /><col style={{ width: '12%' }} /><col style={{ width: '16%' }} /><col style={{ width: '8%' }} />
              </colgroup>
              <TableHead><TableRow sx={{ bgcolor: '#f5f7f5' }}>
                {['状态', '序号/编号', '批号/数量', '主要成分/检测元素/备注', '实验室/项目', '送样人/部门', '时间/检测类型', '附件', '操作'].map(title => <TableCell key={title} sx={{ fontWeight: 700, whiteSpace: 'normal', px: 0.8, py: 1, borderColor: '#e0e0e0' }}>{title}</TableCell>)}
              </TableRow></TableHead>
              <TableBody>
                {records.length === 0 ? <TableRow><TableCell colSpan={9} align="center" sx={{ py: 4, color: 'text.secondary' }}>暂无登记记录</TableCell></TableRow> : records.map(record => (
                  <React.Fragment key={record.id}>
                    <TableRow hover sx={{ bgcolor: record.status === STATUS_DONE ? '#fbfbfb' : '#fff', verticalAlign: 'top', '& td': { px: 0.8, py: 1 } }}>
                      <TableCell><StatusChip status={record.status} /></TableCell>
                      <TableCell><Wrap><Box sx={{ fontWeight: 700 }}>#{record.seq_no}</Box><Box sx={{ color: 'text.secondary', fontSize: '0.68rem', overflowWrap: 'anywhere' }}>{valueText(record.business_no)}</Box></Wrap></TableCell>
                      <TableCell><Wrap><Box sx={{ fontWeight: 700 }}>{valueText(record.batch_no)}</Box><Box color="text.secondary">数量：{valueText(record.quantity)}</Box></Wrap></TableCell>
                      <TableCell><DetailSummary record={record} columns={columns} getRecordValue={getRecordValue} /></TableCell>
                      <TableCell><Wrap><Box>{valueText(record.lab_name)}</Box><Box color="text.secondary">{valueText(record.project_name)}</Box></Wrap></TableCell>
                      <TableCell><Wrap><Box>{valueText(record.user_name)}</Box><Box color="text.secondary">{valueText(record.division_name)}</Box></Wrap></TableCell>
                      <TableCell><TimeSummary record={record} formatDate={formatDate} /></TableCell>
                      <TableCell><AttachmentSummary record={record} attachmentsByRow={attachmentsByRow} attachments={attachments} loading={!!attachmentLoading[record.id]} onLoad={() => onLoadAttachments(record.id)} onUpload={file => onUploadAttachment(record.id, file)} onDelete={id => onDeleteAttachment(id, record.id)} getAttachmentUrl={getAttachmentUrl} /></TableCell>
                      <TableCell><RecordActions record={record} /></TableCell>
                    </TableRow>
                    {editable(record) && <EditRow record={record} />}
                  </React.Fragment>
                ))}
              </TableBody>
            </Table>
          </TableContainer>

          <Box sx={{ display: { xs: 'grid', lg: 'none' }, gap: 1 }}>
            {records.length === 0 ? <Box sx={{ py: 4, textAlign: 'center', color: 'text.secondary' }}>暂无登记记录</Box> : records.map(record => (
              <Card key={record.id} variant="outlined" sx={{ borderRadius: R, borderLeft: `4px solid ${record.status === STATUS_WAIT_SAMPLE ? '#d32f2f' : record.status === STATUS_WAIT_TEST ? '#f9a825' : '#2e7d32'}` }}>
                <CardContent sx={{ p: 1.25, '&:last-child': { pb: 1.25 } }}>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 1, mb: 1 }}><Box><Box sx={{ fontWeight: 700 }}>#{record.seq_no}</Box><Box sx={{ color: 'text.secondary', fontSize: '0.7rem', overflowWrap: 'anywhere' }}>{valueText(record.business_no)}</Box></Box><StatusChip status={record.status} /></Box>
                  {editable(record) ? <Box sx={{ display: 'grid', gap: 1 }}><EditField label="送样人" value={editForm.user_name || ''} onChange={v => onEditFormChange('user_name', v)} /><EditField label="样品批号" value={editForm.batch_no || ''} onChange={v => onEditFormChange('batch_no', v)} /><EditField label="实验室/车间" value={editForm.lab_name || ''} onChange={v => onEditFormChange('lab_name', v)} /><EditField label="项目名称" value={editForm.project_name || ''} onChange={v => onEditFormChange('project_name', v)} /><EditField label="送样时间" value={editForm.submitted_at || ''} onChange={v => onEditFormChange('submitted_at', v)} /><EditField label="样品主要成分" value={editForm.main_components || ''} onChange={v => onEditFormChange('main_components', v)} multiline /><EditField label="备注" value={editForm.notes || ''} onChange={v => onEditFormChange('notes', v)} multiline /><EditActions record={record} /></Box> : <>
                    <Box sx={{ mb: 1, pb: 1, borderBottom: '1px solid #edf1ed' }}><Wrap><Label>批号</Label><Box component="span" sx={{ fontWeight: 700 }}>{valueText(record.batch_no)}</Box></Wrap><Wrap><Label>数量</Label>{valueText(record.quantity)}</Wrap></Box>
                    <Box sx={{ mb: 1, pb: 1, borderBottom: '1px solid #edf1ed' }}><DetailSummary record={record} columns={columns} getRecordValue={getRecordValue} /></Box>
                    <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1, mb: 1 }}><Wrap><Label>实验室</Label>{valueText(record.lab_name)}</Wrap><Wrap><Label>项目</Label>{valueText(record.project_name)}</Wrap><Wrap><Label>送样人</Label>{valueText(record.user_name)}</Wrap><Wrap><Label>部门</Label>{valueText(record.division_name)}</Wrap><Wrap><Label>类型</Label>{valueText(record.detection_type)}</Wrap></Box>
                    <TimeSummary record={record} formatDate={formatDate} showType={false} /><Box sx={{ mt: 1 }}><AttachmentSummary record={record} attachmentsByRow={attachmentsByRow} attachments={attachments} loading={!!attachmentLoading[record.id]} onLoad={() => onLoadAttachments(record.id)} onUpload={file => onUploadAttachment(record.id, file)} onDelete={id => onDeleteAttachment(id, record.id)} getAttachmentUrl={getAttachmentUrl} /></Box><Box sx={{ mt: 1 }}><RecordActions record={record} /></Box>
                  </>}
                </CardContent>
              </Card>
            ))}
          </Box>

          <TablePagination component="div" count={total} page={page} onPageChange={(_, nextPage) => onPageChange(nextPage)} rowsPerPage={pageSize} rowsPerPageOptions={[pageSize]} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`} />
        </>
      )}
    </Paper>
  );
}
