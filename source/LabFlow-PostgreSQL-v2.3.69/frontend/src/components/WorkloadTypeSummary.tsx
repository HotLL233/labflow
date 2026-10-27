import React, { useEffect, useMemo, useState } from 'react';
import { Accordion, AccordionDetails, AccordionSummary, Alert, Box, Breadcrumbs, Button, CircularProgress, Paper, Table, TableBody, TableCell, TableHead, TablePagination, TableRow, ToggleButton, ToggleButtonGroup, Typography, useMediaQuery, useTheme } from '@mui/material';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import dayjs from 'dayjs';
import { useUser } from '../UserContext';
import { hasPermission } from '../constants/permissions';
import { exportWorkloadReport, getWorkloadReport, type WorkloadBreakdown, type WorkloadDetail, type WorkloadReport, type WorkloadReportQuery } from '../api/workloadReport';
import { nextWorkloadDimension, workloadBreakdownLabel, workloadDetailPersonLabel, workloadDrillQuery, workloadShare, type WorkloadDrillDimension, type WorkloadDrillStep } from '../utils/workloadTypeDrill';
import { getAdaptiveColumnWidths } from '../utils/adaptiveColumns';
import { cardContainerSx, cardFieldSx } from '../utils/recordTableStyle';
import { useRecordTableStyle, type RecordTableStyleApi } from './recordTable/useRecordTableStyle';
import TableStyleBar from './recordTable/TableStyleBar';
import RecordCardSummary from './recordTable/RecordCardSummary';
import WorkloadAuxiliaryConfig from './WorkloadAuxiliaryConfig';

const number = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 2 });
const coefficient = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 12 });
const dimensionNames: Record<WorkloadDrillDimension, string> = { type: '类型', method: '方法', instrument: '仪器', auxiliary_category: '辅助类型', auxiliary_method: '辅助方法 / 事项' };
const sourceNames: Record<string, string> = { analysis: '分析检测', rd_sample: '研发送样取样录入', sample_info_sample: '样品信息登记取样录入', auxiliary_work: '辅助工作' };
const summaryColumns = [{ key: 'name', label: '分类', width: 220 }, { key: 'quantity', label: '数量', width: 90 }, { key: 'workload', label: '工作量', width: 110 }, { key: 'count', label: '记录数', width: 80 }, { key: 'share', label: '占当前层', width: 95 }];
const detailColumns = [{ key: 'person', label: '人员 / 日期', width: 160 }, { key: 'type', label: '类型 / 来源', width: 160 }, { key: 'department', label: '部门 / 实验室', width: 140 }, { key: 'method', label: '项目 / 方法 / 仪器', width: 220 }, { key: 'quantity', label: '数量', width: 65 }, { key: 'coefficient', label: '系数', width: 70 }, { key: 'workload', label: '工作量', width: 85 }];
const summaryDefaultRows = [['name', 'workload'], ['quantity', 'count', 'share']];
const detailDefaultRows = [['person', 'workload'], ['type', 'method']];
type DisplayRow = { key: string; values: Record<string, string> };
const scrollCellSx = { maxHeight: 180, overflow: 'auto', minWidth: 0, whiteSpace: 'pre-line', overflowWrap: 'anywhere' };
const historicalName = (row: WorkloadBreakdown) => `${row.name || '未分类'}${row.key.startsWith('legacy:') ? '（历史未绑定）' : ''}`;

export default function WorkloadTypeSummary({ query: baseQuery }: { query: WorkloadReportQuery }) {
  const { user } = useUser();
  const narrow = useMediaQuery(useTheme().breakpoints.down('sm'));
  const canExport = !!(user?.is_admin || hasPermission(user?.permissions ?? [], 'stats:workload:export'));
  const [path, setPath] = useState<WorkloadDrillStep[]>([]);
  const [preferred, setPreferred] = useState<WorkloadDrillDimension>('method');
  const [showDetails, setShowDetails] = useState(false);
  const [page, setPage] = useState(0);
  const [revision, setRevision] = useState(0);
  const [report, setReport] = useState<WorkloadReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [exporting, setExporting] = useState(false);
  const baseKey = JSON.stringify(baseQuery);
  const dimension = nextWorkloadDimension(path, preferred);
  const detailsActive = showDetails || dimension === null;
  const rows = report?.breakdowns[dimension ?? 'type'] ?? [];
  const summaryRows = useMemo<DisplayRow[]>(() => rows.map(row => ({ key: JSON.stringify(path) + ':' + row.key, values: { name: workloadBreakdownLabel(dimension ?? 'type', row, rows), quantity: number(row.detection_quantity + row.auxiliary_quantity), workload: number(row.total_workload), count: number(row.record_count), share: workloadShare(row.total_workload, report?.summary.total_workload ?? 0) } })), [rows, path, report, dimension]);
  const detailRows = useMemo<DisplayRow[]>(() => (report?.details.items ?? []).map(row => ({ key: `${row.origin}:${row.id}`, values: detailValues(row, workloadDetailPersonLabel(row, report?.details.items ?? [])) })), [report]);
  const summaryRowValues = useMemo(() => Object.fromEntries(summaryRows.map(row => [row.key, row.values])), [summaryRows]);
  const detailRowValues = useMemo(() => Object.fromEntries(detailRows.map(row => [row.key, row.values])), [detailRows]);
  const summaryStyle = useRecordTableStyle('workload_type_summary', summaryRowValues);
  const detailStyle = useRecordTableStyle('workload_type_details', detailRowValues);
  const activeStyle = detailsActive ? detailStyle : summaryStyle;
  const mobile = activeStyle.spec.layout.mode === 'card' || (activeStyle.spec.layout.mode === 'auto' && narrow);
  const pageSize = detailStyle.spec.paging.size || 25;
  const query = useMemo(() => workloadDrillQuery(JSON.parse(baseKey), path, page, pageSize), [baseKey, path, page, pageSize]);
  useEffect(() => { setPage(0); }, [baseKey, pageSize]);
  useEffect(() => {
    let active = true;
    setLoading(true); setError(''); setReport(null);
    if (!query.start || !query.end || query.start > query.end) { setError('请选择有效日期范围'); setLoading(false); return; }
    getWorkloadReport(query).then(response => {
      if (!active) return;
      if (response.code !== 0 || !response.data) throw new Error(response.message || '类型汇总加载失败');
      setReport(response.data);
    }).catch(reason => { if (active) setError(reason?.message || '类型汇总加载失败'); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [query, revision]);
  const drill = (row: WorkloadBreakdown) => {
    if (!dimension) return;
    setPath(current => [...current, { dimension, key: row.key, name: summaryRows[rows.indexOf(row)]?.values.name ?? historicalName(row) }]);
    setShowDetails(false); setPage(0);
  };
  const backTo = (length: number) => { setPath(current => current.slice(0, length)); setShowDetails(false); setPage(0); };
  const exportCurrent = async () => {
    setExporting(true); setError('');
    try { await exportWorkloadReport({ ...query, compact_type_summary: undefined }); }
    catch (reason: any) { setError(reason?.message || '导出失败'); }
    finally { setExporting(false); }
  };
  const table = (columns: typeof summaryColumns, data: DisplayRow[], style: RecordTableStyleApi, select?: (index: number) => void) => {
    const visible = columns.filter(column => !style.spec.hiddenColumns.includes(column.key));
    const widths = getAdaptiveColumnWidths(data, visible.map(column => ({ key: column.key, header: column.label, fixed: style.spec.columns[column.key]?.width ?? column.width, getValue: row => row.values[column.key] })));
    return visible.length ? <Table size="small" sx={{ ...style.tableSx, width: '100%', minWidth: 0, tableLayout: 'fixed' }} aria-label={detailsActive ? '类型工作量来源明细' : '类型工作量分类汇总'}>
      <colgroup>{visible.map(column => <col key={column.key} style={{ width: widths[column.key].desktop }} />)}</colgroup>
      <TableHead><TableRow>{visible.map(column => <TableCell key={column.key} sx={style.resolver.headerSx(column.key)}><Box sx={{ ...scrollCellSx, ...style.resolver.headerFlowSx(column.key) }}>{column.key === 'name' ? dimensionNames[dimension ?? 'type'] : column.label}</Box></TableCell>)}</TableRow></TableHead>
      <TableBody>{data.length ? data.map((row, index) => <TableRow key={row.key} hover>{visible.map(column => <TableCell key={column.key} sx={style.resolver.cellSx(column.key, row.key, index)}><Box sx={{ ...scrollCellSx, ...style.resolver.flowSx(column.key, row.key, index) }}>{select && column.key === 'name' ? <Button size="small" onClick={() => select(index)} sx={{ minWidth: 0, p: 0, font: 'inherit', color: 'inherit', textAlign: 'inherit', justifyContent: 'flex-start', overflowWrap: 'anywhere' }}>{row.values[column.key]}</Button> : row.values[column.key] || '—'}</Box></TableCell>)}</TableRow>) : <TableRow><TableCell colSpan={visible.length}>当前筛选没有工作量记录</TableCell></TableRow>}</TableBody>
    </Table> : <Alert severity="info">全部列已隐藏，请在“表格样式 → 列显示”恢复需要的列。</Alert>;
  };
  const cards = (columns: typeof summaryColumns, data: DisplayRow[], style: RecordTableStyleApi, defaults: string[][], select?: (index: number) => void) => {
    const visible = columns.filter(column => !style.spec.hiddenColumns.includes(column.key));
    if (!visible.length) return <Alert severity="info">全部列已隐藏，请在“表格样式 → 列显示”恢复需要的列。</Alert>;
    return <Box sx={{ display: 'grid', gap: 0.75 }}>{data.map((row, index) => <Accordion key={row.key} disableGutters elevation={0} sx={{ ...cardContainerSx(style.spec), '&:before': { display: 'none' } }}>
      <AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ px: 1, minHeight: 48, '& .MuiAccordionSummary-content': { my: 0.5, minWidth: 0 } }}>
        <Box sx={{ minWidth: 0, width: '100%', ...style.resolver.rowSx(row.key) }}><RecordCardSummary card={style.spec.card} defaultRows={defaults} auxFontSize={style.spec.table.auxFontSize} fields={visible.map(column => ({ key: column.key, label: column.key === 'name' ? dimensionNames[dimension ?? 'type'] : column.label, value: <Box component="span" sx={{ ...style.resolver.visualSx(column.key, row.key, index), whiteSpace: 'pre-line', overflowWrap: 'anywhere' }}>{row.values[column.key] || '—'}</Box> }))} /></Box>
      </AccordionSummary>
      <AccordionDetails sx={{ p: 1, pt: 0 }}>{select ? <Button size="small" variant="outlined" fullWidth onClick={() => select(index)}>查看下级汇总与明细</Button> : <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 0.75 }}>{visible.map(column => <Box key={column.key} sx={{ minWidth: 0, p: 0.75, ...cardFieldSx(style.spec), ...style.resolver.visualSx(column.key, row.key, index) }}><Typography variant="caption" color="text.secondary">{column.label}</Typography><Box sx={{ ...scrollCellSx, ...style.resolver.flowSx(column.key, row.key, index) }}>{row.values[column.key] || '—'}</Box></Box>)}</Box>}</AccordionDetails>
    </Accordion>)}{!data.length && <Typography variant="body2">当前筛选没有工作量记录</Typography>}</Box>;
  };
  return <Paper variant="outlined" sx={{ p: { xs: 1, sm: 1.5 }, minWidth: 0, borderRadius: '2px' }}>
    <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 1, mb: 1 }}>
      <Typography variant="subtitle1" fontWeight={700} sx={{ flex: 1 }}>类型汇总</Typography>
      <WorkloadAuxiliaryConfig onSaved={() => { if (path[0]?.key === '辅助工作') setPath(current => current.slice(0, 1)); setPage(0); setShowDetails(false); setRevision(value => value + 1); }} />
      <TableStyleBar api={activeStyle} columns={detailsActive ? detailColumns : summaryColumns} rows={(detailsActive ? detailRows : summaryRows).map(row => ({ key: row.key, label: row.values.person || row.values.name }))} cardLayout={mobile} hasCardLayout summaryDefaultRows={detailsActive ? detailDefaultRows : summaryDefaultRows} dateColumns={[]} onPageSizeChange={detailsActive ? () => setPage(0) : undefined} />
      {canExport && <Button variant="outlined" size="small" disabled={loading || exporting || !report} onClick={exportCurrent}>{exporting ? '正在导出' : '导出当前筛选'}</Button>}
    </Box>
    <Breadcrumbs aria-label="类型汇总分类路径" sx={{ mb: 1, '& .MuiBreadcrumbs-li': { minWidth: 0 }, '& .MuiBreadcrumbs-ol': { flexWrap: 'wrap' } }}>
      <Button size="small" onClick={() => backTo(0)}>全部类型</Button>
      {path.map((step, index) => <Button key={`${step.dimension}:${step.key}`} size="small" sx={{ textAlign: 'left', overflowWrap: 'anywhere' }} onClick={() => backTo(index + 1)}>{step.name}</Button>)}
      {detailsActive && <Typography variant="body2">来源明细</Typography>}
    </Breadcrumbs>
    <Typography variant="caption" color="text.secondary" component="p" sx={{ mb: 1 }}>沿用当前日期、人员和部门筛选。数量为录入数量之和，工作量按系数快照累计；占比以当前层工作量为基准。辅助工作独立分类，不分摊到检测仪器或实验室。新增记录优先使用分类快照；历史缺少分类快照时按当前配置兼容展示，配置调整可能影响这部分历史分类。</Typography>
    {path.length === 1 && <ToggleButtonGroup size="small" exclusive value={detailsActive ? 'details' : dimension} onChange={(_, value) => {
      if (!value) return;
      setShowDetails(value === 'details'); if (value !== 'details') setPreferred(value); setPage(0);
    }} aria-label="类型下级汇总方式" sx={{ mb: 1, display: 'flex', flexWrap: 'wrap', '& .MuiToggleButton-root': { fontSize: 12, px: 1 } }}>
      {path[0].key === '辅助工作' ? [<ToggleButton key="category" value="auxiliary_category">按辅助类型</ToggleButton>, <ToggleButton key="method" value="auxiliary_method">按方法 / 事项</ToggleButton>] : [<ToggleButton key="method" value="method">按方法</ToggleButton>, <ToggleButton key="instrument" value="instrument">按仪器</ToggleButton>]}
      <ToggleButton value="details">来源明细</ToggleButton>
    </ToggleButtonGroup>}
    {path.length !== 1 && dimension && <Button className={detailsActive ? 'labflow-back-button' : undefined} size="small" onClick={() => { setShowDetails(value => !value); setPage(0); }} sx={{ mb: 1 }}>{detailsActive ? `返回${dimensionNames[dimension]}汇总` : '查看当前范围明细'}</Button>}
    {error && <Alert severity="error" sx={{ mb: 1 }} action={<Button size="small" onClick={() => setRevision(value => value + 1)}>重试</Button>}>{error}</Alert>}
    {loading && <Box sx={{ textAlign: 'center', py: 3 }}><CircularProgress size={26} /><Typography variant="body2">正在加载类型汇总</Typography></Box>}
    {report && <>
      <Box sx={{ mb: 1, display: 'flex', flexWrap: 'wrap', gap: 1.5 }}><Typography variant="body2">数量 <b>{number(report.summary.detection_quantity + report.summary.auxiliary_quantity)}</b></Typography><Typography variant="body2">工作量 <b>{number(report.summary.total_workload)}</b></Typography><Typography variant="body2">记录 <b>{report.summary.record_count}</b> 条</Typography></Box>
      {!detailsActive ? mobile ? cards(summaryColumns, summaryRows, summaryStyle, summaryDefaultRows, index => drill(rows[index])) : table(summaryColumns, summaryRows, summaryStyle, index => drill(rows[index])) : <>
        {mobile ? cards(detailColumns, detailRows, detailStyle, detailDefaultRows) : table(detailColumns, detailRows, detailStyle)}
        <TablePagination component="div" count={report.details.total} page={page} rowsPerPage={pageSize} rowsPerPageOptions={[]} onPageChange={(_, value) => setPage(value)} labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} getItemAriaLabel={type => type === 'next' ? '下一页明细' : '上一页明细'} sx={{ '& .MuiTablePagination-toolbar': { px: 0, minHeight: 44 }, '& .MuiTablePagination-displayedRows': { mx: 1 } }} />
      </>}
    </>}
  </Paper>;
}

function detailValues(row: WorkloadDetail, personLabel: string): Record<string, string> {
  return {
    person: `${personLabel}\n${dayjs(row.recorded_at).format('YYYY-MM-DD HH:mm')}${row.ownership_status === 'pending_confirmation' ? '\n待确认归属' : ''}`,
    type: `${row.type}\n${sourceNames[row.source] ?? row.source}`,
    department: `${row.department || '—'}\n${row.lab || '公共工作'}`,
    method: row.kind === 'auxiliary' ? `${row.auxiliary_category || '未分类'}\n${row.method || '—'}` : [row.project, row.method, row.instrument].filter(Boolean).join('\n') || '—',
    quantity: number(row.quantity), coefficient: coefficient(row.coefficient), workload: number(row.workload),
  };
}
