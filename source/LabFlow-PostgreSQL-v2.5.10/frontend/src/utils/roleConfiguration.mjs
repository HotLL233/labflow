const compatibility = new Set(['session:extended', 'stats:workload:access', 'stats:rd:access', 'manage:help']);

export function permissionCategory(permission) {
  const { key, portal = '' } = permission;
  if (key === '*' || key === 'records:work:portal-scoped' || compatibility.has(key) || permission.legacy) return '高级与历史';
  if (key.startsWith('feedback:personnel:')) return '人员变动';
  if (key.includes('auxiliary')) return '辅助工作';
  if (key.includes('sample-info') || portal.includes('样品信息')) return '样品登记';
  if (key.includes(':rd') || key.startsWith('sample:') || key === 'entry:sample' || portal.includes('研发送样')) return '研发送样';
  if (key.includes('work') || key === 'entry:workload' || portal.includes('分析检测')) return '分析检测';
  return '通知与管理';
}

export function configurationFingerprint(draft) {
  const sorted = values => [...values].sort((a, b) => String(a).localeCompare(String(b)));
  return JSON.stringify({
    name: draft.name, description: draft.description,
    permissions: sorted(draft.permissions), purpose: draft.purpose,
    divisions: sorted(draft.divisions), workDivisions: sorted(draft.workDivisions),
    sampleTypes: sorted(draft.sampleTypes), active: draft.active,
  });
}

export function roleConfigurationPayload(draft) {
  return {
    expected_revision: draft.base.configuration_revision,
    name: draft.name.trim(), description: draft.description,
    permissions: [...draft.permissions], role_purpose: draft.purpose,
    division_ids: [...draft.divisions], work_division_ids: [...draft.workDivisions],
    sample_info_type_keys: [...draft.sampleTypes],
  };
}
