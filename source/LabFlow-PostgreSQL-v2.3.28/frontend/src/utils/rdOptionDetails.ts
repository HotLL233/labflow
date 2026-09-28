export type RdOptionDetailRule = {
  trigger_value: string;
  label: string;
  placeholder?: string;
  required?: boolean;
};

/** Read both the current JSON-array format and the legacy delimited format. */
export const parseRdFieldOptions = (source?: string | null): string[] => {
  const raw = (source || '').trim();
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return parsed.map(value => String(value).trim()).filter(Boolean);
    }
  } catch {
    // Fall through to the legacy delimiter format.
  }
  return raw.split(/[,，;；\n]/).map(value => value.trim()).filter(Boolean);
};

export const parseRdOptionDetailRules = (source?: string): RdOptionDetailRule[] => {
  if (!source?.trim()) return [];
  try {
    const parsed = JSON.parse(source);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map((rule): RdOptionDetailRule | null => {
        if (!rule || typeof rule !== 'object') return null;
        const trigger_value = String(rule.trigger_value || '').trim();
        const label = String(rule.label || '').trim();
        if (!trigger_value || !label) return null;
        return { trigger_value, label, placeholder: String(rule.placeholder || ''), required: Boolean(rule.required) };
      })
      .filter((rule): rule is RdOptionDetailRule => Boolean(rule));
  } catch {
    return [];
  }
};

export const rdOptionDetailKey = (fieldKey: string) => `${fieldKey}__detail`;

export const getRdOptionSelection = (value: unknown, rules: RdOptionDetailRule[]) => {
  const text = String(value ?? '');
  const legacy = text.match(/^(.+?)：([\s\S]*)$/);
  if (legacy && rules.some(rule => rule.trigger_value === legacy[1].trim())) {
    return { selected: legacy[1].trim(), legacyDetail: legacy[2].trim() };
  }
  return { selected: text, legacyDetail: '' };
};

export const formatRdOptionValue = (value: unknown, detail: unknown, rules: RdOptionDetailRule[]) => {
  const { selected, legacyDetail } = getRdOptionSelection(value, rules);
  const rule = rules.find(item => item.trigger_value === selected);
  const detailText = String(detail ?? legacyDetail ?? '').trim();
  return rule && detailText ? `${selected}（${detailText}）` : selected || '-';
};
