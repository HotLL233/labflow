import React, { useMemo, useState } from 'react';
import {
  Alert, Box, Button, Chip, Divider, FormControlLabel, MenuItem, Paper, Popover, Select,
  Stack, Switch, TextField, Tooltip, Typography,
} from '@mui/material';
import FormatColorFillIcon from '@mui/icons-material/FormatColorFill';
import FormatColorTextIcon from '@mui/icons-material/FormatColorText';
import FormatPaintIcon from '@mui/icons-material/FormatPaint';
import RestartAltIcon from '@mui/icons-material/RestartAlt';
import RuleIcon from '@mui/icons-material/Rule';
import SaveIcon from '@mui/icons-material/Save';
import {
  COLOR_OPTIONS, FILL_OPTIONS, FONT_OPTIONS, FONT_SIZE_OPTIONS, resolveCellStyle,
  type CellStyle, type StyleAlign, type StyleWrap,
} from '../../utils/recordTableStyle';
import ConditionalFormatDialog from './ConditionalFormatDialog';
import type { RecordTableStyleApi, StyleTarget } from './useRecordTableStyle';

const R = '2px';

export interface TableStyleBarProps {
  api: RecordTableStyleApi;
  /** 当前可见列（键为字段名，标签用于下拉展示） */
  columns: { key: string; label: string }[];
  /** 当前页记录（用于「本行」定位） */
  rows: { key: string; label: string }[];
}

type Scope = 'column' | 'row' | 'body' | 'header';

const wrapLabel: Record<StyleWrap, string> = {
  single: '单行省略',
  wrap: '自动换行',
  clamp2: '最多两行',
};

/** 颜色选择器：只提供受控色板，避免出现无法导出、无法打印的任意颜色。 */
const ColorField: React.FC<{
  label: string;
  value?: string;
  options: string[];
  onChange: (color: string | undefined) => void;
}> = ({ label, value, options, onChange }) => {
  const [anchor, setAnchor] = useState<null | HTMLElement>(null);
  return (
    <>
      <Tooltip title={label}>
        <Button
          size="small"
          variant="outlined"
          onClick={event => setAnchor(event.currentTarget)}
          sx={{ minWidth: 34, px: 0.5, borderRadius: R, borderColor: '#c2ccd6' }}
        >
          <Box sx={{ width: 16, height: 16, borderRadius: R, border: '1px solid rgba(0,0,0,.2)', bgcolor: value || '#fff' }} />
        </Button>
      </Tooltip>
      <Popover
        open={Boolean(anchor)}
        anchorEl={anchor}
        onClose={() => setAnchor(null)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'left' }}
      >
        <Box sx={{ p: 1, width: 196 }}>
          <Typography variant="caption" color="text.secondary">{label}</Typography>
          <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.75 }}>
            {options.map(color => (
              <Box
                key={color}
                role="button"
                onClick={() => { onChange(color); setAnchor(null); }}
                sx={{
                  width: 22, height: 22, borderRadius: R, cursor: 'pointer',
                  bgcolor: color, border: value === color ? '2px solid #1976d2' : '1px solid rgba(0,0,0,.18)',
                }}
              />
            ))}
          </Box>
          <Divider sx={{ my: 0.75 }} />
          <Button size="small" fullWidth onClick={() => { onChange(undefined); setAnchor(null); }}>跟随上层样式</Button>
        </Box>
      </Popover>
    </>
  );
};

/** 记录表 Excel 化样式工具栏。三套记录页共用，作用范围决定样式写到哪一层。 */
const TableStyleBar: React.FC<TableStyleBarProps> = ({ api, columns, rows }) => {
  const [scope, setScope] = useState<Scope>('column');
  const [columnKey, setColumnKey] = useState(columns[0]?.key || '');
  const [rowKey, setRowKey] = useState(rows[0]?.key || '');
  const [ruleOpen, setRuleOpen] = useState(false);

  const activeColumn = columns.some(item => item.key === columnKey) ? columnKey : (columns[0]?.key || '');
  const activeRow = rows.some(item => item.key === rowKey) ? rowKey : (rows[0]?.key || '');

  const target: StyleTarget = useMemo(() => {
    switch (scope) {
      case 'row':
        return { kind: 'row', rowKey: activeRow };
      case 'body':
        return { kind: 'body' };
      case 'header':
        return { kind: 'columnHeader', column: activeColumn };
      default:
        return { kind: 'column', column: activeColumn };
    }
  }, [scope, activeRow, activeColumn]);

  /** 工具栏展示的是「生效值」，即列样式叠加表体样式后的结果。 */
  const effective: CellStyle = useMemo(() => {
    const resolved = resolveCellStyle(api.spec, {
      columnKey: scope === 'header' ? activeColumn : activeColumn,
      isHeader: scope === 'header',
      rowKey: scope === 'row' ? activeRow : undefined,
      rowValues: undefined,
    });
    return resolved.style;
  }, [api.spec, scope, activeColumn, activeRow]);

  const patch = (next: CellStyle) => api.updateCellStyle(target, next);
  const toggle = (key: 'bold' | 'italic' | 'underline') => patch({ [key]: !effective[key] } as CellStyle);

  const scopeHint = scope === 'row'
    ? `样式固定在所选记录上，翻页与排序后依然跟随该记录`
    : scope === 'body'
      ? '作用于所有数据单元格（各列单独设置会覆盖它）'
      : scope === 'header'
        ? '只作用于表头与所选列的表头，便于突出关键字段'
        : '只作用于所选列的数据单元格';

  const sourceText = api.source === 'personal'
    ? '我的视图（本机）'
    : api.source === 'global' ? '全局样式' : '系统默认';

  return (
    <Paper variant="outlined" sx={{ borderRadius: R, p: 0.75, mb: 1, bgcolor: '#fbfcfd' }}>
      <Stack direction="row" spacing={0.75} flexWrap="wrap" alignItems="center" useFlexGap sx={{ rowGap: 0.75 }}>
        <Chip
          size="small"
          color={api.source === 'personal' ? 'warning' : 'default'}
          label={`样式来源：${sourceText}`}
          sx={{ borderRadius: R }}
        />

        <Select
          size="small"
          value={scope}
          onChange={event => setScope(event.target.value as Scope)}
          sx={{ minWidth: 92, height: 30, fontSize: 12, borderRadius: R }}
        >
          <MenuItem value="column" sx={{ fontSize: 13 }}>本列</MenuItem>
          <MenuItem value="row" sx={{ fontSize: 13 }}>本行</MenuItem>
          <MenuItem value="body" sx={{ fontSize: 13 }}>全表</MenuItem>
          <MenuItem value="header" sx={{ fontSize: 13 }}>表头</MenuItem>
        </Select>

        {scope !== 'body' && (
          <Select
            size="small"
            value={scope === 'row' ? activeRow : activeColumn}
            onChange={event => (scope === 'row' ? setRowKey(event.target.value) : setColumnKey(event.target.value))}
            sx={{ minWidth: 130, height: 30, fontSize: 12, borderRadius: R }}
          >
            {(scope === 'row' ? rows : columns).map(item => (
              <MenuItem key={item.key} value={item.key} sx={{ fontSize: 13 }}>{item.label}</MenuItem>
            ))}
            {(scope === 'row' ? rows : columns).length === 0 && <MenuItem value="" disabled sx={{ fontSize: 13 }}>暂无可选项</MenuItem>}
          </Select>
        )}

        <Divider orientation="vertical" flexItem />

        <Select
          size="small"
          displayEmpty
          value={effective.fontFamily || ''}
          onChange={event => patch({ fontFamily: event.target.value || undefined })}
          sx={{ minWidth: 104, height: 30, fontSize: 12, borderRadius: R }}
        >
          {FONT_OPTIONS.map(option => <MenuItem key={option.value} value={option.value} sx={{ fontSize: 13 }}>{option.label}</MenuItem>)}
        </Select>

        <Select
          size="small"
          value={String(effective.fontSize || '')}
          onChange={event => patch({ fontSize: event.target.value ? Number(event.target.value) : undefined })}
          sx={{ minWidth: 66, height: 30, fontSize: 12, borderRadius: R }}
        >
          <MenuItem value="" sx={{ fontSize: 13 }}>默认</MenuItem>
          {FONT_SIZE_OPTIONS.map(size => <MenuItem key={size} value={String(size)} sx={{ fontSize: 13 }}>{size}px</MenuItem>)}
        </Select>

        {([['bold', 'B', '加粗'], ['italic', 'I', '斜体'], ['underline', 'U', '下划线']] as const).map(([key, label, title]) => (
          <Tooltip key={key} title={title}>
            <Button
              size="small"
              variant={effective[key] ? 'contained' : 'outlined'}
              onClick={() => toggle(key)}
              sx={{
                minWidth: 30, px: 0.5, borderRadius: R, borderColor: '#c2ccd6',
                fontWeight: key === 'bold' ? 800 : 600,
                fontStyle: key === 'italic' ? 'italic' : 'normal',
                textDecoration: key === 'underline' ? 'underline' : 'none',
              }}
            >
              {label}
            </Button>
          </Tooltip>
        ))}

        <ColorField label="字体颜色" value={effective.color} options={COLOR_OPTIONS} onChange={color => patch({ color })} />
        <ColorField label="单元格填充" value={effective.bgColor} options={FILL_OPTIONS} onChange={color => patch({ bgColor: color })} />

        <Divider orientation="vertical" flexItem />

        {(['left', 'center', 'right'] as StyleAlign[]).map(align => (
          <Tooltip key={align} title={align === 'left' ? '左对齐' : align === 'center' ? '居中' : '右对齐'}>
            <Button
              size="small"
              variant={effective.align === align ? 'contained' : 'outlined'}
              onClick={() => patch({ align })}
              sx={{ minWidth: 32, px: 0.5, borderRadius: R, borderColor: '#c2ccd6' }}
            >
              {align === 'left' ? '左' : align === 'center' ? '中' : '右'}
            </Button>
          </Tooltip>
        ))}

        <Select
          size="small"
          displayEmpty
          value={effective.wrap || ''}
          onChange={event => patch({ wrap: (event.target.value || undefined) as StyleWrap | undefined })}
          sx={{ minWidth: 108, height: 30, fontSize: 12, borderRadius: R }}
        >
          <MenuItem value="" sx={{ fontSize: 13 }}>默认换行</MenuItem>
          {(['single', 'wrap', 'clamp2'] as StyleWrap[]).map(mode => (
            <MenuItem key={mode} value={mode} sx={{ fontSize: 13 }}>{wrapLabel[mode]}</MenuItem>
          ))}
        </Select>

        <Button
          size="small"
          variant="outlined"
          startIcon={<RestartAltIcon fontSize="small" />}
          onClick={() => patch({
            fontFamily: undefined, fontSize: undefined, bold: undefined, italic: undefined,
            underline: undefined, color: undefined, bgColor: undefined, align: undefined, wrap: undefined,
          })}
          sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
        >
          清除本范围
        </Button>
      </Stack>

      <Stack direction="row" spacing={0.75} flexWrap="wrap" alignItems="center" useFlexGap sx={{ mt: 0.75, rowGap: 0.75 }}>
        <Typography variant="caption" color="text.secondary">行高</Typography>
        <TextField
          size="small"
          type="number"
          value={api.spec.table.rowHeight}
          onChange={event => api.updateTableStyle({ rowHeight: Number(event.target.value) || 38 })}
          sx={{ width: 74, '& input': { fontSize: 12, py: 0.5 } }}
        />
        <Typography variant="caption" color="text.secondary">表头行高</Typography>
        <TextField
          size="small"
          type="number"
          value={api.spec.table.headerHeight}
          onChange={event => api.updateTableStyle({ headerHeight: Number(event.target.value) || 36 })}
          sx={{ width: 74, '& input': { fontSize: 12, py: 0.5 } }}
        />
        <Typography variant="caption" color="text.secondary">边框</Typography>
        <ColorField
          label="表格边框颜色"
          value={api.spec.table.borderColor}
          options={['#d9dfe7', '#c2ccd6', '#9aa7b4', '#1f2d3d', '#1976d2', '#c62828']}
          onChange={color => api.updateTableStyle({ borderColor: color || '#d9dfe7' })}
        />
        <FormControlLabel
          sx={{ m: 0 }}
          control={<Switch size="small" checked={api.spec.table.zebra} onChange={event => api.updateTableStyle({ zebra: event.target.checked })} />}
          label={<Typography variant="caption">斑马纹</Typography>}
        />

        <Divider orientation="vertical" flexItem />

        <Button
          size="small"
          variant="outlined"
          startIcon={<RuleIcon fontSize="small" />}
          onClick={() => setRuleOpen(true)}
          sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
        >
          条件格式{api.spec.rules.length > 0 ? `（${api.spec.rules.length}）` : ''}
        </Button>
        <Button
          size="small"
          variant="outlined"
          startIcon={<FormatPaintIcon fontSize="small" />}
          onClick={api.resetPersonal}
          disabled={api.source !== 'personal'}
          sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
        >
          跟随全局
        </Button>
        <Button
          size="small"
          variant="outlined"
          onClick={api.resetAll}
          sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
        >
          恢复默认
        </Button>
        {api.canEditGlobal && (
          <Button
            size="small"
            variant="contained"
            startIcon={<SaveIcon fontSize="small" />}
            disabled={api.saving}
            onClick={() => api.saveGlobal()}
            sx={{ borderRadius: R }}
          >
            发布为全局
          </Button>
        )}

        <Box sx={{ flex: 1 }} />
        <Typography variant="caption" color="text.secondary">{scopeHint}</Typography>
      </Stack>

      {(api.message || api.source === 'personal') && (
        <Box sx={{ mt: 0.75 }}>
          {api.message ? (
            <Alert severity={api.messageError ? 'error' : 'success'} onClose={api.clearMessage} sx={{ borderRadius: R, py: 0.25 }}>
              {api.message}
            </Alert>
          ) : (
            <Alert severity="info" icon={false} sx={{ borderRadius: R, py: 0.25 }}>
              当前使用本机个人视图，仅自己可见；点击「发布为全局」可让所有人使用（需页面与录入表单配置权限）。
            </Alert>
          )}
        </Box>
      )}

      <ConditionalFormatDialog
        open={ruleOpen}
        onClose={() => setRuleOpen(false)}
        rules={api.spec.rules}
        columns={columns}
        rows={rows}
        onChange={api.setRules}
      />
    </Paper>
  );
};

export default TableStyleBar;

/** 供页面复用的图标色块（填充 / 字体色）提示，避免每个页面各写一套。 */
export const StyleLegendIcons = { FormatColorFillIcon, FormatColorTextIcon };
