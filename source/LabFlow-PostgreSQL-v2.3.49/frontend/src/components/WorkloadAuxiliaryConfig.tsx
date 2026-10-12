import { useState } from 'react';
import {
  Alert, Box, Button, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, TextField, Typography,
} from '@mui/material';
import { client, requireApiSuccess } from '../api/http';
import type { ApiResponse } from '../types';
import { useUser } from '../UserContext';

interface AuxiliaryOption {
  key: string;
  name: string;
  kind: 'method' | 'item';
  active: boolean;
}

interface Configuration {
  options: AuxiliaryOption[];
  mapping: Record<string, string>;
}

export default function WorkloadAuxiliaryConfig({ onSaved }: { onSaved: () => void }) {
  const { user } = useUser();
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [options, setOptions] = useState<AuxiliaryOption[]>([]);
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [search, setSearch] = useState('');

  if (!user?.is_admin) return null;

  const load = async () => {
    setOpen(true);
    setLoading(true);
    setLoaded(false);
    setError('');
    setSearch('');
    try {
      const response = await client.get<ApiResponse<Configuration>>('/stats/workload-auxiliary-config');
      const data = requireApiSuccess(response.data);
      if (!data) throw new Error('辅助工作配置未返回数据');
      setOptions(data.options);
      setMapping(data.mapping);
      setLoaded(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : '读取辅助工作分类失败');
    } finally {
      setLoading(false);
    }
  };

  const save = async () => {
    setSaving(true);
    setError('');
    try {
      const response = await client.put<ApiResponse<Configuration>>('/stats/workload-auxiliary-config', { mapping });
      const data = requireApiSuccess(response.data);
      if (!data) throw new Error('保存配置未返回数据');
      setMapping(data.mapping);
      setOpen(false);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : '保存辅助工作分类失败');
    } finally {
      setSaving(false);
    }
  };

  const visibleOptions = options.filter(option => `${option.name} ${mapping[option.key] || ''}`.includes(search.trim()));

  return <>
    <Button size="small" variant="outlined" onClick={load}>辅助子类型配置</Button>
    <Dialog open={open} onClose={() => { if (!saving && !loading) setOpen(false); }} fullWidth maxWidth="md">
      <DialogTitle>辅助工作子类型配置</DialogTitle>
      <DialogContent>
        <Alert severity="info" sx={{ mb: 2 }}>
          管理员配置对所有用户生效。同一子类型可绑定多个方法或事项，留空归为“未分类”。新工作记录保存分类快照；历史无快照记录按当前配置展示。
        </Alert>
        {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}
        {loading ? <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={28} /></Box> : <>
          <TextField label="搜索方法、事项或子类型" value={search} onChange={event => setSearch(event.target.value)} fullWidth size="small" sx={{ mb: 1 }} />
          <TableContainer sx={{ maxHeight: '55vh' }}>
            <Table size="small" stickyHeader sx={{ tableLayout: 'fixed', '& td': { overflowWrap: 'anywhere', verticalAlign: 'top' } }}>
              <TableHead><TableRow>
                <TableCell sx={{ width: '42%' }}>辅助方法 / 事项</TableCell>
                <TableCell sx={{ width: '16%' }}>来源</TableCell>
                <TableCell sx={{ width: '42%' }}>辅助子类型</TableCell>
              </TableRow></TableHead>
              <TableBody>{visibleOptions.map(option => <TableRow key={option.key}>
                <TableCell>
                  <Typography variant="body2">{option.name}</Typography>
                  {!option.active && <Typography variant="caption" color="text.secondary">已停用或归档</Typography>}
                </TableCell>
                <TableCell>{option.kind === 'method' ? '方法' : '事项'}</TableCell>
                <TableCell>
                  <TextField
                    size="small" fullWidth placeholder="未分类" value={mapping[option.key] || ''} disabled={saving}
                    inputProps={{ maxLength: 64, 'aria-label': `${option.name}的辅助子类型` }}
                    onChange={event => setMapping(current => ({ ...current, [option.key]: event.target.value }))}
                  />
                </TableCell>
              </TableRow>)}</TableBody>
            </Table>
          </TableContainer>
          {!visibleOptions.length && <Typography color="text.secondary" sx={{ py: 3, textAlign: 'center' }}>没有匹配的辅助方法或事项</Typography>}
        </>}
      </DialogContent>
      <DialogActions>
        <Button onClick={() => setOpen(false)} disabled={saving || loading}>取消</Button>
        <Button variant="contained" onClick={save} disabled={!loaded || loading || saving}>{saving ? '保存中…' : '保存配置'}</Button>
      </DialogActions>
    </Dialog>
  </>;
}
