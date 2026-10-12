import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Accordion, AccordionDetails, AccordionSummary, Alert, Box, Button, Chip, CircularProgress, FormControlLabel, Checkbox, MenuItem, Paper, Table, TableBody, TableCell, TableHead, TableRow, TablePagination, Tabs, Tab, TextField, Typography, useMediaQuery, useTheme } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import DownloadIcon from '@mui/icons-material/Download';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import { CartesianGrid, Legend, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import dayjs from 'dayjs';
import { useUser } from '../UserContext';
import { hasPermission } from '../constants/permissions';
import DateRangePicker from '../components/DateRangePicker';
import { exportWorkloadReport, getWorkloadReport, type WorkloadBreakdown, type WorkloadDetail, type WorkloadPerson, type WorkloadReport, type WorkloadReportQuery } from '../api/workloadReport';
import { buildWorkloadMatrix, workloadPersonKey } from '../utils/workloadMatrix';

const dimensions = [['type', '检测类型'], ['department', '部门'], ['lab', '实验室'], ['project', '项目'], ['method', '方法'], ['instrument', '仪器'], ['source', '来源']] as const;
const sourceNames: Record<string, string> = { analysis: '分析检测', rd_sample: '研发送样取样录入', sample_info_sample: '样品信息登记取样录入', auxiliary_work: '辅助工作' };
const dimensionParams: Record<string, keyof WorkloadReportQuery> = { department: 'division_id', lab: 'group_id', project: 'project_id', method: 'method_id', instrument: 'instrument_id', type: 'type', source: 'source' };
const number = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 2 });
const coefficientNumber = (value: number) => value.toLocaleString('zh-CN', { maximumFractionDigits: 12 });
const tableSx = { width: '100%', tableLayout: 'fixed', '& th, & td': { p: 1, verticalAlign: 'top', overflowWrap: 'anywhere', border: '1px solid', borderColor: 'divider' } } as const;
const cell = (value: React.ReactNode) => <Box sx={{ maxHeight: 120, overflow: 'auto', minWidth: 0, overflowWrap: 'anywhere' }}>{value}</Box>;

const WorkloadReportPage: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { user } = useUser();
  const mobile = useMediaQuery(useTheme().breakpoints.down('sm'));
  const canScope = !!(user?.is_admin || ['records:work:view-scope', 'records:work:view-all', 'stats:workload:view-all'].some(key => hasPermission(user?.permissions ?? [], key)));
  const canExport = !!(user?.is_admin || hasPermission(user?.permissions ?? [], 'stats:workload:export'));
  const requestedSubject = Number(searchParams.get('subject_user_id'));
  const initialSubject = canScope && requestedSubject > 0 ? requestedSubject : undefined;
  const [view, setView] = useState<'mine' | 'scope'>(initialSubject ? 'scope' : 'mine');
  const [subject, setSubject] = useState<number | undefined>(initialSubject);
  const [start, setStart] = useState(() => dayjs().startOf('month').format('YYYY-MM-DD'));
  const [end, setEnd] = useState(() => dayjs().format('YYYY-MM-DD'));
  const [period, setPeriod] = useState<'day' | 'week' | 'month'>('day');
  const [pending, setPending] = useState(false);
  const [zero, setZero] = useState(false);
  const [dimension, setDimension] = useState('type');
  const [filters, setFilters] = useState<Record<string, { value: string | number; label: string }>>({});
  const [page, setPage] = useState(0);
  const [report, setReport] = useState<WorkloadReport | null>(null);
  const [peopleOptions, setPeopleOptions] = useState<WorkloadPerson[]>([]);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState('');
  const sequence = useRef(0);
  const query = useMemo<WorkloadReportQuery>(() => ({ start, end, view, subject_user_id: view === 'scope' ? subject : undefined, group_by: period, include_pending_ownership: pending, include_zero_users: zero, page: page + 1, page_size: 25, ...Object.fromEntries(Object.entries(filters).map(([key, value]) => [key, value.value])) }), [start, end, view, subject, period, pending, zero, page, filters]);
  useEffect(() => {
    const seq = ++sequence.current;
    let active = true;
    setLoading(true); setError(''); setReport(null);
    if (!start || !end || start > end) { setError('请选择有效日期范围，开始日期不能晚于结束日期'); setLoading(false); return; }
    getWorkloadReport(query).then(response => {
      if (!active || seq !== sequence.current) return;
      if (response.code !== 0 || !response.data) throw new Error(response.message || '统计加载失败');
      setReport(response.data);
      setPeopleOptions(current => {
        const options = new Map(current.map(person => [workloadPersonKey(person), person]));
        response.data!.people.forEach(person => options.set(workloadPersonKey(person), person));
        return [...options.values()];
      });
    }).catch(reason => { if (active && seq === sequence.current) setError(reason?.message || '统计加载失败'); })
      .finally(() => { if (active && seq === sequence.current) setLoading(false); });
    return () => { active = false; };
  }, [query]);
  useEffect(() => { setPeopleOptions([]); }, [view, start, end]);
  const matrix = useMemo(() => buildWorkloadMatrix(report?.people ?? [], report?.matrix ?? []), [report]);
  const personLabel = (person: { user_id: number | null; user_name: string }) => {
    if (person.user_id === null) return `${person.user_name}（历史未绑定）`;
    const ids = [...new Set([...peopleOptions, ...(report?.people ?? [])].filter(item => item.user_name === person.user_name && item.user_id !== null).map(item => item.user_id!))].sort((a, b) => a - b);
    return ids.length > 1 ? `${person.user_name}（同名账号 ${ids.indexOf(person.user_id) + 1}）` : person.user_name;
  };
  const drill = (key: string, row: WorkloadBreakdown) => {
    const parameter = dimensionParams[key];
    const numeric = key !== 'source' && key !== 'type';
    if (numeric && !/^\d+$/.test(row.key)) return;
    setFilters(current => ({ ...current, [parameter]: { value: numeric ? Number(row.key) : row.key, label: `${dimensions.find(item => item[0] === key)?.[1]}：${sourceNames[row.name] ?? row.name}` } }));
    setPage(0);
  };
  const drillPerson = (person: WorkloadPerson) => { if (canScope && view === 'scope' && person.user_id !== null) { setSubject(person.user_id); setPage(0); } };
  const exportCurrent = async () => {
    setExporting(true); setError('');
    try { await exportWorkloadReport(query); } catch (reason: any) { setError(reason?.message || '导出失败'); } finally { setExporting(false); }
  };
  const reset = () => { setFilters({}); setSubject(undefined); setPage(0); };
  const sectionTitle = (title: string, extra?: React.ReactNode) => <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 1, mb: 1.5 }}><Typography variant="h6" fontWeight={700}>{title}</Typography>{extra}</Box>;
  const detailFields = (row: WorkloadDetail) => [['人员', personLabel(row)], ['日期', dayjs(row.recorded_at).format('YYYY-MM-DD HH:mm:ss')], ['类型', row.type], ['部门', row.department], ['实验室', row.lab || '公共工作'], ['项目', row.project || '—'], ['方法 / 辅助项', row.method || '—'], ['仪器', row.instrument || '—'], ['来源', sourceNames[row.source] ?? row.source], ['数量 × 系数', `${row.quantity} × ${coefficientNumber(row.coefficient)}`], ['工作量', number(row.workload)], ['归属', row.ownership_status === 'pending_confirmation' ? '待确认' : '已确认']];

  return <Box sx={{ minWidth: 0, width: '100%', '& .MuiPaper-root': { borderRadius: '2px' } }}>
    <Box sx={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 1, mb: 2 }}>
      <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/stats')}>返回专业报表</Button>
      <Typography variant="h5" fontWeight={700} sx={{ flex: { xs: '0 0 100%', sm: 1 }, order: { xs: -1, sm: 0 } }}>工作量汇总</Typography>
      {canExport && <Button variant="outlined" startIcon={<DownloadIcon />} disabled={loading || exporting || !report} onClick={exportCurrent}>{exporting ? '正在导出' : '导出当前筛选'}</Button>}
    </Box>
    <Tabs value={view} onChange={(_, value) => { setView(value); reset(); }} aria-label="工作量查看范围" sx={{ mb: 1 }}>
      <Tab value="mine" label="我的工作量" />{canScope && <Tab value="scope" label="授权范围汇总" />}
    </Tabs>
    {!canScope && requestedSubject > 0 && requestedSubject !== user?.id && <Alert severity="warning" sx={{ mb: 2 }}>当前账号只能查看本人工作量，所选人员统计需要授权范围权限</Alert>}
    <Alert severity="info" sx={{ mb: 2 }}>
      {view === 'mine' ? `当前查看本人（${user?.username ?? ''}）的工作量` : user?.is_admin ? '当前查看全局授权范围内的人员工作量' : '当前查看角色授权部门内的人员工作量'}，同时遵循账号部门权限。
    </Alert>
    <Accordion elevation={0} sx={{ mb: 2, '&:before': { display: 'none' } }}><AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ px: 1, minHeight: 36 }}><Typography variant="body2" color="text.secondary">统计口径与历史记录说明</Typography></AccordionSummary><AccordionDetails sx={{ pt: 0 }}><Typography variant="body2" color="text.secondary">工作量 = 数量 × 系数快照。辅助工作为人员公共工作量，不分摊实验室、项目、方法和仪器；这些分类筛选仍保留已授权的公共辅助量。历史未归属部门的辅助记录仅本人或管理员可见。历史未绑定人员仅展示，不按同名账号自动归属。</Typography></AccordionDetails></Accordion>
    <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mb: 2 }}>
      <DateRangePicker startDate={start} endDate={end} onStartChange={value => { setStart(value); setPage(0); }} onEndChange={value => { setEnd(value); setPage(0); }} />
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1.5, alignItems: 'center', mt: 2 }}>
        {view === 'scope' && <TextField select size="small" label="汇总人员" value={subject ?? ''} sx={{ minWidth: 180, maxWidth: '100%' }} onChange={event => { setSubject(event.target.value ? Number(event.target.value) : undefined); setPage(0); }}>
          <MenuItem value="">全部授权人员</MenuItem>
          {subject && !peopleOptions.some(person => person.user_id === subject) && <MenuItem value={subject}>所选人员</MenuItem>}
          {peopleOptions.filter(person => person.user_id !== null).map(person => <MenuItem key={person.user_id} value={person.user_id!}>{personLabel(person)}</MenuItem>)}
        </TextField>}
        <TextField select size="small" label="趋势周期" value={period} onChange={event => setPeriod(event.target.value as typeof period)} sx={{ minWidth: 110 }}><MenuItem value="day">按日</MenuItem><MenuItem value="week">按周</MenuItem><MenuItem value="month">按月</MenuItem></TextField>
        <FormControlLabel control={<Checkbox checked={pending} onChange={event => { setPending(event.target.checked); setPage(0); }} />} label="包括待确认归属" />
        {view === 'scope' && <FormControlLabel control={<Checkbox checked={zero} onChange={event => setZero(event.target.checked)} />} label="显示零工作量人员" />}
        <Button onClick={reset}>清除分类与人员筛选</Button>
      </Box>
      {Object.keys(filters).length > 0 && <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 1 }}>{Object.entries(filters).map(([key, value]) => <Chip key={key} label={value.label} onDelete={() => { setFilters(current => { const next = { ...current }; delete next[key]; return next; }); setPage(0); }} />)}</Box>}
    </Paper>
    {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}
    {loading && <Box sx={{ p: 4, textAlign: 'center' }}><CircularProgress size={30} /><Typography>正在加载工作量汇总</Typography></Box>}
    {report && <>
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2,minmax(0,1fr))', md: 'repeat(4,minmax(0,1fr))' }, gap: 1.5, mb: 2 }}>
        {([['检测数量', report.summary.detection_quantity], ['检测工作量', report.summary.detection_workload], ['辅助工作量', report.summary.auxiliary_workload], ['总工作量', report.summary.total_workload]] as const).map(([label, value]) => <Paper key={label} variant="outlined" sx={{ p: 1.5, minWidth: 0 }}><Typography variant="body2" color="text.secondary">{label}</Typography><Typography variant="h5" fontWeight={700} sx={{ overflowWrap: 'anywhere' }}>{number(value)}</Typography></Paper>)}
      </Box>
      <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mb: 2, minWidth: 0 }}>
        {sectionTitle('分类汇总', <TextField select label="分类维度" size="small" value={dimension} onChange={event => setDimension(event.target.value)} sx={{ minWidth: 120 }}>{dimensions.map(([key, name]) => <MenuItem key={key} value={key}>{name}</MenuItem>)}</TextField>)}
        <Typography variant="caption" color="text.secondary">点击分类可筛选汇总及来源明细。部门、实验室、项目、方法、仪器分类仅包含检测工作量。</Typography>
        <Table size="small" sx={{ ...tableSx, mt: 1 }} aria-label="分类工作量汇总"><TableHead><TableRow><TableCell sx={{ width: '45%' }}>分类</TableCell><TableCell align="right">数量</TableCell><TableCell align="right">工作量</TableCell></TableRow></TableHead><TableBody>
          {(report.breakdowns[dimension] ?? []).map(row => <TableRow key={row.key} hover><TableCell>{['type', 'source'].includes(dimension) || /^\d+$/.test(row.key) ? <Button size="small" onClick={() => drill(dimension, row)} sx={{ justifyContent: 'flex-start', textAlign: 'left', p: 0, minWidth: 0, overflowWrap: 'anywhere' }}>{sourceNames[row.name] ?? row.name}</Button> : cell(`${row.name}（历史未绑定）`)}</TableCell><TableCell align="right">{number(row.detection_quantity + row.auxiliary_quantity)}</TableCell><TableCell align="right">{number(row.total_workload)}</TableCell></TableRow>)}
          {!(report.breakdowns[dimension]?.length) && <TableRow><TableCell colSpan={3}>当前范围无此类工作量</TableCell></TableRow>}
        </TableBody></Table>
      </Paper>
      <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mb: 2, minWidth: 0 }}>
        {sectionTitle('人员工作量汇总')}
        {mobile ? report.people.map(person => <Accordion key={workloadPersonKey(person)} disableGutters elevation={0} sx={{ borderBottom: '1px solid', borderColor: 'divider', '&:before': { display: 'none' } }}><AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ px: 0, minHeight: 48, '& .MuiAccordionSummary-content': { gap: 1, alignItems: 'center', minWidth: 0 } }}><Typography sx={{ flex: 1, overflowWrap: 'anywhere' }}>{personLabel(person)}</Typography><Typography fontWeight={700}>{number(person.total_workload)}</Typography></AccordionSummary><AccordionDetails sx={{ p: 1, pt: 0 }}><Typography variant="body2">检测 {number(person.detection_workload)} · 辅助 {number(person.auxiliary_workload)} · 检测数量 {number(person.detection_quantity)}</Typography>{canScope && view === 'scope' && person.user_id !== null && <Button size="small" onClick={() => drillPerson(person)}>查看此人分类与明细</Button>}</AccordionDetails></Accordion>) : <Table size="small" sx={tableSx} aria-label="人员工作量汇总"><TableHead><TableRow>{['人员', '检测数量', '检测工作量', '辅助工作量', '总工作量'].map(title => <TableCell key={title}>{title}</TableCell>)}</TableRow></TableHead><TableBody>{report.people.map(person => <TableRow key={workloadPersonKey(person)}><TableCell>{person.user_id !== null && view === 'scope' ? <Button size="small" onClick={() => drillPerson(person)} sx={{ p: 0, textAlign: 'left', minWidth: 0 }}>{cell(personLabel(person))}</Button> : cell(personLabel(person))}</TableCell>{[person.detection_quantity, person.detection_workload, person.auxiliary_workload, person.total_workload].map((value, index) => <TableCell align="right" key={index}>{number(value)}</TableCell>)}</TableRow>)}<TableRow><TableCell>合计</TableCell>{[report.summary.detection_quantity, report.summary.detection_workload, report.summary.auxiliary_workload, report.summary.total_workload].map((value, index) => <TableCell align="right" key={index}>{number(value)}</TableCell>)}</TableRow></TableBody></Table>}
        {!report.people.length && <Typography color="text.secondary">当前范围无人员工作量</Typography>}
      </Paper>
      <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mb: 2, minWidth: 0 }}>
        {sectionTitle('人员 × 检测类型矩阵')}
        {mobile ? <><Typography variant="caption" color="text.secondary">展开人员查看各类型工作量，辅助工作单独列出。</Typography>{matrix.rows.map(person => <Accordion key={person.key} disableGutters elevation={0} sx={{ borderBottom: '1px solid', borderColor: 'divider', '&:before': { display: 'none' } }}><AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ px: 0, '& .MuiAccordionSummary-content': { justifyContent: 'space-between', gap: 1 } }}><Typography sx={{ overflowWrap: 'anywhere' }}>{personLabel(person)}</Typography><Typography fontWeight={700}>{number(person.total_workload)}</Typography></AccordionSummary><AccordionDetails sx={{ p: 1, pt: 0 }}>{matrix.types.map(type => <Box key={type} sx={{ display: 'flex', justifyContent: 'space-between', gap: 1, py: 0.5, borderBottom: '1px solid', borderColor: 'divider' }}><Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{type}</Typography><Typography variant="body2">{number(person.cells.get(type) ?? 0)}</Typography></Box>)}</AccordionDetails></Accordion>)}<Box sx={{ mt: 1 }}><Typography fontWeight={700}>列合计 · {number(matrix.total)}</Typography>{matrix.types.map(type => <Typography variant="body2" key={type}>{type}：{number(matrix.columns.get(type) ?? 0)}</Typography>)}</Box></> : <Table size="small" sx={tableSx} aria-label="人员类型矩阵"><TableHead><TableRow><TableCell>人员</TableCell>{matrix.types.map(type => <TableCell key={type}>{cell(type)}</TableCell>)}<TableCell>行合计</TableCell></TableRow></TableHead><TableBody>{matrix.rows.map(person => <TableRow key={person.key}><TableCell>{cell(personLabel(person))}</TableCell>{matrix.types.map(type => <TableCell align="right" key={type}>{cell(number(person.cells.get(type) ?? 0))}</TableCell>)}<TableCell align="right">{cell(number(person.total_workload))}</TableCell></TableRow>)}<TableRow><TableCell>列合计</TableCell>{matrix.types.map(type => <TableCell align="right" key={type}>{cell(number(matrix.columns.get(type) ?? 0))}</TableCell>)}<TableCell align="right">{cell(number(matrix.total))}</TableCell></TableRow></TableBody></Table>}
      </Paper>
      <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, mb: 2, minWidth: 0 }}>
        {sectionTitle('工作量趋势')}
        {report.trend.length ? <Box sx={{ width: '100%', height: mobile ? 230 : 280, minWidth: 0 }}><ResponsiveContainer width="100%" height="100%"><LineChart data={report.trend} margin={{ top: 10, right: 10, bottom: 0, left: 0 }}><CartesianGrid strokeDasharray="3 3" /><XAxis dataKey="period" fontSize={11} minTickGap={35} /><YAxis width={44} fontSize={11} /><Tooltip /><Legend /><Line dataKey="detection_workload" name="检测工作量" stroke="#1976d2" dot={false} /><Line dataKey="auxiliary_workload" name="辅助工作量" stroke="#ed6c02" dot={false} /><Line dataKey="total_workload" name="总工作量" stroke="#2e7d32" dot={false} /></LineChart></ResponsiveContainer></Box> : <Typography color="text.secondary">当前范围暂无趋势数据</Typography>}
      </Paper>
      <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, minWidth: 0 }}>
        {sectionTitle(`来源明细 · ${report.details.total} 条`)}
        {mobile ? report.details.items.map(row => <Accordion key={`${row.kind}:${row.id}`} disableGutters elevation={0} sx={{ borderBottom: '1px solid', borderColor: 'divider', '&:before': { display: 'none' } }}><AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ px: 0, '& .MuiAccordionSummary-content': { minWidth: 0, display: 'block' } }}><Box sx={{ display: 'flex', justifyContent: 'space-between', gap: 1 }}><Typography fontWeight={600} sx={{ overflowWrap: 'anywhere' }}>{personLabel(row)} · {row.type}</Typography><Typography fontWeight={700}>{number(row.workload)}</Typography></Box><Typography variant="caption" color="text.secondary">{dayjs(row.recorded_at).format('MM-DD HH:mm')} · {row.kind === 'auxiliary' ? row.method : row.project} · 数量 {row.quantity}</Typography></AccordionSummary><AccordionDetails sx={{ px: 0, py: 1 }}><Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 0.75 }}>{detailFields(row).map(([label, value]) => <Box key={label} sx={{ p: 0.75, border: '1px solid', borderColor: 'divider', minWidth: 0 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{value}</Typography></Box>)}</Box></AccordionDetails></Accordion>) : <Table size="small" sx={tableSx} aria-label="工作量来源明细"><TableHead><TableRow>{['人员 / 日期', '类型 / 来源', '部门 / 实验室', '项目 / 方法 / 仪器', '数量', '系数', '工作量'].map(title => <TableCell key={title}>{title}</TableCell>)}</TableRow></TableHead><TableBody>{report.details.items.map(row => <TableRow key={`${row.kind}:${row.id}`}><TableCell>{cell(<>{personLabel(row)}<br />{dayjs(row.recorded_at).format('YYYY-MM-DD HH:mm')}{row.ownership_status === 'pending_confirmation' && <Chip label="待确认" size="small" color="warning" />}</>)}</TableCell><TableCell>{cell(<>{row.type}<br />{sourceNames[row.source] ?? row.source}</>)}</TableCell><TableCell>{cell(<>{row.department}<br />{row.lab || '公共工作'}</>)}</TableCell><TableCell>{cell(<>{row.project}<br />{row.method}<br />{row.instrument}</>)}</TableCell><TableCell align="right">{row.quantity}</TableCell><TableCell align="right">{coefficientNumber(row.coefficient)}</TableCell><TableCell align="right">{number(row.workload)}</TableCell></TableRow>)}</TableBody></Table>}
        {!report.details.total && <Typography color="text.secondary">当前筛选没有工作量记录</Typography>}
        <TablePagination component="div" count={report.details.total} page={page} rowsPerPage={25} rowsPerPageOptions={[]} onPageChange={(_, value) => setPage(value)} labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} getItemAriaLabel={type => type === 'next' ? '下一页明细' : '上一页明细'} sx={{ '& .MuiTablePagination-toolbar': { px: 0, minHeight: 48 }, '& .MuiTablePagination-spacer': { flex: '1 1 0' }, '& .MuiTablePagination-displayedRows': { mx: 1 } }} />
      </Paper>
    </>}
  </Box>;
};
export default WorkloadReportPage;
