import React, { useEffect, useMemo, useState } from 'react';
import {
  Box, Button, Divider, FormControlLabel, IconButton, InputAdornment, Paper,
  Stack, Switch, TextField, Tooltip, Typography,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import GridOnIcon from '@mui/icons-material/GridOn';
import LockIcon from '@mui/icons-material/Lock';
import SaveIcon from '@mui/icons-material/Save';
import TableRowsIcon from '@mui/icons-material/TableRows';
import type { ExportTemplate, SheetConfig } from './exportTemplateDefaults';

type Props = {
  template: ExportTemplate;
  onChange: (template: ExportTemplate) => void;
  onSave: () => void;
  saving?: boolean;
};

const formulaColumns = new Set([
  'workload', 'total_workload', 'detail_amount', 'project_total', 'lab_total',
  'total_qty', 'daily_total', 'type_total', 'amount', 'total_amount',
]);

const formulaDescription = (key: string) => {
  if (key.includes('workload')) return '= 数量 x 系数';
  if (key.includes('amount')) return '= 数量 x 单价 x 单价倍率';
  return '= SUM(当前分组明细)';
};

const columnSample = (key: string, index: number) => {
  if (key.includes('date') || key.includes('month')) return '2026-08-01';
  if (key.includes('lab')) return '质量控制室';
  if (key.includes('project')) return 'A001';
  if (key.includes('user')) return '张东丽';
  if (key.includes('method')) return '液相';
  if (key.includes('instrument')) return 'LC-01';
  if (key.includes('quantity')) return '18';
  if (key.includes('coefficient') || key.includes('price') || key.includes('multiplier')) return '1';
  if (formulaColumns.has(key)) return index % 2 ? '=SUM(...)' : '=数量 x 系数';
  return '-';
};

const SpreadsheetTemplateEditor: React.FC<Props> = ({ template, onChange, onSave, saving = false }) => {
  const sheetEntries = useMemo(() => Object.entries(template.sheets), [template.sheets]);
  const [activeSheetId, setActiveSheetId] = useState(() => sheetEntries[0]?.[0] || '');
  const activeSheet = template.sheets[activeSheetId];
  const [activeColumnKey, setActiveColumnKey] = useState('');
  const columns = useMemo(() => activeSheet ? Object.entries(activeSheet.columns) : [], [activeSheet]);
  const selectedKey = activeColumnKey && activeSheet?.columns[activeColumnKey]
    ? activeColumnKey : columns[0]?.[0] || '';
  const selectedColumn = activeSheet?.columns[selectedKey];

  useEffect(() => {
    if (!template.sheets[activeSheetId]) {
      setActiveSheetId(sheetEntries[0]?.[0] || '');
      setActiveColumnKey('');
    }
  }, [activeSheetId, sheetEntries, template.sheets]);

  const updateSheet = (sheetId: string, update: Partial<SheetConfig>) => {
    onChange({ ...template, sheets: { ...template.sheets, [sheetId]: { ...template.sheets[sheetId], ...update } } });
  };

  const updateColumn = (columnKey: string, update: Record<string, unknown>) => {
    if (!activeSheet) return;
    updateSheet(activeSheetId, {
      columns: {
        ...activeSheet.columns,
        [columnKey]: { ...activeSheet.columns[columnKey], ...update },
      },
    });
  };

  const selectSheet = (sheetId: string) => {
    setActiveSheetId(sheetId);
    setActiveColumnKey('');
  };

  if (!activeSheet) return null;
  const activeColumns = columns.filter(([, column]) => column.visible !== false);
  const isFormula = formulaColumns.has(selectedKey);

  return (
    <Box sx={{ border: '1px solid #d7e0e5', bgcolor: '#edf2f5', minWidth: 0 }}>
      <Box sx={{ display: 'grid', gridTemplateColumns: '220px minmax(460px, 1fr) 286px', minHeight: 610 }}>
        <Box sx={{ bgcolor: '#fff', borderRight: '1px solid #d7e0e5', p: 1.25 }}>
          <Typography variant="caption" sx={{ color: '#627483', fontWeight: 700, px: .75 }}>工作表</Typography>
          <Stack spacing={.35} sx={{ mt: 1 }}>
            {sheetEntries.map(([sheetId, sheet]) => (
              <Button key={sheetId} onClick={() => selectSheet(sheetId)} variant="text"
                sx={{ justifyContent: 'flex-start', minHeight: 38, px: .8, color: sheetId === activeSheetId ? '#08624c' : '#405360', bgcolor: sheetId === activeSheetId ? '#e2f3ed' : 'transparent', borderRadius: '3px', textTransform: 'none', fontWeight: sheetId === activeSheetId ? 700 : 400 }}>
                <Box sx={{ width: 9, height: 9, bgcolor: sheet.color || '#94a3b8', borderRadius: '2px', mr: 1 }} />
                <Box sx={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', textAlign: 'left' }}>{sheet.title}</Box>
                <Typography variant="caption" sx={{ ml: .75, color: '#80909c' }}>{Object.keys(sheet.columns).length}</Typography>
              </Button>
            ))}
          </Stack>
          <Divider sx={{ my: 1.5 }} />
          <Typography variant="caption" sx={{ color: '#718290', display: 'block', lineHeight: 1.7, px: .75 }}>
            列配置只调整展示方式。系统计算列和统计逻辑受到保护。
          </Typography>
        </Box>

        <Box sx={{ minWidth: 0, p: 1.5, overflow: 'auto' }}>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1.25 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: .75 }}>
              <GridOnIcon fontSize="small" sx={{ color: '#0a7560' }} />
              <Typography variant="subtitle2" sx={{ fontWeight: 700 }}>{activeSheet.title}</Typography>
              <Typography variant="caption" sx={{ color: '#788996' }}>Excel 预览</Typography>
            </Box>
            <FormControlLabel control={<Switch size="small" checked={activeSheet.enabled} onChange={() => updateSheet(activeSheetId, { enabled: !activeSheet.enabled })} />} label={<Typography variant="caption">导出此表</Typography>} />
          </Box>
          <Paper elevation={0} sx={{ minWidth: 720, border: '1px solid #cbd6dc', borderRadius: 0, overflow: 'hidden', bgcolor: '#fff' }}>
            <Box sx={{ height: 31, display: 'flex', alignItems: 'center', px: 1.25, borderBottom: '1px solid #dbe4e9', bgcolor: '#f7fafb', color: '#61717e', fontSize: 12 }}>
              <TableRowsIcon sx={{ fontSize: 16, mr: .75 }} /> 点击列标题编辑表头、列宽和显示状态
              <Box sx={{ ml: 'auto', color: '#8a5a0b', display: 'flex', alignItems: 'center', gap: .5 }}><LockIcon sx={{ fontSize: 14 }} /> 公式列受保护</Box>
            </Box>
            <Box sx={{ display: 'grid', gridTemplateColumns: `42px repeat(${Math.max(activeColumns.length, 1)}, minmax(94px, 1fr))`, minWidth: 720 }}>
              <Box sx={{ gridColumn: '1 / -1', display: 'grid', gridTemplateColumns: '42px 1fr' }}>
                <Box sx={{ height: 24, bgcolor: '#f0f4f6', borderRight: '1px solid #dbe4e9' }} />
                <Box sx={{ display: 'grid', gridTemplateColumns: `repeat(${Math.max(activeColumns.length, 1)}, minmax(94px, 1fr))` }}>
                  {activeColumns.map((_, index) => <Box key={index} sx={{ height: 24, textAlign: 'center', bgcolor: '#f0f4f6', borderRight: '1px solid #dbe4e9', color: '#758493', fontSize: 11, lineHeight: '24px' }}>{String.fromCharCode(65 + index)}</Box>)}
                </Box>
              </Box>
              <Box sx={{ height: 39, bgcolor: '#dceeff', borderRight: '1px solid #dbe4e9', borderBottom: '1px solid #dbe4e9' }}>1</Box>
              <Box sx={{ gridColumn: 'span ' + Math.max(activeColumns.length, 1), height: 39, bgcolor: '#dceeff', borderBottom: '1px solid #b7d7ee', textAlign: 'center', lineHeight: '39px', color: '#234961', fontWeight: 700 }}>{activeSheet.title}</Box>
              <Box sx={{ height: 27, bgcolor: '#f4f7f8', borderRight: '1px solid #dbe4e9', borderBottom: '1px solid #dbe4e9', color: '#758493', textAlign: 'center', fontSize: 11, lineHeight: '27px' }}>2</Box>
              {activeColumns.map(([key, column]) => <Button key={key} onClick={() => setActiveColumnKey(key)} sx={{ minWidth: 0, borderRadius: 0, borderRight: '1px solid #dbe4e9', borderBottom: '1px solid #dbe4e9', bgcolor: key === selectedKey ? '#0d6574' : '#2e7d8c', color: '#fff', textTransform: 'none', fontWeight: 700, justifyContent: 'flex-start', px: 1, height: 27, fontSize: 12, '&:hover': { bgcolor: '#0d6574' } }}>
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{column.label}</span>{formulaColumns.has(key) && <LockIcon sx={{ fontSize: 12, ml: 'auto' }} />}
              </Button>)}
              {[0, 1, 2, 3].map(rowIndex => <React.Fragment key={rowIndex}>
                <Box sx={{ height: 29, bgcolor: '#f4f7f8', borderRight: '1px solid #dbe4e9', borderBottom: '1px solid #dbe4e9', color: '#758493', textAlign: 'center', fontSize: 11, lineHeight: '29px' }}>{rowIndex + 3}</Box>
                {activeColumns.map(([key], index) => <Box key={key} sx={{ height: 29, px: 1, borderRight: '1px solid #dbe4e9', borderBottom: '1px solid #dbe4e9', color: formulaColumns.has(key) ? '#17634f' : '#465968', fontFamily: formulaColumns.has(key) ? 'Consolas, monospace' : 'inherit', fontSize: 12, lineHeight: '29px', bgcolor: formulaColumns.has(key) ? '#f4fcf8' : '#fff', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{columnSample(key, index + rowIndex)}</Box>)}
              </React.Fragment>)}
            </Box>
          </Paper>
        </Box>

        <Box sx={{ bgcolor: '#fff', borderLeft: '1px solid #d7e0e5', display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <Box sx={{ px: 1.5, py: 1.35, borderBottom: '1px solid #d7e0e5' }}><Typography variant="subtitle2" sx={{ fontWeight: 700 }}>工作表与列属性</Typography></Box>
          <Box sx={{ p: 1.5, flex: 1 }}>
            <TextField fullWidth size="small" label="工作表名称" value={activeSheet.title} inputProps={{ maxLength: 31 }} onChange={event => updateSheet(activeSheetId, { title: event.target.value })} sx={{ mb: 1.25 }} />
            <TextField fullWidth size="small" type="color" label="工作表颜色" value={activeSheet.color} onChange={event => updateSheet(activeSheetId, { color: event.target.value })} sx={{ mb: 1.5 }} InputLabelProps={{ shrink: true }} />
            <Divider sx={{ mb: 1.5 }} />
            {selectedColumn ? <>
              <TextField fullWidth size="small" label="字段" value={selectedKey} InputProps={{ readOnly: true }} sx={{ mb: 1.25 }} />
              <TextField fullWidth size="small" label="表头名称" value={selectedColumn.label} onChange={event => updateColumn(selectedKey, { label: event.target.value })} sx={{ mb: 1.25 }} />
              <TextField fullWidth size="small" type="number" label="列宽" value={selectedColumn.width} onChange={event => updateColumn(selectedKey, { width: Math.min(60, Math.max(3, Number(event.target.value) || 3)) })} sx={{ mb: .75 }} />
              <FormControlLabel control={<Switch size="small" checked={selectedColumn.visible !== false} onChange={() => updateColumn(selectedKey, { visible: selectedColumn.visible === false })} />} label={<Typography variant="body2">导出时显示</Typography>} />
              <Stack direction="row" spacing={.75} sx={{ mt: 1, mb: 1.5 }}>
                <Tooltip title="固定列顺序由统计结构决定"><span><IconButton size="small" disabled><ArrowBackIcon fontSize="small" /></IconButton></span></Tooltip>
                <Tooltip title="固定列顺序由统计结构决定"><span><IconButton size="small" disabled><ArrowForwardIcon fontSize="small" /></IconButton></span></Tooltip>
                <Typography variant="caption" sx={{ color: '#718290', alignSelf: 'center' }}>{isFormula ? '公式列固定位置' : '固定列顺序'}</Typography>
              </Stack>
              {isFormula && <Paper variant="outlined" sx={{ p: 1, bgcolor: '#fff9ed', borderColor: '#efd49a' }}><Typography variant="caption" sx={{ display: 'flex', gap: .5, alignItems: 'center', color: '#76591e', fontWeight: 700 }}><LockIcon sx={{ fontSize: 13 }} /> 系统公式，不可编辑</Typography><Typography variant="caption" sx={{ color: '#76591e', fontFamily: 'Consolas, monospace' }}>{formulaDescription(selectedKey)}</Typography></Paper>}
            </> : <Typography variant="body2" sx={{ color: '#718290', lineHeight: 1.7 }}>此工作表的列由当前检测类型动态生成，可配置工作表名称、颜色和是否导出。</Typography>}
          </Box>
          <Box sx={{ p: 1.25, borderTop: '1px solid #d7e0e5', display: 'flex', justifyContent: 'flex-end' }}><Button variant="contained" size="small" startIcon={<SaveIcon />} onClick={onSave} disabled={saving} sx={{ borderRadius: '3px', bgcolor: '#087f5b', '&:hover': { bgcolor: '#076c4d' } }}>{saving ? '保存中' : '保存模板'}</Button></Box>
        </Box>
      </Box>
    </Box>
  );
};

export default SpreadsheetTemplateEditor;
