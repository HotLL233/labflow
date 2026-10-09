import React from 'react';
import {
  Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, Divider, FormControlLabel,
  IconButton, MenuItem, Select, Stack, Switch, TextField, Tooltip, Typography,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import {
  COLOR_OPTIONS, FILL_OPTIONS, RULE_OP_LABELS, createRuleId,
  type CellStyle, type ConditionalRule, type RuleOp,
} from '../../utils/recordTableStyle';

const R = '2px';

export interface ConditionalFormatDialogProps {
  open: boolean;
  onClose: () => void;
  rules: ConditionalRule[];
  columns: { key: string; label: string }[];
  rows: { key: string; label: string }[];
  onChange: (rules: ConditionalRule[]) => void;
}

const colorSwatch = (color: string) => (
  <Box component="span" sx={{ display: 'inline-block', width: 12, height: 12, borderRadius: '2px', bgcolor: color, border: '1px solid rgba(0,0,0,.2)', mr: 0.75, verticalAlign: 'middle' }} />
);

/**
 * 条件格式规则编辑器。
 *
 * 行级样式的推荐做法：用字段值（状态、数量、日期等）描述规则，
 * 而不是按行号手工刷色——这样翻页、排序、数据更新后样式依旧正确。
 */
const ConditionalFormatDialog: React.FC<ConditionalFormatDialogProps> = ({
  open, onClose, rules, columns, rows, onChange,
}) => {
  const fieldOptions = columns.length > 0 ? columns : [{ key: 'status', label: '状态' }, { key: 'quantity', label: '数量' }];
  const rowField = fieldOptions[0]?.key || 'status';

  const addRule = () => {
    onChange([
      ...rules,
      {
        id: createRuleId(),
        name: `规则 ${rules.length + 1}`,
        enabled: true,
        range: 'row',
        match: 'all',
        conditions: [{ field: rowField, op: 'eq', value: '' }],
        style: { bgColor: '#fff5f5', leftBar: '#c62828' },
      },
    ]);
  };

  const updateRule = (id: string, patch: Partial<ConditionalRule>) => {
    onChange(rules.map(rule => (rule.id === id ? { ...rule, ...patch } : rule)));
  };

  const updateStyle = (id: string, patch: CellStyle & { leftBar?: string }) => {
    const rule = rules.find(item => item.id === id);
    if (!rule) return;
    updateRule(id, { style: { ...rule.style, ...patch } });
  };

  const move = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= rules.length) return;
    const next = [...rules];
    [next[index], next[target]] = [next[target], next[index]];
    onChange(next);
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle sx={{ fontSize: 15, fontWeight: 700 }}>
        条件格式规则
        <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 1 }}>
          按字段值自动套用样式，排在后面的规则优先级更高
        </Typography>
      </DialogTitle>
      <DialogContent dividers>
        {rules.length === 0 && (
          <Typography variant="body2" color="text.secondary" sx={{ py: 2, textAlign: 'center' }}>
            暂无规则。例如「状态 = 待取样 → 整行浅红」，可让待处理记录一眼可见。
          </Typography>
        )}
        <Stack spacing={1.25}>
          {rules.map((rule, index) => (
            <Box key={rule.id} sx={{ border: '1px solid #d9dfe7', borderRadius: R, p: 1.25, bgcolor: rule.enabled ? '#fff' : '#fafbfc' }}>
              <Stack direction="row" spacing={0.75} alignItems="center" flexWrap="wrap" useFlexGap>
                <TextField
                  size="small"
                  value={rule.name}
                  onChange={event => updateRule(rule.id, { name: event.target.value })}
                  sx={{ width: 150, '& input': { fontSize: 12, py: 0.5 } }}
                />
                <Select
                  size="small"
                  value={rule.range}
                  onChange={event => updateRule(rule.id, { range: event.target.value as 'row' | 'column' })}
                  sx={{ minWidth: 90, height: 30, fontSize: 12, borderRadius: R }}
                >
                  <MenuItem value="row" sx={{ fontSize: 13 }}>整行</MenuItem>
                  <MenuItem value="column" sx={{ fontSize: 13 }}>指定列</MenuItem>
                </Select>
                {rule.range === 'column' && (
                  <Select
                    size="small"
                    value={rule.column || columns[0]?.key || ''}
                    onChange={event => updateRule(rule.id, { column: event.target.value })}
                    sx={{ minWidth: 120, height: 30, fontSize: 12, borderRadius: R }}
                  >
                    {columns.map(column => <MenuItem key={column.key} value={column.key} sx={{ fontSize: 13 }}>{column.label}</MenuItem>)}
                  </Select>
                )}
                <Select
                  size="small"
                  value={rule.match}
                  onChange={event => updateRule(rule.id, { match: event.target.value as 'all' | 'any' })}
                  sx={{ minWidth: 92, height: 30, fontSize: 12, borderRadius: R }}
                >
                  <MenuItem value="all" sx={{ fontSize: 13 }}>满足全部</MenuItem>
                  <MenuItem value="any" sx={{ fontSize: 13 }}>满足任一</MenuItem>
                </Select>
                <FormControlLabel
                  sx={{ m: 0 }}
                  control={<Switch size="small" checked={rule.enabled} onChange={event => updateRule(rule.id, { enabled: event.target.checked })} />}
                  label={<Typography variant="caption">启用</Typography>}
                />
                <Box sx={{ flex: 1 }} />
                <Tooltip title="上移（优先级更高）">
                  <span>
                    <IconButton size="small" disabled={index === 0} onClick={() => move(index, -1)}><ArrowUpwardIcon fontSize="small" /></IconButton>
                  </span>
                </Tooltip>
                <Tooltip title="下移">
                  <span>
                    <IconButton size="small" disabled={index === rules.length - 1} onClick={() => move(index, 1)}><ArrowDownwardIcon fontSize="small" /></IconButton>
                  </span>
                </Tooltip>
                <Tooltip title="删除规则">
                  <IconButton size="small" color="error" onClick={() => onChange(rules.filter(item => item.id !== rule.id))}>
                    <DeleteOutlineIcon fontSize="small" />
                  </IconButton>
                </Tooltip>
              </Stack>

              <Divider sx={{ my: 1 }} />

              <Stack spacing={0.75}>
                {rule.conditions.map((condition, conditionIndex) => (
                  <Stack key={`${rule.id}-${conditionIndex}`} direction="row" spacing={0.75} alignItems="center" flexWrap="wrap" useFlexGap>
                    <Typography variant="caption" color="text.secondary" sx={{ width: 42 }}>
                      {conditionIndex === 0 ? '当' : rule.match === 'any' ? '或' : '且'}
                    </Typography>
                    <Select
                      size="small"
                      value={condition.field}
                      onChange={event => {
                        const next = [...rule.conditions];
                        next[conditionIndex] = { ...condition, field: event.target.value };
                        updateRule(rule.id, { conditions: next });
                      }}
                      sx={{ minWidth: 130, height: 30, fontSize: 12, borderRadius: R }}
                    >
                      {fieldOptions.map(option => <MenuItem key={option.key} value={option.key} sx={{ fontSize: 13 }}>{option.label}</MenuItem>)}
                    </Select>
                    <Select
                      size="small"
                      value={condition.op}
                      onChange={event => {
                        const next = [...rule.conditions];
                        next[conditionIndex] = { ...condition, op: event.target.value as RuleOp };
                        updateRule(rule.id, { conditions: next });
                      }}
                      sx={{ minWidth: 110, height: 30, fontSize: 12, borderRadius: R }}
                    >
                      {(Object.keys(RULE_OP_LABELS) as RuleOp[]).map(op => (
                        <MenuItem key={op} value={op} sx={{ fontSize: 13 }}>{RULE_OP_LABELS[op]}</MenuItem>
                      ))}
                    </Select>
                    {condition.op !== 'empty' && condition.op !== 'notEmpty' && (
                      <TextField
                        size="small"
                        placeholder="比较值"
                        value={condition.value}
                        onChange={event => {
                          const next = [...rule.conditions];
                          next[conditionIndex] = { ...condition, value: event.target.value };
                          updateRule(rule.id, { conditions: next });
                        }}
                        sx={{ width: 170, '& input': { fontSize: 12, py: 0.5 } }}
                      />
                    )}
                    {rule.conditions.length > 1 && (
                      <IconButton
                        size="small"
                        onClick={() => updateRule(rule.id, { conditions: rule.conditions.filter((_, i) => i !== conditionIndex) })}
                      >
                        <DeleteOutlineIcon fontSize="small" />
                      </IconButton>
                    )}
                  </Stack>
                ))}
                <Box>
                  <Button
                    size="small"
                    startIcon={<AddIcon fontSize="small" />}
                    onClick={() => updateRule(rule.id, { conditions: [...rule.conditions, { field: rowField, op: 'eq', value: '' }] })}
                    sx={{ borderRadius: R }}
                  >
                    添加条件
                  </Button>
                </Box>
              </Stack>

              <Divider sx={{ my: 1 }} />

              <Stack direction="row" spacing={1.25} alignItems="center" flexWrap="wrap" useFlexGap>
                <Typography variant="caption" color="text.secondary">命中样式</Typography>
                <Select
                  size="small"
                  displayEmpty
                  value={rule.style.color || ''}
                  onChange={event => updateStyle(rule.id, { color: event.target.value || undefined })}
                  sx={{ minWidth: 118, height: 30, fontSize: 12, borderRadius: R }}
                >
                  <MenuItem value="" sx={{ fontSize: 13 }}>默认字色</MenuItem>
                  {COLOR_OPTIONS.map(color => (
                    <MenuItem key={color} value={color} sx={{ fontSize: 13 }}>{colorSwatch(color)}{color}</MenuItem>
                  ))}
                </Select>
                <Select
                  size="small"
                  displayEmpty
                  value={rule.style.bgColor || ''}
                  onChange={event => updateStyle(rule.id, { bgColor: event.target.value || undefined })}
                  sx={{ minWidth: 118, height: 30, fontSize: 12, borderRadius: R }}
                >
                  <MenuItem value="" sx={{ fontSize: 13 }}>默认底色</MenuItem>
                  {FILL_OPTIONS.map(color => (
                    <MenuItem key={color} value={color} sx={{ fontSize: 13 }}>{colorSwatch(color)}{color}</MenuItem>
                  ))}
                </Select>
                <Select
                  size="small"
                  displayEmpty
                  value={rule.style.leftBar || ''}
                  onChange={event => updateStyle(rule.id, { leftBar: event.target.value || undefined })}
                  sx={{ minWidth: 118, height: 30, fontSize: 12, borderRadius: R }}
                >
                  <MenuItem value="" sx={{ fontSize: 13 }}>无左侧色条</MenuItem>
                  {COLOR_OPTIONS.map(color => (
                    <MenuItem key={color} value={color} sx={{ fontSize: 13 }}>{colorSwatch(color)}{color}</MenuItem>
                  ))}
                </Select>
                <FormControlLabel
                  sx={{ m: 0 }}
                  control={<Switch size="small" checked={Boolean(rule.style.bold)} onChange={event => updateStyle(rule.id, { bold: event.target.checked })} />}
                  label={<Typography variant="caption">加粗</Typography>}
                />
                <FormControlLabel
                  sx={{ m: 0 }}
                  control={<Switch size="small" checked={rule.style.italic === true} onChange={event => updateStyle(rule.id, { italic: event.target.checked })} />}
                  label={<Typography variant="caption">斜体</Typography>}
                />
                <Select
                  size="small"
                  displayEmpty
                  value={rule.style.wrap || ''}
                  onChange={event => updateStyle(rule.id, { wrap: (event.target.value || undefined) as CellStyle['wrap'] })}
                  sx={{ minWidth: 108, height: 30, fontSize: 12, borderRadius: R }}
                >
                  <MenuItem value="" sx={{ fontSize: 13 }}>默认换行</MenuItem>
                  <MenuItem value="single" sx={{ fontSize: 13 }}>单行省略</MenuItem>
                  <MenuItem value="wrap" sx={{ fontSize: 13 }}>自动换行</MenuItem>
                  <MenuItem value="clamp2" sx={{ fontSize: 13 }}>最多两行</MenuItem>
                </Select>
              </Stack>
            </Box>
          ))}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Typography variant="caption" color="text.secondary" sx={{ mr: 'auto', ml: 1 }}>
          共 {rules.length} 条规则（上限 20 条）· 当前页 {rows.length} 条记录
        </Typography>
        <Button onClick={addRule} startIcon={<AddIcon />} disabled={rules.length >= 20} sx={{ borderRadius: R }}>新增规则</Button>
        <Button variant="contained" onClick={onClose} sx={{ borderRadius: R }}>完成</Button>
      </DialogActions>
    </Dialog>
  );
};

export default ConditionalFormatDialog;
