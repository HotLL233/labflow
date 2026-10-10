import type { ReactNode } from 'react';
import { Box, Typography } from '@mui/material';
import { cardSummaryGroups, type CardLevelStyle } from '../../utils/recordTableStyle';

export interface RecordCardSummaryField {
  key: string;
  label: string;
  value: ReactNode;
}

/** 调用页只传允许展示的字段；摘要不改变字段可见性和操作权限。 */
export default function RecordCardSummary({ card, fields, defaultRows }: {
  card: CardLevelStyle;
  fields: RecordCardSummaryField[];
  defaultRows: string[][];
}) {
  const available = new Map(fields.filter(field => field.value != null && field.value !== false
    && !(typeof field.value === 'string' && !field.value.trim())).map(field => [field.key, field]));
  return <Box sx={{ display: 'grid', gap: 0.4, py: 0.5, minWidth: 0 }}>
    {cardSummaryGroups(card, defaultRows).map((keys, index) => {
      const row = keys.map(key => available.get(key)).filter((field): field is RecordCardSummaryField => Boolean(field));
      if (!row.length) return null;
      return <Box key={index} data-summary-row={index + 1} sx={{ display: 'flex', flexWrap: 'wrap', gap: '2px 10px', minWidth: 0 }}>
        {row.map(field => <Typography component="span" variant="body2" key={field.key} sx={{ minWidth: 0, maxWidth: '100%', whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word', lineHeight: 1.45 }}>
          {field.label && <Box component="span" sx={{ color: 'text.secondary', fontSize: '0.72rem' }}>{field.label}：</Box>}{field.value}
        </Typography>)}
      </Box>;
    })}
  </Box>;
}
