type RoleIdentity = {
  role_purpose?: string | null;
  identity_key?: string | null;
  system_key?: string | null;
  name?: string;
  template_name?: string | null;
};

export function rolePurpose(role: RoleIdentity): 'personal' | 'public_rd' | 'public_analysis' {
  if (role.role_purpose != null) {
    return role.role_purpose === 'public_rd' || role.role_purpose === 'public_analysis'
      ? role.role_purpose : 'personal';
  }
  // Old servers have no purpose field; only their exact built-in identities are compatible.
  if (role.system_key === 'analysis_public_account') return 'public_analysis';
  if (role.system_key === 'public_account' || role.system_key === 'rd_public_account') return 'public_rd';
  if (role.system_key != null) return 'personal';
  if (role.name === '分析检测公共账号') return 'public_analysis';
  if (role.name === '公共账号' || role.name === '研发送样公共账号') return 'public_rd';
  return 'personal';
}

export function roleBusinessIdentity(role: RoleIdentity): string {
  if (role.identity_key != null) return role.identity_key;
  const legacy: Record<string, string> = {
    system_admin: 'system_admin', analyst: 'analyst', analysis_leader: 'analysis_leader',
    rd_sender: 'rd_sender', rd_leader: 'rd_leader',
    系统管理员: 'system_admin', 分析检测员: 'analyst', 分析检测组长: 'analysis_leader',
    研发送样员: 'rd_sender', 研发送样组长: 'rd_leader',
    分析检测员模板: 'analyst', 分析检测组长模板: 'analysis_leader',
    研发送样员模板: 'rd_sender', 研发送样组长模板: 'rd_leader',
  };
  return legacy[role.system_key || ''] || legacy[role.template_name || ''] || legacy[role.name || ''] || 'ordinary';
}
