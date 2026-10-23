import { useEffect, useState } from 'react';
import {
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Paper,
  Typography,
} from '@mui/material';
import DownloadIcon from '@mui/icons-material/Download';
import { downloadUserImportTemplate, importUsers, userList } from '../api/client';
import type { User } from '../types';

type UserImportResult = {
  created?: number;
  updated?: number;
  skipped?: number;
  roles_mapped?: number;
  roles_ignored?: number;
  users_without_roles?: number;
  modules?: {
    rd?: { created?: number; updated?: number; skipped?: number };
    work?: { created?: number; updated?: number; skipped?: number };
  };
  warnings?: string[];
  errors?: string[];
};

type Props = {
  open: boolean;
  onClose: () => void;
  onMessage: (message: string, isError?: boolean) => void;
  onUsersUpdated: (users: User[]) => void;
};

const R = '2px';

const UserImportDialog = ({ open, onClose, onMessage, onUsersUpdated }: Props) => {
  const [file, setFile] = useState<File | null>(null);
  const [updateExisting, setUpdateExisting] = useState(false);
  const [result, setResult] = useState<UserImportResult | null>(null);

  useEffect(() => {
    if (!open) return;
    setFile(null);
    setUpdateExisting(false);
    setResult(null);
  }, [open]);

  const close = () => {
    setFile(null);
    setUpdateExisting(false);
    setResult(null);
    onClose();
  };

  const submit = async () => {
    if (!file) return;
    try {
      const response = await importUsers(file, updateExisting);
      if (response.code !== 0) {
        onMessage(response.message, true);
        return;
      }
      setResult(response.data as UserImportResult);
      const users = await userList();
      if (users.code === 0 && users.data) onUsersUpdated(users.data);
    } catch {
      onMessage('导入失败', true);
    }
  };

  return (
    <Dialog open={open} onClose={close} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>批量导入用户</DialogTitle>
      <DialogContent>
        <Box sx={{ mb: 2 }}>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            ① 下载系统生成的 Excel 模板。一个文件内包含“研发送样用户导入”和“分析检测用户导入”两个工作表。
          </Typography>
          <Button
            variant="outlined"
            size="small"
            startIcon={<DownloadIcon />}
            onClick={async () => {
              try {
                await downloadUserImportTemplate();
                onMessage('用户导入模板已下载');
              } catch (error) {
                onMessage(error instanceof Error ? error.message : '模板下载失败', true);
              }
            }}
            sx={{ borderRadius: R }}
          >
            下载用户导入模板（.xlsx）
          </Button>
        </Box>
        <Box sx={{ mb: 2 }}>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            ② 分别填写两个工作表后上传。研发送样用户只填写一个归属部门；分析检测用户的数据范围由角色配置决定，不填写业务范围部门和实验室。
          </Typography>
          <Button variant="outlined" component="label" sx={{ borderRadius: R }}>
            选择文件
            <input
              type="file"
              hidden
              accept=".xlsx"
              onChange={event => {
                setFile(event.target.files?.[0] || null);
                setResult(null);
                event.target.value = '';
              }}
            />
          </Button>
          {file && (
            <Chip
              label={file.name}
              size="small"
              color="primary"
              variant="outlined"
              sx={{ ml: 1, borderRadius: R }}
              onDelete={() => setFile(null)}
            />
          )}
        </Box>
        <FormControlLabel
          control={<Checkbox checked={updateExisting} onChange={event => setUpdateExisting(event.target.checked)} />}
          label="按所属工作表更新已有用户的归属信息与角色（不修改密码）"
        />
        {result && (
          <Paper elevation={0} sx={{ p: 2, bgcolor: '#f5f9f5', borderRadius: R, border: '1px solid rgba(0,0,0,0.06)' }}>
            <Typography variant="body2" sx={{ color: '#2e7d32' }}>✓ 成功导入 {result.created || 0} 条</Typography>
            {(result.updated || 0) > 0 && <Typography variant="body2" sx={{ color: '#1976d2' }}>✓ 更新已有用户 {result.updated} 条</Typography>}
            {(result.skipped || 0) > 0 && <Typography variant="body2" sx={{ color: '#f57c00' }}>⚠ 跳过 {result.skipped} 条</Typography>}
            {result.modules && <>
              <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>研发送样：成功 {result.modules.rd?.created || 0} 条，更新 {result.modules.rd?.updated || 0} 条，跳过 {result.modules.rd?.skipped || 0} 条</Typography>
              <Typography variant="body2" color="text.secondary">分析检测：成功 {result.modules.work?.created || 0} 条，更新 {result.modules.work?.updated || 0} 条，跳过 {result.modules.work?.skipped || 0} 条</Typography>
            </>}
            <Typography variant="body2" color="text.secondary">已映射角色 {result.roles_mapped || 0} 个，未匹配/无权限角色 {result.roles_ignored || 0} 个，无角色用户 {result.users_without_roles || 0} 个</Typography>
            {(result.warnings || []).map((message, index) => <Typography key={`warning-${index}`} variant="caption" display="block" color="warning.main">{message}</Typography>)}
            {(result.errors || []).map((message, index) => <Typography key={`error-${index}`} variant="caption" display="block" color="error">{message}</Typography>)}
          </Paper>
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={close} sx={{ borderRadius: R }}>关闭</Button>
        <Button variant="contained" disabled={!file} onClick={submit} sx={{ borderRadius: R, bgcolor: '#2e7d32' }}>开始导入</Button>
      </DialogActions>
    </Dialog>
  );
};

export default UserImportDialog;
