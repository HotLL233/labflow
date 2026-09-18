import React from 'react';
import {
  Box,
  Typography,
  Card,
  CardContent,
} from '@mui/material';
import TextFieldsIcon from '@mui/icons-material/TextFields';
import ArrowDropDownCircleIcon from '@mui/icons-material/ArrowDropDownCircle';
import NotesIcon from '@mui/icons-material/Notes';
import NumbersIcon from '@mui/icons-material/Numbers';
import CalendarTodayIcon from '@mui/icons-material/CalendarToday';
import HorizontalRuleIcon from '@mui/icons-material/HorizontalRule';
import TitleIcon from '@mui/icons-material/Title';
import type { FieldDef } from '../types/layout';

interface FieldLibraryProps {
  onAddField: (fieldType: FieldDef['type']) => void;
}

interface LibraryItem {
  type: FieldDef['type'];
  label: string;
  icon: React.ReactNode;
}

const LIBRARY_ITEMS: LibraryItem[] = [
  { type: 'text', label: '文本输入', icon: <TextFieldsIcon sx={{ fontSize: 20 }} /> },
  { type: 'select', label: '下拉选择', icon: <ArrowDropDownCircleIcon sx={{ fontSize: 20 }} /> },
  { type: 'textarea', label: '多行文本', icon: <NotesIcon sx={{ fontSize: 20 }} /> },
  { type: 'number', label: '数字输入', icon: <NumbersIcon sx={{ fontSize: 20 }} /> },
  { type: 'date', label: '日期选择', icon: <CalendarTodayIcon sx={{ fontSize: 20 }} /> },
  { type: 'divider', label: '分割线', icon: <HorizontalRuleIcon sx={{ fontSize: 20 }} /> },
  { type: 'heading', label: '标题', icon: <TitleIcon sx={{ fontSize: 20 }} /> },
];

const FieldLibrary: React.FC<FieldLibraryProps> = ({ onAddField }) => {
  return (
    <Box sx={{ p: 1.5 }}>
      <Typography variant="subtitle2" sx={{ fontWeight: 700, mb: 1.5, color: '#555' }}>
        字段库
      </Typography>
      <Typography variant="caption" sx={{ color: '#999', mb: 1, display: 'block' }}>
        点击添加字段到页面
      </Typography>
      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.75 }}>
        {LIBRARY_ITEMS.map((item) => (
          <Card
            key={item.type}
            elevation={0}
            onClick={() => onAddField(item.type)}
            sx={{
              display: 'flex',
              alignItems: 'center',
              gap: 1,
              p: 1,
              borderRadius: '2px',
              cursor: 'pointer',
              border: '1px solid rgba(0,0,0,0.08)',
              transition: 'all 0.15s ease',
              '&:hover': {
                borderColor: '#1976d2',
                bgcolor: 'rgba(25,118,210,0.04)',
              },
              userSelect: 'none',
            }}
          >
            <Box sx={{ color: '#1976d2', display: 'flex', alignItems: 'center' }}>
              {item.icon}
            </Box>
            <Typography variant="body2" sx={{ fontWeight: 500, color: '#333' }}>
              {item.label}
            </Typography>
          </Card>
        ))}
      </Box>
    </Box>
  );
};

export default FieldLibrary;
