import React, { useState, useEffect, useCallback } from 'react';
import { Box, Button, Snackbar, Alert } from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import type { ReactNode } from 'react';
import type { FieldDef } from '../types/layout';
import { getSetting, updateSetting } from '../api/client';
import { useUser } from '../UserContext';
import LayoutDesigner from './LayoutDesigner';

interface EditablePageShellProps {
  pageKey: string; // 'sample-entry', 'home' 等
  children: ReactNode;
  defaultFields: FieldDef[];
  renderField: (
    field: FieldDef,
    isEditMode: boolean,
    onUpdate: (f: FieldDef) => void
  ) => ReactNode;
  onFieldsLoaded?: (fields: FieldDef[]) => void;
}

const R = '2px';

const EditablePageShell: React.FC<EditablePageShellProps> = ({
  pageKey,
  children,
  defaultFields,
  renderField,
  onFieldsLoaded,
}) => {
  const { user, hasPermission } = useUser();
  const isAdmin = user?.is_admin || hasPermission('manage:settings');

  const [fields, setFields] = useState<FieldDef[]>(defaultFields);
  const [editMode, setEditMode] = useState(false);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);

  // Load layout from server
  useEffect(() => {
    const settingKey = `layout_${pageKey}`;
    getSetting(settingKey)
      .then((res) => {
        if (res.code === 0 && res.data) {
          try {
            const parsed = JSON.parse(res.data.value) as FieldDef[];
            if (Array.isArray(parsed) && parsed.length > 0) {
              setFields(parsed);
              onFieldsLoaded?.(parsed);
            }
          } catch {
            // fallback to default
          }
        }
      })
      .catch(() => {
        // fallback to default
      });
  }, [pageKey, onFieldsLoaded]);

  const handleSave = useCallback(
    async (newFields: FieldDef[]) => {
      const settingKey = `layout_${pageKey}`;
      try {
        // v0.4.42: 用 Axios 走拦截器，自动附 token（fetch 不会自动加）
        const saved = await updateSetting(settingKey, newFields);
        if (saved.code !== 0) {
          throw new Error(saved.message || '服务端返回错误');
        }
        // 更新本地状态
        setFields(newFields);
        onFieldsLoaded?.(newFields);
        setEditMode(false);
        setSnackMsg('✅ 布局已保存，页面即将刷新');
        setSnackErr(false);
        // 稍后刷新使所有组件重新读取最新配置
        setTimeout(() => { try { window.location.reload(); } catch {} }, 1800);
      } catch (e: any) {
        setEditMode(false); // 先关编辑器，让用户看到错误提示
        const msg = e?.message || String(e) || '未知错误';
        setSnackMsg('❌ 保存失败: ' + msg);
        setSnackErr(true);
        console.error('[EditablePageShell] save error:', e);
      }
    },
    [pageKey, onFieldsLoaded]
  );

  return (
    <>
      {/* 编辑模式开关（仅管理员可见） */}
      {isAdmin && !editMode && (
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', mb: 1 }}>
          <Button
            variant="outlined"
            size="small"
            startIcon={<EditIcon />}
            onClick={() => setEditMode(true)}
            sx={{ borderRadius: R, fontSize: '0.8rem' }}
          >
            ✎ 编辑布局
          </Button>
        </Box>
      )}

      {/* 页面内容 */}
      {children}

      {/* 布局编辑器（全屏覆盖） */}
      {editMode && (
        <LayoutDesigner
          fields={fields}
          onChange={setFields}
          onSave={handleSave}
          onClose={() => setEditMode(false)}
          renderFieldPreview={(field) => renderField(field, true, () => {})}
        />
      )}

      {/* Snackbar */}
      <Snackbar
        open={!!snackMsg}
        autoHideDuration={8000}
        onClose={() => setSnackMsg('')}
        anchorOrigin={{ vertical: 'top', horizontal: 'center' }}
        sx={{ zIndex: 10001, mt: 6 }}
      >
        <Alert
          severity={snackErr ? 'error' : 'success'}
          sx={{ borderRadius: R }}
          onClose={() => setSnackMsg('')}
        >
          {snackMsg}
        </Alert>
      </Snackbar>
    </>
  );
};

export default EditablePageShell;
