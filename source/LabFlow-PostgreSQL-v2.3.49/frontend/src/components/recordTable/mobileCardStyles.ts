/** 手机操作统一触控尺寸；按钮宽度仍由各业务的自定义配置决定。 */
export const mobileActionButtonSx = {
  minHeight: 40,
  boxSizing: 'border-box',
  px: 1,
  py: 0.75,
  fontSize: '0.75rem',
  lineHeight: 1.3,
  borderRadius: '4px',
  boxShadow: 'none',
  whiteSpace: 'normal',
  '& .MuiButton-startIcon': { ml: 0, mr: 0.5 },
  '& .MuiSvgIcon-root': { fontSize: 18 },
} as const;

export const mobileRecordActionsSx = {
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'center',
  justifyContent: 'flex-start',
  gap: 0.75,
  width: '100%',
  minWidth: 0,
  '& .MuiButton-root': mobileActionButtonSx,
} as const;
