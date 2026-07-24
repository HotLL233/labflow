import React, { useState, useCallback } from 'react';
import {
  Box,
  Typography,
  Paper,
  IconButton,
  Button,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import EditIcon from '@mui/icons-material/Edit';
import SaveIcon from '@mui/icons-material/Save';
import type { FieldDef } from '../types/layout';
import EditableField from './EditableField';
import FieldLibrary from './FieldLibrary';
import FieldPropertyPanel from './FieldPropertyPanel';

interface LayoutDesignerProps {
  fields: FieldDef[];
  onChange: (fields: FieldDef[]) => void;
  onSave: (fields: FieldDef[]) => void;
  onClose: () => void;
  renderFieldPreview: (field: FieldDef) => React.ReactNode;
}

const R = '2px';

const LayoutDesigner: React.FC<LayoutDesignerProps> = ({
  fields,
  onChange,
  onSave,
  onClose,
  renderFieldPreview,
}) => {
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  const visibleFields = fields.filter((f) => f.visible);

  const selectedField = fields.find((f) => f.key === selectedKey) ?? null;

  const handleFieldSelect = useCallback((key: string) => {
    setSelectedKey((prev) => (prev === key ? null : key));
  }, []);

  const handleFieldDelete = useCallback(
    (key: string) => {
      const updated = fields.filter((f) => f.key !== key);
      onChange(updated);
      if (selectedKey === key) setSelectedKey(null);
    },
    [fields, onChange, selectedKey]
  );

  const handlePropertyChange = useCallback(
    (updated: FieldDef) => {
      const idx = fields.findIndex((f) => f.key === updated.key);
      if (idx === -1) return;
      const newFields = [...fields];
      newFields[idx] = updated;
      onChange(newFields);
    },
    [fields, onChange]
  );

  const handleAddField = useCallback(
    (fieldType: FieldDef['type']) => {
      const newKey = `field_${Date.now()}`;
      const newField: FieldDef = {
        key: newKey,
        type: fieldType,
        label: getDefaultLabel(fieldType),
        width: 150,
        required: false,
        visible: true,
        sort_order: fields.length + 1,
        placeholder: '',
      };
      const updated = [...fields, newField];
      onChange(updated);
      setSelectedKey(newKey);
    },
    [fields, onChange]
  );

  const handleSave = useCallback(() => {
    onSave(fields);
  }, [fields, onSave]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  }, []);

  // Simple reorder via drag/drop on drag handles
  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const fieldKey = e.dataTransfer.getData('text/field-key');
      if (!fieldKey) return;
      // Move field to end (simple approach)
      const idx = fields.findIndex((f) => f.key === fieldKey);
      if (idx === -1) return;
      const newFields = [...fields];
      const [moved] = newFields.splice(idx, 1);
      newFields.push(moved);
      // Update sort_order
      const reordered = newFields.map((f, i) => ({ ...f, sort_order: i + 1 }));
      onChange(reordered);
    },
    [fields, onChange]
  );

  return (
    <Box
      sx={{
        position: 'fixed',
        inset: 0,
        zIndex: 9999,
        display: 'flex',
        flexDirection: 'column',
        bgcolor: 'rgba(0,0,0,0.5)',
      }}
    >
      {/* 顶部工具栏 */}
      <Paper
        elevation={2}
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 2,
          py: 1,
          borderRadius: 0,
          bgcolor: '#fff',
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <EditIcon sx={{ fontSize: 20, color: '#1976d2' }} />
          <Typography variant="subtitle1" sx={{ fontWeight: 700 }}>
            编辑页面布局
          </Typography>
        </Box>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <Button
            variant="contained"
            size="small"
            startIcon={<SaveIcon />}
            onClick={handleSave}
            sx={{ borderRadius: R, fontSize: '0.8rem' }}
          >
            发布布局
          </Button>
          <Button
            variant="outlined"
            size="small"
            onClick={onClose}
            sx={{ borderRadius: R, fontSize: '0.8rem' }}
          >
            完成编辑
          </Button>
          <IconButton size="small" onClick={onClose} sx={{ color: '#999' }}>
            <CloseIcon />
          </IconButton>
        </Box>
      </Paper>

      {/* 三栏内容区 */}
      <Box
        sx={{
          flexGrow: 1,
          display: 'grid',
          gridTemplateColumns: '240px 1fr 280px',
          gap: 0,
          overflow: 'hidden',
        }}
      >
        {/* 左栏：字段库 */}
        <Paper
          elevation={1}
          sx={{
            borderRadius: 0,
            overflow: 'auto',
            bgcolor: '#fafafa',
            borderRight: '1px solid rgba(0,0,0,0.08)',
          }}
        >
          <FieldLibrary onAddField={handleAddField} />
        </Paper>

        {/* 中栏：页面预览 */}
        <Paper
          elevation={1}
          sx={{
            borderRadius: 0,
            overflow: 'auto',
            bgcolor: '#f5f5f5',
            p: 2,
          }}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
        >
          <Paper
            elevation={0}
            sx={{
              p: 2,
              bgcolor: '#fff',
              borderRadius: R,
              minHeight: '100%',
            }}
          >
            {visibleFields.length === 0 ? (
              <Typography
                variant="body2"
                color="text.secondary"
                sx={{ textAlign: 'center', py: 4 }}
              >
                暂无可见字段。从左栏添加字段。
              </Typography>
            ) : (
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
                {visibleFields.map((field) => (
                  <EditableField
                    key={field.key}
                    fieldKey={field.key}
                    isEditMode={true}
                    selected={selectedKey === field.key}
                    onSelect={() => handleFieldSelect(field.key)}
                    onDelete={() => handleFieldDelete(field.key)}
                  >
                    {renderFieldPreview(field)}
                  </EditableField>
                ))}
              </Box>
            )}
          </Paper>
        </Paper>

        {/* 右栏：属性面板 */}
        <Paper
          elevation={1}
          sx={{
            borderRadius: 0,
            overflow: 'auto',
            bgcolor: '#fafafa',
            borderLeft: '1px solid rgba(0,0,0,0.08)',
          }}
        >
          {selectedField ? (
            <FieldPropertyPanel
              field={selectedField}
              onChange={handlePropertyChange}
              onDelete={() => handleFieldDelete(selectedField.key)}
            />
          ) : (
            <Box sx={{ p: 2, textAlign: 'center' }}>
              <Typography variant="body2" color="text.secondary">
                点击选择字段以编辑属性
              </Typography>
            </Box>
          )}
        </Paper>
      </Box>
    </Box>
  );
};

function getDefaultLabel(type: FieldDef['type']): string {
  const map: Record<FieldDef['type'], string> = {
    text: '文本字段',
    select: '下拉选择',
    textarea: '多行文本',
    number: '数字',
    date: '日期',
    datetime: '日期时间',
    divider: '分割线',
    heading: '标题',
  };
  return map[type] || '新字段';
}

export default LayoutDesigner;
