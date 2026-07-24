import React, { useEffect, useState } from 'react';
import {
  Box, Paper, Typography, Table, TableHead, TableRow, TableCell, TableBody, IconButton,
  TextField, Button, Snackbar, Alert, Chip, Switch, FormControlLabel,
} from '@mui/material';
import SaveIcon from '@mui/icons-material/Save';
import { getRdRecordColumns, updateRdRecordColumn } from '../api/client';
import type { RdRecordColumn } from '../types';

const R = '2px';

const AdminRdRecordColumns: React.FC = () => {
  const [cols, setCols] = useState<RdRecordColumn[]>([]);
  const [msg, setMsg] = useState('');
  const [err, setErr] = useState(false);
  const [editWidth, setEditWidth] = useState<Record<number, number>>({});
  const [editShowList, setEditShowList] = useState<Record<number, boolean>>({});
  const [editShowForm, setEditShowForm] = useState<Record<number, boolean>>({});
  const [saving, setSaving] = useState<Record<number, boolean>>({});

  const load = () => {
    getRdRecordColumns().then(r => {
      if (r.code === 0 && r.data) {
        setCols(r.data);
        const w: Record<number, number> = {};
        const sl: Record<number, boolean> = {};
        const sf: Record<number, boolean> = {};
        r.data.forEach(c => { w[c.id] = c.width; sl[c.id] = c.show_in_list; sf[c.id] = c.show_in_form; });
        setEditWidth(w);
        setEditShowList(sl);
        setEditShowForm(sf);
      }
    }).catch(() => setErr(true));
  };

  useEffect(() => { load(); }, []);

  const save = async (col: RdRecordColumn) => {
    setSaving(p => ({ ...p, [col.id]: true }));
    try {
      const r = await updateRdRecordColumn(col.id, {
        width: editWidth[col.id] ?? col.width,
        show_in_list: editShowList[col.id] ?? col.show_in_list,
        show_in_form: editShowForm[col.id] ?? col.show_in_form,
      });
      if (r.code === 0) {
        setMsg('保存成功');
        load();
      } else {
        setErr(true); setMsg(r.message);
      }
    } catch (e: any) {
      setErr(true); setMsg(e.message || '保存失败');
    } finally {
      setSaving(p => ({ ...p, [col.id]: false }));
    }
  };

  return (
    <Box>
      <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 2 }}>研发送样列配置</Typography>
      <Paper elevation={0} sx={{ borderRadius: R, border: '1px solid rgba(0,0,0,0.08)', overflow: 'auto' }}>
        <Table size="small">
          <TableHead>
            <TableRow sx={{ bgcolor: '#f5f5f5' }}>
              <TableCell sx={{ fontWeight: 700 }}>列名</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>显示名称</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>列宽</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>列表显示</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>表单显示</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>操作</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {cols.map(col => (
              <TableRow key={col.id} hover>
                <TableCell sx={{ fontSize: '0.85rem' }}>{col.name}</TableCell>
                <TableCell sx={{ fontSize: '0.85rem' }}>{col.label}</TableCell>
                <TableCell sx={{ p: 0.5 }}>
                  <TextField
                    size="small"
                    type="number"
                    value={editWidth[col.id] ?? col.width}
                    onChange={e => setEditWidth(p => ({ ...p, [col.id]: Math.max(30, Number(e.target.value) || 50) }))}
                    inputProps={{ min: 30, max: 500, step: 10, style: { padding: '2px 6px', width: 70 } }}
                    sx={{ '& .MuiOutlinedInput-root': { fontSize: '0.8rem', borderRadius: R } }}
                  />
                </TableCell>
                <TableCell sx={{ p: 0.5 }}>
                  <Switch
                    size="small"
                    checked={editShowList[col.id] ?? col.show_in_list}
                    onChange={e => setEditShowList(p => ({ ...p, [col.id]: e.target.checked }))}
                  />
                </TableCell>
                <TableCell sx={{ p: 0.5 }}>
                  <Switch
                    size="small"
                    checked={editShowForm[col.id] ?? col.show_in_form}
                    onChange={e => setEditShowForm(p => ({ ...p, [col.id]: e.target.checked }))}
                  />
                </TableCell>
                <TableCell sx={{ p: 0.5 }}>
                  <IconButton size="small" color="primary" onClick={() => save(col)} disabled={saving[col.id]}>
                    <SaveIcon fontSize="small" />
                  </IconButton>
                </TableCell>
              </TableRow>
            ))}
            {cols.length === 0 && (
              <TableRow><TableCell colSpan={6} align="center" sx={{ color: '#999', py: 3 }}>暂无列配置</TableCell></TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>
      <Snackbar open={!!msg} autoHideDuration={2500} onClose={() => setMsg('')}>
        <Alert severity={err ? 'error' : 'success'} onClose={() => setMsg('')}>{msg}</Alert>
      </Snackbar>
    </Box>
  );
};

export default AdminRdRecordColumns;
