import React, { useMemo } from 'react';
import { Box, Chip, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Typography } from '@mui/material';

const LABELS: Record<string, string> = {
  business_no: '业务编号', project_name: '项目', lab_name: '实验室', group_name: '实验室',
  method_name: '检测方法', instrument_code: '仪器编号', instrument_type: '仪器类型',
  user_name: '人员', sender: '送样人', quantity: '数量', multiplier: '单价倍率',
  coefficient_snapshot: '系数', recorded_at: '记录时间', submitted_at: '送样时间',
  detection_date: '检测日期', batch_no: '批号', main_components: '主要成分',
  high_item: '高项', notes: '备注', status: '状态', sampler: '取样人',
  sampled_by: '取样人', sampled_at: '取样时间', detected_by: '检测人',
  detected_at: '检测完成时间', project_status: '项目状态', is_active: '启用状态',
  name: '名称', full_name: '全称', label: '显示名称', description: '描述',
  color: '颜色', sort_order: '排序', type_names: '检测类型', method_names: '关联方法',
  lab_names: '关联实验室', role_names: '角色', permissions: '权限', deleted_at: '删除时间',
};

const HIDDEN_KEYS = new Set([
  'project_id', 'method_id', 'group_id', 'division_id', 'instrument_id', 'owner_user_id',
  'created_by_user_id', 'subject_user_id', 'lab_ids', 'method_ids', 'type_ids', 'password',
]);

const normalize = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  const object = value as Record<string, unknown>;
  const data = object.data && typeof object.data === 'object' && !Array.isArray(object.data)
    ? object.data as Record<string, unknown> : {};
  return { ...data, ...object };
};

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const display = (value: unknown): string => {
  if (value === null || value === undefined || value === '') return '-';
  if (typeof value === 'boolean') return value ? '是' : '否';
  if (Array.isArray(value)) return value.length ? value.join('、') : '-';
  if (typeof value === 'object') return JSON.stringify(value, null, 2);
  return String(value);
};

interface Props {
  before?: Record<string, unknown> | null;
  after?: Record<string, unknown> | null;
  compact?: boolean;
}

const AuditDiffTable: React.FC<Props> = ({ before, after, compact = false }) => {
  const rows = useMemo(() => {
    const left = normalize(before);
    const right = normalize(after);
    return [...new Set([...Object.keys(left), ...Object.keys(right)])]
      .filter(key => key !== 'data' && !HIDDEN_KEYS.has(key) && !same(left[key], right[key]))
      .map(key => ({ key, label: LABELS[key] || key, before: left[key], after: right[key] }));
  }, [before, after]);

  if (!rows.length) return <Typography variant="body2" color="text.secondary">未记录字段变化</Typography>;

  return (
    <TableContainer sx={{ border: '1px solid #e0e0e0', maxHeight: compact ? 260 : 420 }}>
      <Table size="small" stickyHeader>
        <TableHead><TableRow>
          <TableCell sx={{ width: compact ? 105 : 150, fontWeight: 700 }}>变更字段</TableCell>
          <TableCell sx={{ fontWeight: 700, bgcolor: '#fff7ed' }}>修改前</TableCell>
          <TableCell sx={{ fontWeight: 700, bgcolor: '#eff6ff' }}>修改后</TableCell>
        </TableRow></TableHead>
        <TableBody>{rows.map(row => <TableRow key={row.key}>
          <TableCell><Chip size="small" label={row.label} variant="outlined" sx={{ borderRadius: '2px' }} /></TableCell>
          <TableCell sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', color: '#9a3412' }}>{display(row.before)}</TableCell>
          <TableCell sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', color: '#1d4ed8', fontWeight: 600 }}>{display(row.after)}</TableCell>
        </TableRow>)}</TableBody>
      </Table>
      {!compact && <Box component="details" sx={{ px: 1.5, py: 1, borderTop: '1px solid #eee' }}>
        <Typography component="summary" variant="caption" sx={{ cursor: 'pointer' }}>查看完整技术快照</Typography>
        <Box component="pre" sx={{ m: 0, mt: 1, p: 1, bgcolor: '#fafafa', fontSize: 11, whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>
          {JSON.stringify({ before, after }, null, 2)}
        </Box>
      </Box>}
    </TableContainer>
  );
};

export default AuditDiffTable;
