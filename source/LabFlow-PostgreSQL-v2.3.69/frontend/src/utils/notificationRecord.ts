export const parseNotificationRecordId = (value: string | null): number | undefined => {
  if (!value || !/^[1-9]\d*$/.test(value)) return undefined;
  const id = Number(value);
  return Number.isSafeInteger(id) ? id : undefined;
};
