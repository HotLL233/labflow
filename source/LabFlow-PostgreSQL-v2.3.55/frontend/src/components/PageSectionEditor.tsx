import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { Box, IconButton, Switch, Typography, TextField, Dialog, DialogTitle, DialogContent, DialogActions, Button, Snackbar, Alert, Tooltip } from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import VisibilityIcon from '@mui/icons-material/Visibility';
import VisibilityOffIcon from '@mui/icons-material/VisibilityOff';
import type { ReactNode } from 'react';
import { getSetting, updateSetting } from '../api/client';
import { useUser } from '../UserContext';

// ========== Edit Mode Context ==========
interface PageEditContextValue {
  editMode: boolean;
  setEditMode: (v: boolean) => void;
}
const PageEditContext = createContext<PageEditContextValue>({ editMode: false, setEditMode: () => {} });

export const usePageEditMode = () => useContext(PageEditContext);

interface PageEditProviderProps {
  children: ReactNode;
}
export const PageEditProvider: React.FC<PageEditProviderProps> = ({ children }) => {
  const [editMode, setEditMode] = useState(false);
  return (
    <PageEditContext.Provider value={{ editMode, setEditMode }}>
      {children}
    </PageEditContext.Provider>
  );
};

// ========== Global Edit Toggle ==========
export const PageEditToggle: React.FC = () => {
  const { editMode, setEditMode } = usePageEditMode();
  const { user, hasPermission } = useUser();
  const isAdmin = user?.is_admin || hasPermission('manage:settings');
  if (!isAdmin) return null;
  return (
    <Box sx={{ display: 'flex', justifyContent: 'flex-end', mb: 1 }}>
      <Button
        variant={editMode ? 'contained' : 'outlined'}
        size="small"
        startIcon={<EditIcon />}
        onClick={() => setEditMode(!editMode)}
        sx={{ borderRadius: '2px', fontSize: '0.8rem' }}
        color={editMode ? 'warning' : 'primary'}
      >
        {editMode ? '退出编辑' : '✎ 编辑页面'}
      </Button>
    </Box>
  );
};

// ========== Layout Config Types ==========
export interface SectionConfig {
  visible: boolean;
  label?: string;
  color?: string;
  columns?: any[];
  buttonLabel?: string;
  chipLabel?: string;
  actionButtons?: {
    addLabel?: string;
    deleteLabel?: string;
    resetLabel?: string;
  };
}

export interface PageLayout {
  sections: Record<string, SectionConfig>;
}

const R = '2px';

// ========== PageSectionEditor ==========
interface PageSectionEditorProps {
  pageKey: string;       // 页面标识（如 'home', 'sample_entry'）
  sectionKey: string;    // section 标识（如 'brand-title', 'entry-cards'）
  defaultLabel?: string; // 默认显示文本
  defaultVisible?: boolean;
  children: ReactNode;
  /** 可选：编辑后渲染时的额外样式 */
  wrapperSx?: Record<string, any>;
}

export const PageSectionEditor: React.FC<PageSectionEditorProps> = ({
  pageKey,
  sectionKey,
  defaultLabel,
  defaultVisible = true,
  children,
  wrapperSx,
}) => {
  const { editMode } = usePageEditMode();
  const { user, hasPermission } = useUser();
  const isAdmin = user?.is_admin || hasPermission('manage:settings');

  const [layout, setLayout] = useState<PageLayout | null>(null);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);

  // 编辑对话框
  const [labelDialogOpen, setLabelDialogOpen] = useState(false);
  const [editLabel, setEditLabel] = useState('');

  // buttonLabel 编辑
  const [btnLabelDialogOpen, setBtnLabelDialogOpen] = useState(false);
  const [editBtnLabel, setEditBtnLabel] = useState('');

  // chipLabel 编辑
  const [chipLabelDialogOpen, setChipLabelDialogOpen] = useState(false);
  const [editChipLabel, setEditChipLabel] = useState('');

  // actionButtons 编辑
  const [actionBtnsDialogOpen, setActionBtnsDialogOpen] = useState(false);
  const [editActionBtns, setEditActionBtns] = useState({ addLabel: '', deleteLabel: '', resetLabel: '' });

  // 读取布局配置
  useEffect(() => {
    if (!pageKey) return;
    const settingKey = `layout_${pageKey}`;
    getSetting(settingKey)
      .then((res) => {
        if (res.code === 0 && res.data) {
          try {
            const parsed = JSON.parse(res.data.value) as PageLayout;
            if (parsed && parsed.sections) {
              setLayout(parsed);
            }
          } catch {
            // fallback to default
          }
        }
      })
      .catch(() => {});
  }, [pageKey]);

  // 获取当前 section 的配置（合并默认值）
  const sectionConfig = layout?.sections?.[sectionKey];
  const visible = sectionConfig?.visible ?? defaultVisible;
  const label = sectionConfig?.label ?? defaultLabel;

  // 保存整体 layout
  const saveLayout = useCallback(async (updated: PageLayout) => {
    try {
      const settingKey = `layout_${pageKey}`;
      await updateSetting(settingKey, updated);
      setLayout(updated);
      setSnackMsg('已保存');
      setSnackErr(false);
    } catch {
      setSnackMsg('保存失败');
      setSnackErr(true);
    }
  }, [pageKey]);

  // 切换可见性
  const toggleVisibility = useCallback(() => {
    if (!layout) {
      // 首次创建
      const newLayout: PageLayout = {
        sections: { [sectionKey]: { visible: !defaultVisible, label: defaultLabel } },
      };
      saveLayout(newLayout);
    } else {
      const updated = { ...layout, sections: { ...layout.sections } };
      updated.sections[sectionKey] = {
        ...(updated.sections[sectionKey] || { visible: defaultVisible }),
        visible: !visible,
      };
      saveLayout(updated);
    }
  }, [layout, sectionKey, visible, defaultVisible, defaultLabel, saveLayout]);

  // 打开 label 编辑对话框
  const openLabelEditor = useCallback(() => {
    setEditLabel(label || defaultLabel || '');
    setLabelDialogOpen(true);
  }, [label, defaultLabel]);

  // 保存 label
  const saveLabel = useCallback(() => {
    if (!layout) {
      const newLayout: PageLayout = {
        sections: { [sectionKey]: { visible: defaultVisible, label: editLabel } },
      };
      saveLayout(newLayout);
    } else {
      const updated = { ...layout, sections: { ...layout.sections } };
      updated.sections[sectionKey] = {
        ...(updated.sections[sectionKey] || { visible: defaultVisible }),
        label: editLabel,
      };
      saveLayout(updated);
    }
    setLabelDialogOpen(false);
  }, [layout, sectionKey, visible, defaultVisible, editLabel, saveLayout]);

  // buttonLabel 编辑
  const openBtnLabelEditor = useCallback(() => {
    setEditBtnLabel(sectionConfig?.buttonLabel || '');
    setBtnLabelDialogOpen(true);
  }, [sectionConfig]);

  const saveBtnLabel = useCallback(() => {
    if (!layout) {
      const newLayout: PageLayout = {
        sections: { [sectionKey]: { visible: defaultVisible, buttonLabel: editBtnLabel } },
      };
      saveLayout(newLayout);
    } else {
      const updated = { ...layout, sections: { ...layout.sections } };
      updated.sections[sectionKey] = {
        ...(updated.sections[sectionKey] || { visible: defaultVisible }),
        buttonLabel: editBtnLabel,
      };
      saveLayout(updated);
    }
    setBtnLabelDialogOpen(false);
  }, [layout, sectionKey, defaultVisible, editBtnLabel, saveLayout]);

  // chipLabel 编辑
  const openChipLabelEditor = useCallback(() => {
    setEditChipLabel(sectionConfig?.chipLabel || '');
    setChipLabelDialogOpen(true);
  }, [sectionConfig]);

  const saveChipLabel = useCallback(() => {
    if (!layout) {
      const newLayout: PageLayout = {
        sections: { [sectionKey]: { visible: defaultVisible, chipLabel: editChipLabel } },
      };
      saveLayout(newLayout);
    } else {
      const updated = { ...layout, sections: { ...layout.sections } };
      updated.sections[sectionKey] = {
        ...(updated.sections[sectionKey] || { visible: defaultVisible }),
        chipLabel: editChipLabel,
      };
      saveLayout(updated);
    }
    setChipLabelDialogOpen(false);
  }, [layout, sectionKey, defaultVisible, editChipLabel, saveLayout]);

  // actionButtons 编辑
  const openActionBtnsEditor = useCallback(() => {
    const ab = sectionConfig?.actionButtons;
    setEditActionBtns({
      addLabel: ab?.addLabel || '',
      deleteLabel: ab?.deleteLabel || '',
      resetLabel: ab?.resetLabel || '',
    });
    setActionBtnsDialogOpen(true);
  }, [sectionConfig]);

  const saveActionBtns = useCallback(() => {
    if (!layout) {
      const newLayout: PageLayout = {
        sections: { [sectionKey]: { visible: defaultVisible, actionButtons: editActionBtns } },
      };
      saveLayout(newLayout);
    } else {
      const updated = { ...layout, sections: { ...layout.sections } };
      updated.sections[sectionKey] = {
        ...(updated.sections[sectionKey] || { visible: defaultVisible }),
        actionButtons: editActionBtns,
      };
      saveLayout(updated);
    }
    setActionBtnsDialogOpen(false);
  }, [layout, sectionKey, defaultVisible, editActionBtns, saveLayout]);

  // 不渲染隐藏的 section（仅在非编辑模式下隐藏）
  if (!editMode && !visible) {
    return null;
  }

  return (
    <Box sx={{ position: 'relative', ...(wrapperSx || {}) }}>
      {/* 非编辑模式：直接渲染 children */}
      {(!editMode || !isAdmin) && children}

      {/* 编辑模式（管理员）：渲染 children + 编辑覆盖层 */}
      {editMode && isAdmin && (
        <>
          {/* 编辑工具栏 - 右上角 */}
          <Box
            sx={{
              position: 'absolute',
              top: -8,
              right: -8,
              zIndex: 100,
              display: 'flex',
              gap: 0.5,
              bgcolor: 'rgba(255,193,7,0.9)',
              borderRadius: R,
              px: 0.5,
              py: 0.3,
              boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
            }}
          >
            <Tooltip title={visible ? '点击隐藏' : '点击显示'}>
              <IconButton size="small" onClick={toggleVisibility} sx={{ color: visible ? '#e65100' : '#999', p: 0.3 }}>
                {visible ? <VisibilityIcon fontSize="small" /> : <VisibilityOffIcon fontSize="small" />}
              </IconButton>
            </Tooltip>
            {defaultLabel !== undefined && (
              <Tooltip title="编辑文本">
                <IconButton size="small" onClick={openLabelEditor} sx={{ color: '#e65100', p: 0.3 }}>
                  <EditIcon fontSize="small" />
                </IconButton>
              </Tooltip>
            )}
            {sectionConfig?.buttonLabel !== undefined && (
              <Tooltip title="编辑按钮文字">
                <IconButton size="small" onClick={openBtnLabelEditor} sx={{ color: '#e65100', p: 0.3 }}>
                  <EditIcon fontSize="small" />
                </IconButton>
              </Tooltip>
            )}
            {sectionConfig?.chipLabel !== undefined && (
              <Tooltip title="编辑标签">
                <IconButton size="small" onClick={openChipLabelEditor} sx={{ color: '#e65100', p: 0.3 }}>
                  <EditIcon fontSize="small" />
                </IconButton>
              </Tooltip>
            )}
            {sectionConfig?.actionButtons !== undefined && (
              <Tooltip title="编辑按钮文字">
                <IconButton size="small" onClick={openActionBtnsEditor} sx={{ color: '#e65100', p: 0.3 }}>
                  <EditIcon fontSize="small" />
                </IconButton>
              </Tooltip>
            )}
            <Typography variant="caption" sx={{ color: '#e65100', fontWeight: 600, alignSelf: 'center', fontSize: '0.65rem', px: 0.5 }}>
              {sectionKey}
            </Typography>
          </Box>

          {/* 编辑模式边框 */}
          <Box sx={{ outline: visible ? '2px dashed #ff9800' : '2px dashed #e57373', outlineOffset: 2, borderRadius: R, opacity: visible ? 1 : 0.4 }}>
            {children}
          </Box>
        </>
      )}

      {/* Label 编辑对话框 */}
      <Dialog open={labelDialogOpen} onClose={() => setLabelDialogOpen(false)} maxWidth="xs" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle sx={{ fontWeight: 700, fontSize: '1rem' }}>编辑文本</DialogTitle>
        <DialogContent>
          <TextField
            autoFocus
            fullWidth
            size="small"
            value={editLabel}
            onChange={e => setEditLabel(e.target.value)}
            placeholder="输入显示文本"
            sx={{ mt: 1, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setLabelDialogOpen(false)} size="small" sx={{ borderRadius: R }}>取消</Button>
          <Button onClick={saveLabel} variant="contained" size="small" sx={{ borderRadius: R }}>保存</Button>
        </DialogActions>
      </Dialog>

      {/* buttonLabel 编辑对话框 */}
      <Dialog open={btnLabelDialogOpen} onClose={() => setBtnLabelDialogOpen(false)} maxWidth="xs" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle sx={{ fontWeight: 700, fontSize: '1rem' }}>编辑按钮文字</DialogTitle>
        <DialogContent>
          <TextField
            autoFocus
            fullWidth
            size="small"
            value={editBtnLabel}
            onChange={e => setEditBtnLabel(e.target.value)}
            placeholder="输入按钮文字"
            sx={{ mt: 1, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setBtnLabelDialogOpen(false)} size="small" sx={{ borderRadius: R }}>取消</Button>
          <Button onClick={saveBtnLabel} variant="contained" size="small" sx={{ borderRadius: R }}>保存</Button>
        </DialogActions>
      </Dialog>

      {/* chipLabel 编辑对话框 */}
      <Dialog open={chipLabelDialogOpen} onClose={() => setChipLabelDialogOpen(false)} maxWidth="xs" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle sx={{ fontWeight: 700, fontSize: '1rem' }}>编辑标签</DialogTitle>
        <DialogContent>
          <TextField
            autoFocus
            fullWidth
            size="small"
            value={editChipLabel}
            onChange={e => setEditChipLabel(e.target.value)}
            placeholder="输入标签文字"
            sx={{ mt: 1, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setChipLabelDialogOpen(false)} size="small" sx={{ borderRadius: R }}>取消</Button>
          <Button onClick={saveChipLabel} variant="contained" size="small" sx={{ borderRadius: R }}>保存</Button>
        </DialogActions>
      </Dialog>

      {/* actionButtons 编辑对话框 */}
      <Dialog open={actionBtnsDialogOpen} onClose={() => setActionBtnsDialogOpen(false)} maxWidth="xs" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle sx={{ fontWeight: 700, fontSize: '1rem' }}>编辑按钮文字</DialogTitle>
        <DialogContent>
          <TextField
            fullWidth
            size="small"
            label="添加行"
            value={editActionBtns.addLabel}
            onChange={e => setEditActionBtns(p => ({ ...p, addLabel: e.target.value }))}
            sx={{ mt: 1, mb: 1.5, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
          <TextField
            fullWidth
            size="small"
            label="删除选中"
            value={editActionBtns.deleteLabel}
            onChange={e => setEditActionBtns(p => ({ ...p, deleteLabel: e.target.value }))}
            sx={{ mb: 1.5, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
          <TextField
            fullWidth
            size="small"
            label="重置"
            value={editActionBtns.resetLabel}
            onChange={e => setEditActionBtns(p => ({ ...p, resetLabel: e.target.value }))}
            sx={{ '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setActionBtnsDialogOpen(false)} size="small" sx={{ borderRadius: R }}>取消</Button>
          <Button onClick={saveActionBtns} variant="contained" size="small" sx={{ borderRadius: R }}>保存</Button>
        </DialogActions>
      </Dialog>

      {/* Snackbar */}
      <Snackbar open={!!snackMsg} autoHideDuration={2000} onClose={() => setSnackMsg('')} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snackErr ? 'error' : 'success'} sx={{ borderRadius: R }} onClose={() => setSnackMsg('')}>{snackMsg}</Alert>
      </Snackbar>
    </Box>
  );
};

export default PageSectionEditor;
