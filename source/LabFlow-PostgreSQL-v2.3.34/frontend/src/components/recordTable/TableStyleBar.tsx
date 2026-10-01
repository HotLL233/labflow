import React, { useMemo, useState } from 'react';
import {
  Alert, Box, Button, Checkbox, Divider, Drawer, FormControlLabel, IconButton, MenuItem,
  Popover, Select, Stack, Switch, TextField, Tooltip, Typography,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import FormatColorFillIcon from '@mui/icons-material/FormatColorFill';
import FormatColorTextIcon from '@mui/icons-material/FormatColorText';
import FormatPaintIcon from '@mui/icons-material/FormatPaint';
import RestartAltIcon from '@mui/icons-material/RestartAlt';
import RuleIcon from '@mui/icons-material/Rule';
import SaveIcon from '@mui/icons-material/Save';
import TuneIcon from '@mui/icons-material/Tune';
import {
  COLOR_OPTIONS, DATE_FORMAT_OPTIONS, FILL_OPTIONS, FONT_OPTIONS, FONT_SIZE_OPTIONS, PAGE_SIZE_OPTIONS,
  resolveCellStyle, type CellStyle, type LayoutMode, type StyleAlign, type StyleWrap,
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
  /** 该页面是否有选择复选框列 */
  hasCheckbox?: boolean;
  /** 该页面是否有序号列 */
  hasSeq?: boolean;
  /** 日期 / 日期时间字段，用于提示日期格式只对这些列生效 */
  dateColumns?: string[];
  /** v2.3.31：当前是否窄屏卡片布局，卡片下表格级样式不适用 */
  cardLayout?: boolean;
  /** v2.3.32：每页条数变化时通知页面重新按新条数取数（分页由父页面发请求时需要） */
  onPageSizeChange?: (size: number) => void;
  /** v2.3.34：该页面是否同时具备卡片与表格两种布局；只有表格的页面不显示布局开关 */
  hasCardLayout?: boolean;
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
  compact?: boolean;
}> = ({ label, value, options, onChange, compact }) => {
  const [anchor, setAnchor] = useState<null | HTMLElement>(null);
  return (
    <>
      <Tooltip title={label}>
        <Button
          size="small"
          variant="outlined"
          onClick={event => setAnchor(event.currentTarget)}
          sx={{ minWidth: compact ? 30 : 34, px: 0.5, borderRadius: R, borderColor: '#c2ccd6' }}
        >
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
            <Box sx={{ width: 14, height: 14, borderRadius: R, border: '1px solid rgba(0,0,0,.2)', bgcolor: value || '#fff' }} />
            {!compact && <Typography variant="caption">{label}</Typography>}
          </Box>
        </Button>
      </Tooltip>
      <Popover
        open={Boolean(anchor)}
        anchorEl={anchor}
        onClose={() => setAnchor(null)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'left' }}
      >
        <Box sx={{ p: 1, width: 200 }}>
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

const SectionTitle: React.FC<{ children: React.ReactNode; hint?: string }> = ({ children, hint }) => (
  <Box sx={{ mb: 0.75 }}>
    <Typography variant="subtitle2" sx={{ fontWeight: 800, fontSize: 12.5 }}>{children}</Typography>
    {hint && <Typography variant="caption" color="text.secondary">{hint}</Typography>}
  </Box>
);

/**
 * 记录表样式入口。
 *
 * v2.3.30：配置全部收进「表格样式」按钮打开的抽屉，
 * 平时只占一个按钮，不再常驻占用页面高度。
 */
const TableStyleBar: React.FC<TableStyleBarProps> = ({
  api, columns, rows, hasCheckbox = false, hasSeq = false, dateColumns = [], cardLayout = false,
  onPageSizeChange, hasCardLayout = true,
}) => {
  const [open, setOpen] = useState(false);
  const [ruleOpen, setRuleOpen] = useState(false);
  const [scope, setScope] = useState<Scope>('column');
  const [columnKey, setColumnKey] = useState(columns[0]?.key || '');
  const [rowKey, setRowKey] = useState(rows[0]?.key || '');

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

  /** 界面显示的是生效值（列样式叠加表体样式后的结果）。 */
  const effective: CellStyle = useMemo(() => resolveCellStyle(api.spec, {
    columnKey: activeColumn,
    isHeader: scope === 'header',
    rowKey: scope === 'row' ? activeRow : undefined,
  }).style, [api.spec, scope, activeColumn, activeRow]);

  const patch = (next: CellStyle) => api.updateCellStyle(target, next);
  const toggle = (key: 'bold' | 'italic' | 'underline') => patch({ [key]: !effective[key] } as CellStyle);

  const columnWidth = api.spec.columns[activeColumn]?.width;
  const hiddenCount = api.spec.hiddenColumns.length;
  const visibleCount = columns.length - columns.filter(item => api.spec.hiddenColumns.includes(item.key)).length;
  const isDateColumn = dateColumns.includes(activeColumn);
  const isPersonal = api.source === 'personal';

  const scopeHint = scope === 'row'
    ? '样式绑定在所选记录上，翻页与排序后依然跟随该记录'
    : scope === 'body'
      ? '作用于全部数据单元格；单列设置会覆盖它'
      : scope === 'header'
        ? '只改变所选列的表头'
        : '只作用于所选列的数据单元格';

  return (
    <>
      <Button
        size="small"
        variant={isPersonal ? 'contained' : 'outlined'}
        color={isPersonal ? 'warning' : 'primary'}
        startIcon={<TuneIcon fontSize="small" />}
        onClick={() => setOpen(true)}
        sx={{ borderRadius: R }}
      >
        表格样式{isPersonal ? '（我的视图）' : ''}
      </Button>

      <Drawer
        anchor="right"
        open={open}
        onClose={() => setOpen(false)}
        PaperProps={{ sx: { width: { xs: '92vw', sm: 360 } } }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', px: 1.5, py: 1, borderBottom: '1px solid #e0e0e0' }}>
          <Typography sx={{ fontWeight: 800, fontSize: 14 }}>表格样式</Typography>
          <Typography variant="caption" color="text.secondary" sx={{ ml: 1 }}>
            {api.source === 'personal' ? '我的视图（仅本机）' : api.source === 'global' ? '全局样式' : '系统默认'}
          </Typography>
          <Box sx={{ flex: 1 }} />
          <IconButton size="small" onClick={() => setOpen(false)} aria-label="关闭表格样式"><CloseIcon fontSize="small" /></IconButton>
        </Box>

        <Box sx={{ p: 1.5, overflow: 'auto' }}>
          {api.message && (
            <Alert severity={api.messageError ? 'error' : 'success'} onClose={api.clearMessage} sx={{ borderRadius: R, mb: 1.5, py: 0.25 }}>
              {api.message}
            </Alert>
          )}

          {/* ---------- 列显示 ---------- */}
          <SectionTitle hint={`已显示 ${visibleCount} / ${columns.length} 列`}>列显示</SectionTitle>
          <Stack direction="row" spacing={0.5} flexWrap="wrap" useFlexGap sx={{ mb: 0.75 }}>
            {hasCheckbox && (
              <FormControlLabel
                sx={{ m: 0, mr: 1 }}
                control={<Switch size="small" checked={api.spec.table.showCheckbox} onChange={event => api.updateTableStyle({ showCheckbox: event.target.checked })} />}
                label={<Typography variant="caption">选择框列</Typography>}
              />
            )}
            {hasSeq && (
              <FormControlLabel
                sx={{ m: 0 }}
                control={<Switch size="small" checked={api.spec.table.showSeq} onChange={event => api.updateTableStyle({ showSeq: event.target.checked })} />}
                label={<Typography variant="caption">序号列</Typography>}
              />
            )}
          </Stack>
          <Box sx={{ border: '1px solid #e0e0e0', borderRadius: R, maxHeight: 188, overflow: 'auto', mb: 0.75 }}>
            {columns.map(column => (
              <Box
                key={column.key}
                onClick={() => api.toggleColumnHidden(column.key)}
                sx={{ display: 'flex', alignItems: 'center', px: 0.75, py: 0.25, cursor: 'pointer', '&:hover': { bgcolor: 'action.hover' } }}
              >
                <Checkbox size="small" checked={!api.spec.hiddenColumns.includes(column.key)} sx={{ p: 0.4, mr: 0.5 }} />
                <Typography variant="caption" noWrap>{column.label}</Typography>
              </Box>
            ))}
          </Box>
          <Stack direction="row" spacing={0.75} sx={{ mb: 1.5 }}>
            <Button size="small" disabled={hiddenCount === 0} onClick={api.showAllColumns} sx={{ borderRadius: R }}>全部显示</Button>
            <Button size="small" disabled={hiddenCount === 0} onClick={() => columns.forEach(column => api.spec.hiddenColumns.includes(column.key) || api.toggleColumnHidden(column.key))} sx={{ borderRadius: R }}>只留未隐藏</Button>
          </Stack>

          <Divider sx={{ mb: 1.5 }} />

          {/* ---------- 表格 ---------- */}
          <SectionTitle hint="行高、表头冻结与斑马纹对整张表生效">表格</SectionTitle>
          {cardLayout && (
            <Alert severity="info" icon={false} sx={{ borderRadius: R, py: 0.25, mb: 0.75 }}>
              窄屏使用卡片布局：行高、表头行高、列宽、斑马纹与冻结表头不适用，已置灰。
            </Alert>
          )}
          <Stack direction="row" spacing={1} alignItems="center" flexWrap="wrap" useFlexGap sx={{ mb: 0.75 }}>
            <Typography variant="caption" sx={{ width: 56 }}>行高</Typography>
            <TextField
              size="small" type="number" value={api.spec.table.rowHeight} disabled={cardLayout}
              onChange={event => api.updateTableStyle({ rowHeight: Number(event.target.value) || 38 })}
              sx={{ width: 80, '& input': { fontSize: 12, py: 0.5 } }}
            />
            <Typography variant="caption" sx={{ width: 62 }}>表头行高</Typography>
            <TextField
              size="small" type="number" value={api.spec.table.headerHeight} disabled={cardLayout}
              onChange={event => api.updateTableStyle({ headerHeight: Number(event.target.value) || 36 })}
              sx={{ width: 80, '& input': { fontSize: 12, py: 0.5 } }}
            />
          </Stack>
          <Stack direction="row" spacing={0.75} alignItems="center" flexWrap="wrap" useFlexGap>
            <FormControlLabel
              sx={{ m: 0 }}
              control={<Switch size="small" disabled={cardLayout} checked={api.spec.table.stickyHeader} onChange={event => api.updateTableStyle({ stickyHeader: event.target.checked })} />}
              label={<Typography variant="caption">冻结表头</Typography>}
            />
            <FormControlLabel
              sx={{ m: 0 }}
              control={<Switch size="small" disabled={cardLayout} checked={api.spec.table.zebra} onChange={event => api.updateTableStyle({ zebra: event.target.checked })} />}
              label={<Typography variant="caption">斑马纹</Typography>}
            />
            <Box sx={{ pointerEvents: cardLayout ? 'none' : 'auto', opacity: cardLayout ? 0.5 : 1 }}>
              <ColorField
                label="边框颜色" compact value={api.spec.table.borderColor}
                options={['#d9dfe7', '#c2ccd6', '#9aa7b4', '#1f2d3d', '#1976d2', '#c62828']}
                onChange={color => api.updateTableStyle({ borderColor: color || '#d9dfe7' })}
              />
            </Box>
          </Stack>

          <Divider sx={{ my: 1.5 }} />

          {/* ---------- 布局 ---------- */}
          {hasCardLayout && (
            <>
              <SectionTitle hint="自动：手机用卡片、电脑用表格（手机开「桌面网站」会按电脑处理）">布局</SectionTitle>
              <Stack direction="row" spacing={0.75} sx={{ mb: 1.5 }}>
                {([['auto', '自动'], ['card', '卡片'], ['table', '表格']] as [LayoutMode, string][]).map(([mode, label]) => (
                  <Button
                    key={mode}
                    size="small"
                    variant={api.spec.layout.mode === mode ? 'contained' : 'outlined'}
                    onClick={() => api.setLayoutMode(mode)}
                    sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
                  >
                    {label}
                  </Button>
                ))}
              </Stack>

              <Divider sx={{ mb: 1.5 }} />
            </>
          )}

          {/* ---------- 手机卡片 ---------- */}
          <SectionTitle hint="窄屏卡片布局的边框与底色，可与表格分别配色">手机卡片</SectionTitle>
          <Stack direction="row" spacing={0.75} alignItems="center" flexWrap="wrap" useFlexGap sx={{ mb: 0.75 }}>
            <Typography variant="caption" sx={{ width: 72 }}>卡片外框</Typography>
            <ColorField
              label="卡片外框颜色" compact value={api.spec.card.borderColor}
              options={FILL_OPTIONS.concat(['#e0e0e0', '#c2ccd6', '#9aa7b4', '#1f2d3d'])}
              onChange={color => api.updateCardStyle({ borderColor: color || '#e0e0e0' })}
            />
            <TextField
              size="small" type="number" value={api.spec.card.borderWidth}
              onChange={event => api.updateCardStyle({ borderWidth: Number(event.target.value) || 0 })}
              sx={{ width: 60, '& input': { fontSize: 12, py: 0.5 } }}
            />
            <Typography variant="caption" color="text.secondary">px</Typography>
            <Typography variant="caption" sx={{ width: 72, ml: 1 }}>卡片底色</Typography>
            <ColorField
              label="卡片底色" compact value={api.spec.card.bgColor || undefined}
              options={FILL_OPTIONS}
              onChange={color => api.updateCardStyle({ bgColor: color || '' })}
            />
          </Stack>
          <Stack direction="row" spacing={0.75} alignItems="center" flexWrap="wrap" useFlexGap sx={{ mb: 1.5 }}>
            <Typography variant="caption" sx={{ width: 72 }}>字段块外框</Typography>
            <ColorField
              label="字段块外框颜色" compact value={api.spec.card.fieldBorderColor}
              options={FILL_OPTIONS.concat(['#d9dfe7', '#c2ccd6', '#9aa7b4', '#1f2d3d'])}
              onChange={color => api.updateCardStyle({ fieldBorderColor: color || '#d9dfe7' })}
            />
            <TextField
              size="small" type="number" value={api.spec.card.fieldBorderWidth}
              onChange={event => api.updateCardStyle({ fieldBorderWidth: Number(event.target.value) || 0 })}
              sx={{ width: 60, '& input': { fontSize: 12, py: 0.5 } }}
            />
            <Typography variant="caption" color="text.secondary">px</Typography>
            <Typography variant="caption" sx={{ width: 72, ml: 1 }}>字段块底色</Typography>
            <ColorField
              label="字段块底色" compact value={api.spec.card.fieldBgColor || undefined}
              options={FILL_OPTIONS}
              onChange={color => api.updateCardStyle({ fieldBgColor: color || '' })}
            />
          </Stack>

          <Divider sx={{ mb: 1.5 }} />

          {/* ---------- 分页 ---------- */}
          <SectionTitle hint="每页条数对界面与接口请求同时生效">分页</SectionTitle>
          <Stack direction="row" spacing={0.75} alignItems="center" sx={{ mb: 1.5 }}>
            <Typography variant="caption" sx={{ width: 72 }}>每页行数</Typography>
            <Select
              size="small" value={String(api.spec.paging.size)}
              onChange={event => {
                const size = Number(event.target.value);
                api.setPageSize(size);
                onPageSizeChange?.(size);
              }}
              sx={{ minWidth: 128, height: 32, fontSize: 12.5, borderRadius: R }}
            >
              <MenuItem value="0" sx={{ fontSize: 13 }}>页面默认</MenuItem>
              {PAGE_SIZE_OPTIONS.map(size => (
                <MenuItem key={size} value={String(size)} sx={{ fontSize: 13 }}>{size} 行</MenuItem>
              ))}
            </Select>
          </Stack>

          <Divider sx={{ mb: 1.5 }} />

          {/* ---------- 样式作用范围 ---------- */}
          <SectionTitle hint={scopeHint}>样式范围</SectionTitle>
          <Stack direction="row" spacing={0.75} flexWrap="wrap" useFlexGap sx={{ mb: 0.75 }}>
            {([['column', '本列'], ['row', '本行'], ['body', '全表'], ['header', '表头']] as [Scope, string][]).map(([value, label]) => (
              <Button
                key={value} size="small" variant={scope === value ? 'contained' : 'outlined'}
                onClick={() => setScope(value)} sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
              >
                {label}
              </Button>
            ))}
          </Stack>
          {scope !== 'body' && (
            <Select
              size="small" fullWidth
              value={scope === 'row' ? activeRow : activeColumn}
              onChange={event => (scope === 'row' ? setRowKey(event.target.value) : setColumnKey(event.target.value))}
              sx={{ height: 32, fontSize: 12.5, borderRadius: R, mb: 0.75 }}
            >
              {(scope === 'row' ? rows : columns).map(item => (
                <MenuItem key={item.key} value={item.key} sx={{ fontSize: 13 }}>{item.label}</MenuItem>
              ))}
              {(scope === 'row' ? rows : columns).length === 0 && <MenuItem value="" disabled sx={{ fontSize: 13 }}>暂无可选项</MenuItem>}
            </Select>
          )}

          {/* ---------- 列宽 ---------- */}
          {scope === 'column' && (
            <>
              <Stack direction="row" spacing={0.75} alignItems="center" sx={{ mb: 0.75 }}>
                <Typography variant="caption" sx={{ width: 56 }}>列宽</Typography>
                <TextField
                  size="small" type="number" value={columnWidth ?? ''} placeholder="自动" disabled={cardLayout}
                  onChange={event => {
                    const value = Number(event.target.value);
                    api.setColumnWidth(activeColumn, value > 0 ? value : undefined);
                  }}
                  sx={{ width: 90, '& input': { fontSize: 12, py: 0.5 } }}
                />
                <Typography variant="caption" color="text.secondary">px</Typography>
                <Button
                  size="small" disabled={cardLayout || columnWidth === undefined}
                  onClick={() => api.setColumnWidth(activeColumn, undefined)}
                  sx={{ borderRadius: R }}
                >
                  恢复自动
                </Button>
              </Stack>
              <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1.5 }}>
                {cardLayout
                  ? '窄屏为卡片布局，列宽由卡片自适应，不适用。'
                  : '也可以直接拖动表头右侧的竖线调整列宽，拖完会自动写入这里。'}
              </Typography>
            </>
          )}

          {/* ---------- 日期格式 ---------- */}
          <Stack direction="row" spacing={0.75} alignItems="center" sx={{ mb: 1.5 }}>
            <Typography variant="caption" sx={{ width: 56 }}>日期格式</Typography>
            <Select
              size="small" displayEmpty value={effective.dateFormat || ''}
              disabled={scope === 'body'}
              onChange={event => patch({ dateFormat: event.target.value || undefined })}
              sx={{ minWidth: 160, height: 32, fontSize: 12.5, borderRadius: R }}
            >
              {DATE_FORMAT_OPTIONS.map(option => (
                <MenuItem key={option.value} value={option.value} sx={{ fontSize: 13 }}>{option.label}</MenuItem>
              ))}
            </Select>
          </Stack>
          {scope === 'column' && !isDateColumn && (
            <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1.5 }}>
              当前列不是日期列，设置后不生效。日期列：{dateColumns.length > 0 ? dateColumns.map(key => columns.find(item => item.key === key)?.label || key).join('、') : '本表无'}
            </Typography>
          )}

          <Divider sx={{ mb: 1.5 }} />

          {/* ---------- 文本样式 ---------- */}
          <SectionTitle>文字与对齐</SectionTitle>
          <Stack direction="row" spacing={0.75} flexWrap="wrap" useFlexGap sx={{ mb: 0.75 }}>
            <Select
              size="small" displayEmpty value={effective.fontFamily || ''}
              onChange={event => patch({ fontFamily: event.target.value || undefined })}
              sx={{ minWidth: 104, height: 32, fontSize: 12.5, borderRadius: R }}
            >
              {FONT_OPTIONS.map(option => <MenuItem key={option.value} value={option.value} sx={{ fontSize: 13 }}>{option.label}</MenuItem>)}
            </Select>
            <Select
              size="small" displayEmpty value={String(effective.fontSize || '')}
              onChange={event => patch({ fontSize: event.target.value ? Number(event.target.value) : undefined })}
              sx={{ minWidth: 74, height: 32, fontSize: 12.5, borderRadius: R }}
            >
              <MenuItem value="" sx={{ fontSize: 13 }}>默认</MenuItem>
              {FONT_SIZE_OPTIONS.map(size => <MenuItem key={size} value={String(size)} sx={{ fontSize: 13 }}>{size}px</MenuItem>)}
            </Select>
            {([['bold', 'B', '加粗'], ['italic', 'I', '斜体'], ['underline', 'U', '下划线']] as const).map(([key, label, title]) => (
              <Tooltip key={key} title={title}>
                <Button
                  size="small" variant={effective[key] ? 'contained' : 'outlined'} onClick={() => toggle(key)}
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
            <ColorField label="字体颜色" compact value={effective.color} options={COLOR_OPTIONS} onChange={color => patch({ color })} />
            <ColorField label="单元格填充" compact value={effective.bgColor} options={FILL_OPTIONS} onChange={color => patch({ bgColor: color })} />
          </Stack>
          <Stack direction="row" spacing={0.75} alignItems="center" flexWrap="wrap" useFlexGap sx={{ mb: 0.75 }}>
            <Typography variant="caption" sx={{ width: 56 }}>对齐</Typography>
            {(['left', 'center', 'right'] as StyleAlign[]).map(align => (
              <Button
                key={align} size="small" variant={effective.align === align ? 'contained' : 'outlined'}
                onClick={() => patch({ align })} sx={{ minWidth: 32, px: 0.5, borderRadius: R, borderColor: '#c2ccd6' }}
              >
                {align === 'left' ? '左' : align === 'center' ? '中' : '右'}
              </Button>
            ))}
            <Select
              size="small" displayEmpty value={effective.wrap || ''}
              onChange={event => patch({ wrap: (event.target.value || undefined) as StyleWrap | undefined })}
              sx={{ minWidth: 108, height: 32, fontSize: 12.5, borderRadius: R }}
            >
              <MenuItem value="" sx={{ fontSize: 13 }}>默认换行</MenuItem>
              {(['single', 'wrap', 'clamp2'] as StyleWrap[]).map(mode => (
                <MenuItem key={mode} value={mode} sx={{ fontSize: 13 }}>{wrapLabel[mode]}</MenuItem>
              ))}
            </Select>
          </Stack>
          <Button
            size="small" variant="outlined" startIcon={<RestartAltIcon fontSize="small" />}
            onClick={() => patch({
              fontFamily: undefined, fontSize: undefined, bold: undefined, italic: undefined,
              underline: undefined, color: undefined, bgColor: undefined, align: undefined,
              wrap: undefined, dateFormat: undefined,
            })}
            sx={{ borderRadius: R, borderColor: '#c2ccd6', mb: 1.5 }}
          >
            清除本范围样式
          </Button>

          <Divider sx={{ mb: 1.5 }} />

          {/* ---------- 条件格式 ---------- */}
          <SectionTitle hint="按字段值自动套用样式，排在后面的规则优先">条件格式</SectionTitle>
          <Button
            size="small" variant="outlined" startIcon={<RuleIcon fontSize="small" />}
            onClick={() => setRuleOpen(true)} sx={{ borderRadius: R, mb: 1.5 }}
          >
            编辑规则{api.spec.rules.length > 0 ? `（${api.spec.rules.length}）` : ''}
          </Button>

          <Divider sx={{ mb: 1.5 }} />

          {/* ---------- 保存 ---------- */}
          <SectionTitle hint={api.canEditGlobal ? '发布后所有人可见，变更写入审计日志' : '仅系统管理员可以发布全局样式'}>保存方式</SectionTitle>
          <Stack direction="row" spacing={0.75} flexWrap="wrap" useFlexGap>
            <Button
              size="small" variant="outlined" startIcon={<FormatPaintIcon fontSize="small" />}
              onClick={api.resetPersonal} disabled={!isPersonal} sx={{ borderRadius: R, borderColor: '#c2ccd6' }}
            >
              跟随全局
            </Button>
            <Button size="small" variant="outlined" onClick={api.resetAll} sx={{ borderRadius: R, borderColor: '#c2ccd6' }}>
              恢复默认
            </Button>
            {api.canEditGlobal && (
              <Button
                size="small" variant="contained" startIcon={<SaveIcon fontSize="small" />}
                disabled={api.saving} onClick={() => api.saveGlobal()} sx={{ borderRadius: R }}
              >
                发布为全局
              </Button>
            )}
          </Stack>
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>
            未发布的修改只保存在本机个人视图，不影响其他人。
          </Typography>
        </Box>
      </Drawer>

      <ConditionalFormatDialog
        open={ruleOpen}
        onClose={() => setRuleOpen(false)}
        rules={api.spec.rules}
        columns={columns}
        rows={rows}
        onChange={api.setRules}
      />
    </>
  );
};

export default TableStyleBar;

export const StyleLegendIcons = { FormatColorFillIcon, FormatColorTextIcon };
