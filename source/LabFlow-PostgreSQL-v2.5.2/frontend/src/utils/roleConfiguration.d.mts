import type { PermissionDef, RolePurpose } from '../types';
export function permissionCategory(permission: Pick<PermissionDef, 'key'> & { portal?: string; legacy?: boolean }): string;
export function matchesPermissionFilter(permission: Pick<PermissionDef, 'key'> & Partial<PermissionDef> & { legacy?: boolean; sources?: string[]; explanation?: string }, search: string, group?: string): boolean;
export interface ConfigurationValues {
  name: string; description: string; permissions: string[]; purpose: RolePurpose;
  divisions: number[]; workDivisions: number[]; sampleTypes: string[]; active: boolean;
}
export function configurationFingerprint(draft: ConfigurationValues): string;
export function roleConfigurationPayload(draft: ConfigurationValues & {base: { configuration_revision?: string }}): {
  expected_revision: string; name: string; description: string; permissions: string[];
  role_purpose: RolePurpose; division_ids: number[]; work_division_ids: number[]; sample_info_type_keys: string[];
};
