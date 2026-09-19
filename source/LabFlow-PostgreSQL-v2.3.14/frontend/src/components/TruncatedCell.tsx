import React from 'react';
import { Tooltip, Box } from '@mui/material';

interface Props {
  value: React.ReactNode;
  maxWidth?: number;
  title?: string;
}

const R = '2px';

const TruncatedCell: React.FC<Props> = ({ value, maxWidth = 140, title }) => {
  if (value == null || value === '') return <>-</>;
  // 如果是 ReactNode（非纯字符串），直接包裹 truncation 即可
  if (typeof value !== 'string' && typeof value !== 'number') {
    return (
      <Box sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth }}>
        {value}
      </Box>
    );
  }
  const str = String(value);
  // 短字符串直接显示
  if (str.length <= Math.floor(maxWidth / 8)) {
    return (
      <Box sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth }}>
        {str}
      </Box>
    );
  }
  return (
    <Tooltip title={title || str} placement="top" arrow disableInteractive>
      <Box sx={{
        overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        maxWidth, cursor: 'help', display: 'inline-block', verticalAlign: 'middle',
        borderRadius: R, px: 0.5,
      }}>
        {str}
      </Box>
    </Tooltip>
  );
};

export default TruncatedCell;
