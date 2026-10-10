import type { ReactNode } from 'react';
import { Box, Typography } from '@mui/material';
import { cardFieldSx, cardSummaryGroups, DEFAULT_RECORD_TABLE_STYLE, type CardLevelStyle } from '../../utils/recordTableStyle';

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
  const fieldSx = cardFieldSx({ ...DEFAULT_RECORD_TABLE_STYLE, card });
  return <Box sx={{ display: 'grid', gap: 0.5, py: 0.5, minWidth: 0 }}>
    {cardSummaryGroups(card, defaultRows).map((keys, index) => {
      const row = keys.map(key => available.get(key)).filter((field): field is RecordCardSummaryField => Boolean(field));
      if (!row.length) return null;
      const columns = row.length % 3 === 0 ? 3 : Math.min(row.length, 2);
      return <Box key={index} data-summary-row={index + 1} sx={{ display: 'grid', gridTemplateColumns: `repeat(${Math.min(row.length, 2)}, minmax(0, 1fr))`, '& > :last-child': { gridColumn: row.length % 2 ? '1 / -1' : 'auto' }, '@media (min-width: 360px)': { gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, '& > :last-child': { gridColumn: row.length % columns === 1 ? '1 / -1' : 'auto' } }, gap: 0.5, minWidth: 0 }}>
        {row.map(field => <Typography component="span" variant="body2" key={field.key} sx={{ ...fieldSx, borderRadius: '2px', boxSizing: 'border-box', px: 0.6, py: 0.35, minWidth: 0, maxWidth: '100%', whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word', lineHeight: 1.4 }}>
          {field.label && <Box component="span" sx={{ color: 'text.secondary', fontSize: '0.72rem' }}>{field.label}：</Box>}{field.value}
        </Typography>)}
      </Box>;
    })}
  </Box>;
}
