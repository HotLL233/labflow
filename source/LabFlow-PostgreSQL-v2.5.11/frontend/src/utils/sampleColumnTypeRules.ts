export type SampleColumnTypeRuleDraft = {
  type_key: string;
  is_visible: boolean;
  is_required: boolean;
  show_in_form: boolean;
  show_in_list: boolean;
  show_in_export: boolean;
  sort_order: number;
};

type SelectedFlags = Pick<SampleColumnTypeRuleDraft,
  'is_required' | 'show_in_form' | 'show_in_list' | 'show_in_export' | 'sort_order'>;

export function buildSampleColumnTypeRules(
  rules: SampleColumnTypeRuleDraft[], selectedTypeKey: string,
  selectedFlags: SelectedFlags, isNewColumn: boolean,
): SampleColumnTypeRuleDraft[] {
  return rules.map(rule => {
    const useSelectedFlags = isNewColumn || rule.type_key === selectedTypeKey;
    const flags = useSelectedFlags ? selectedFlags : rule;
    return {
      ...rule,
      is_required: rule.is_visible && flags.show_in_form &&
        (rule.type_key === selectedTypeKey ? selectedFlags.is_required : rule.is_required),
      show_in_form: rule.is_visible && flags.show_in_form,
      show_in_list: rule.is_visible && flags.show_in_list,
      show_in_export: rule.is_visible && flags.show_in_export,
      sort_order: rule.type_key === selectedTypeKey ? selectedFlags.sort_order : rule.sort_order,
    };
  });
}
