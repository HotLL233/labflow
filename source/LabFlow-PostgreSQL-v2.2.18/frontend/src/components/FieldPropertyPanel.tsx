import React from 'react';
import {
  Box,
  Typography,
  TextField,
  Switch,
  FormControlLabel,
  Slider,
  Button,
  Divider,
} from '@mui/material';
import DeleteIcon from '@mui/icons-material/Delete';
import type { FieldDef } from '../types/layout';

interface FieldPropertyPanelProps {
  field: FieldDef;
  onChange: (updated: FieldDef) => void;
  onDelete: () => void;
}

const FieldPropertyPanel: React.FC<FieldPropertyPanelProps> = ({
  field,
  onChange,
  onDelete,
}) => {
  const update = (patch: Partial<FieldDef>) => {
    onChange({ ...field, ...patch });
  };

  return (
    <Box sx={{ p: 1.5 }}>
      <Typography variant="subtitle2" sx={{ fontWeight: 700, mb: 1.5, color: '#555' }}>
        字段属性
      </Typography>
      <Typography variant="caption" sx={{ color: '#999', mb: 2, display: 'block' }}>
        键: {field.key}
      </Typography>

      {/* Label */}
      <Box sx={{ mb: 1.5 }}>
        <Typography variant="caption" sx={{ color: '#666', mb: 0.25, display: 'block' }}>
          标签
        </Typography>
        <TextField
          size="small"
          fullWidth
          value={field.label}
          onChange={(e) => update({ label: e.target.value })}
          sx={{ '& .MuiOutlinedInput-root': { borderRadius: '2px', fontSize: '0.8rem' } }}
        />
      </Box>

      {/* Width */}
      <Box sx={{ mb: 1.5 }}>
        <Typography variant="caption" sx={{ color: '#666', mb: 0.25, display: 'block' }}>
          宽度: {field.width}px
        </Typography>
        <Slider
          size="small"
          min={50}
          max={500}
          value={field.width}
          onChange={(_e, v) => update({ width: v as number })}
          sx={{ mt: 1 }}
        />
      </Box>

      {/* Placeholder */}
      {(field.type === 'text' || field.type === 'textarea' || field.type === 'number') && (
        <Box sx={{ mb: 1.5 }}>
          <Typography variant="caption" sx={{ color: '#666', mb: 0.25, display: 'block' }}>
            占位符
          </Typography>
          <TextField
            size="small"
            fullWidth
            value={field.placeholder || ''}
            onChange={(e) => update({ placeholder: e.target.value })}
            sx={{ '& .MuiOutlinedInput-root': { borderRadius: '2px', fontSize: '0.8rem' } }}
          />
        </Box>
      )}

      <Divider sx={{ my: 1.5 }} />

      {/* Toggle switches */}
      <FormControlLabel
        control={
          <Switch
            size="small"
            checked={field.required}
            onChange={(e) => update({ required: e.target.checked })}
          />
        }
        label={<Typography variant="caption">必填</Typography>}
        sx={{ mb: 0.5 }}
      />
      <FormControlLabel
        control={
          <Switch
            size="small"
            checked={field.visible}
            onChange={(e) => update({ visible: e.target.checked })}
          />
        }
        label={<Typography variant="caption">可见</Typography>}
        sx={{ mb: 1.5 }}
      />

      <Divider sx={{ my: 1.5 }} />

      {/* Delete button */}
      <Button
        variant="outlined"
        color="error"
        fullWidth
        size="small"
        startIcon={<DeleteIcon />}
        onClick={onDelete}
        sx={{ borderRadius: '2px', fontSize: '0.8rem' }}
      >
        删除此字段
      </Button>
    </Box>
  );
};

export default FieldPropertyPanel;
