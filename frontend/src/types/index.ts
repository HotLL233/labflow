export interface ProjectGroup {
  id: number;
  name: string;
  sort_order: number;
  description?: string;
  created_at: string;
  project_count?: number;
  project_names?: string;
  rd_record_count?: number;
  show_in_work?: boolean;
  show_in_rd?: boolean;
  division_id?: number | null;
  division_name?: string | null;
}

// ========== v0.4.24: 事业部 ==========
export interface Division {
  id: number;
  name: string;
  sort_order: number;
  lab_count?: number;
  color?: string;
  is_active?: boolean;
}

// v0.2.17: 卡片独立 — Project 简化
export interface Project {
  id: number;
  name: string;
  full_name?: string;
  notes: string;
  sort_order?: number;
  is_active?: boolean;
  lab_ids: number[];
  lab_names: string[];
  method_ids: number[];
  method_names: string[];
  created_at: string;
  group_name?: string;
  coefficient?: number;
  method_type?: string;
  high_item?: string | null;
  project_status?: 'ongoing' | 'archived';
  archived_at?: string | null;
  archived_by?: string | null;
}

// v0.2.17: 新增 Method 类型
export interface Method {
  id: number;
  method_code: string;
  name: string;
  full_name: string;
  coefficient: number;
  multiplier: number;
  amount?: number;
  notes: string;
  is_active: boolean;
  type_ids: number[];
  type_names: string[];
  instrument_id?: number | null;
  instrument_code: string;
  instrument_name: string;
  instrument_type: string;
  created_at: string;
}

export interface Instrument {
  id: number;
  code: string;
  name: string;
  instrument_type: string;
  is_active: boolean;
  notes: string;
  created_at: string;
}

// v0.2.8: 方法类型
export interface MethodType {
  id: number;
  name: string;
  sort_order: number;
}

export interface WorkRecord {
  id: number;
  business_no: string;
  project_id: number;
  method_id?: number;
  project_name?: string;
  group_name?: string;
  user_name: string;
  quantity: number;
  recorded_at: string;
  batch_no?: string;
  extra_info?: string;
  instrument?: string;
  instrument_code?: string;
  instrument_type?: string;
  method_name?: string;
  method_type?: string;
  multiplier?: number;
  division_id?: number | null;
  created_at: string;
  status?: string;
  sampler?: string;
  sampled_at?: string;
  detected_by?: string;
  detected_at?: string;
  subject_user_id?: number | null;
  created_by_user_id?: number | null;
  notes?: string;
  high_item?: string | null;
  coefficient_snapshot: number;
}

export interface SampleRecord {
  id: number;
  group_id: number;
  group_name?: string;
  sample_name: string;
  sample_type?: string;
  quantity: number;
  sample_count?: number;
  unit?: string;
  batch_no?: string;
  user_name: string;
  recorded_at: string;
  submitted_at?: string;
  project_id?: number;
  notes?: string;
  extra_info?: string;
  created_at: string;
}

export interface AuditLog {
  id: number;
  user_id?: number | null;
  action: string;
  table_name: string;
  record_id: number | null;
  user_name: string;
  detail?: string;
  module: string;
  business_no: string;
  before_data?: Record<string, unknown> | null;
  after_data?: Record<string, unknown> | null;
  source: string;
  created_at: string;
}

export interface RecordEvent {
  id: number;
  module: string;
  table_name: string;
  record_id: number;
  business_no: string;
  event_type: string;
  from_status?: string | null;
  to_status?: string | null;
  operator: string;
  operated_at: string;
  reason: string;
  before_data?: Record<string, unknown> | null;
  after_data?: Record<string, unknown> | null;
}

export interface StatsDetail {
  period: string;
  total_quantity: number;
  record_count: number;
  coefficient_score: number;
}

export interface StatsSummary {
  total_quantity: number;
  total_records: number;
  user_count: number;
  project_count: number;
  coefficient_score: number;
  details: StatsDetail[];
}

export interface UserStats {
  user_name: string;
  total_quantity: number;
  record_count: number;
  coefficient_score: number;
}

export interface ProjectStats {
  project_id: number;
  project_name: string;
  group_name: string;
  total_quantity: number;
  record_count: number;
  coefficient_score: number;
}

export interface TypeStats {
  instrument_type: string;
  total_quantity: number;
  record_count: number;
  coefficient_score: number;
}

export interface InstrumentStats {
  instrument: string;
  instrument_type: string;
  total_quantity: number;
  record_count: number;
  user_count: number;
  coefficient_score: number;
}

// --- Record Update (user correction) ---
export interface RecordUpdate {
  user_name?: string;
  quantity?: number;
  recorded_at?: string;
  multiplier?: number;
}

// --- API Response ---
export interface ApiResponse<T> {
  code: number;
  message: string;
  data: T | null;
}

// --- Sample Stats ---
export interface GroupSampleStats {
  group_name: string;
  count: number;
  total_samples: number;
}

export interface ProjectSampleStats {
  project_name: string;
  group_name: string;
  count: number;
  total_samples: number;
}

export interface UserSampleStats {
  user_name: string;
  count: number;
  total_samples: number;
}

export interface SampleStats {
  total_count?: number;
  total_samples?: number;
  by_group?: GroupSampleStats[];
  by_project?: ProjectSampleStats[];
  by_user?: UserSampleStats[];
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}

// --- Import Result (Excel导入返回结果) ---
export interface ImportResult {
  success: boolean;
  total_rows_read: number;
  inserted: number;
  updated: number;
  skipped: number;
  sheet_name: string;
  columns_found: string[];
  errors: string[];
  warnings: string[];
}

// v0.2.17: Method import summary
export interface ImportSummary {
  total_methods: number;
  total_projects: number;
  total_groups: number;
  by_type: { method_type: string; count: number }[];
}

export interface BackupStatus {
  auto_enabled: boolean;
  auto_interval_hours: number;
  max_backup_count: number;
  backup_mode?: 'database' | 'full';
  backup_sync_dir?: string | null;
  last_backup: string | null;
  backup_count: number;
  backup_files: { name: string; size: number; time: string; kind?: 'database' | 'full' }[];
  db_size: number;
  tables: { table: string; rows: number; label?: string }[];
  backups_dir: string;
  pending_restore?: boolean;
}

// v0.3.0: 导入映射配置
export interface ImportMapping {
  id: number;
  header_pattern: string;
  match_mode: string;
  target_table: string;
  default_type: string;
  priority: number;
  is_active: boolean;
}

// ========== v0.3.7: 导出预览数据类型 ==========

// Sheet 1 预览数据（后端 FlatRow = (String, String, String, String, i64, bool, f64)）
// JSON 序列化后为含数字索引的 object
export interface Sheet1Row {
  0: string; // 实验室
  1: string; // 项目代号
  2: string; // 液相仪器
  3: string; // 检测方法
  4: number; // 检测数量
  5: boolean; // 是否气相
  6: number; // 系数
}
export type Sheet1Data = [string, string, string, string, number, number, boolean, number, string | null][];

// Sheet 2: 仪器-汇总
export interface Sheet2Row {
  date: string;
  instrument: string;
  lab: string;
  project: string;
  method: string;
  quantity: number;
}

// Sheet 3: 项目-汇总
export interface Sheet3Row {
  project: string;
  lab: string;
  instrument: string;
  method: string;
  quantity: number;
  unit_price: number;  // 单价（原 amount）
  multiplier?: number;
}

// Sheet 4: 实验室-汇总
export interface Sheet4Row {
  lab: string;
  project: string;
  instrument: string;
  method: string;
  quantity: number;
  unit_price: number;  // 单价（原 amount）
  multiplier?: number;
}

// Sheet 5: 人员-汇总（原始记录）
export interface Sheet5Row {
  recorded_at: string;
  lab: string;
  project: string;
  method: string;
  method_type: string;
  quantity: number;
  user_name: string;
}

// Sheet 6: 人员汇总表
export interface Sheet6Row {
  user_name: string;
  method_type: string;
  coefficient: number;
  quantity: number;
}

// Sheet 7: 实验室总表
export interface Sheet7Row {
  lab: string;
  project: string;
  method_type: string;
  unit_price: number;  // 单价（原 amount）
  quantity: number;
  multiplier?: number;
}

// Sheet 8: 项目总表
export interface Sheet8Row {
  project: string;
  method_type: string;
  unit_price: number;  // 单价（原 amount）
  quantity: number;
  multiplier?: number;
}

// Sheet 9: 仪器汇总表
export interface Sheet9Row {
  instrument: string;
  quantity: number;
  instrument_type: string;
}

// ========== v0.4.11: 帮助文档 ==========
export interface HelpDocument {
  id: number;
  title: string;
  filename: string;
  file_path: string;
  file_type: string;
  file_size: number;
  is_visible: boolean;
  sort_order: number;
  page_count: number;
  created_at: string;
  updated_at: string;
}

export interface TocItem {
  id: string;
  text: string;
  level: number;
  children: TocItem[];
}

export interface HelpArticle {
  id: number;
  title: string;
  content_html: string;
  toc_json: string | null;
  source_file: string | null;
  is_visible: boolean;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

// Sheet 10: 理化汇总表
export interface Sheet10Row {
  method: string;
  quantity: number;
}

// ========== v0.4.22: 样品信息登记 ==========
export interface SampleInfoRecord {
  id: number;
  business_no: string;
  status: string;
  seq_no: number;
  batch_no: string;
  user_name: string;
  lab_name: string;
  project_name: string;
  submitted_at: string;
  detection_date: string;
  sampled_by: string;
  sampled_at?: string | null;
  detected_by: string;
  main_components: string;
  detection_type: string;
  type_key: string;
  division_id?: number | null;
  division_name?: string | null;
  quantity: number;
  notes: string;
  extra_fields?: Record<string, any>;
  created_at: string;
  updated_at?: string;
  deleted_at?: string | null;
  group_id?: number | null;
  created_by_user_id?: number | null;
}

// ========== v0.4.26: 列自定义 ==========
export interface SampleInfoColumn {
  id: number;
  field_key: string;
  label: string;
  data_type: 'text' | 'number' | 'select' | 'date' | 'attachment';
  is_predefined: boolean;
  is_required: boolean;
  is_active: boolean;
  width: number;
  sort_order: number;
  options: string | null;
  show_in_list: boolean;
  show_in_export: boolean;
  show_in_form: boolean;
  type_key?: string | null;
  visible_types?: string[];
  created_at: string;
  updated_at: string | null;
}

// ========== v0.4.27-A: 列可见性 ==========
export interface SampleInfoColumnVisibility {
  id: number;
  type_key: string;
  column_id: number;
  is_visible: boolean;
}

// ========== v0.4.27-A: 附件 ==========
export interface SampleInfoAttachment {
  id: number;
  record_id: number;
  file_name: string;
  stored_name: string;
  file_size: number;
  file_type: string;
  created_at: string;
}

// ========== v0.4.27-A: 用户 ==========
export interface User {
  id: number;
  username: string;
  division_id?: number | null;
  division_name?: string | null;
  group_id?: number | null;
  group_name?: string | null;
  is_admin: boolean;
  is_active: boolean;
  /** 关联角色 id（NULL 表示未分配角色） */
  role_id?: number | null;
  role_name?: string | null;
  /** 多角色 id 集合 */
  role_ids?: number[];
  role_names?: string[];
  /** 由角色自动计算的用户归属组，不是业务实验室。 */
  affiliation_groups?: string[];
  /** 角色对应的权限点集合 */
  permissions?: string[];
  created_at: string;
  updated_at?: string | null;
}

// ========== v0.4.32: 用户分级（角色） ==========
export interface Role {
  id: number;
  name: string;
  description: string;
  is_system: number; // 1=系统内置角色
  sort_order: number;
}

export interface RoleWithPermissions {
  id: number;
  name: string;
  description: string;
  is_system: number;
  sort_order: number;
  permissions: string[];
}

export interface LoginRequest {
  username: string;
  password: string;
  device_id?: string;
  device_name?: string;
}

export interface LoginResponse {
  token: string;
  user: User;
}

export interface UserUpdate {
  username?: string;
  password?: string;
  division_id?: number | null;
  group_id?: number | null;
  is_admin?: boolean;
  is_active?: boolean;
  role_ids?: number[];
}

export interface UserSession {
  id: number;
  user_id: number;
  username: string;
  created_at: string;
  expires_at: string;
  device_id: string;
  device_name: string;
  is_expired: boolean;
}

// v0.4.28: 事业部统计
export interface DivisionStats {
  division_id: number | null;
  division_name: string;
  total_quantity: number;
  record_count: number;
  coefficient_score: number;
  lab_count: number;
}

// v0.4.28: 事业部导出预览
export interface Sheet11Row {
  division_name: string;
  lab_count: number;
  total_quantity: number;
  record_count: number;
  coefficient_score: number;
}

export interface ColumnVisibilityItem {
  column_id: number;
  is_visible: boolean;
}

// ========== v0.4.33: 研发送样列配置 ==========
export interface RdRecordColumn {
  id: number;
  name: string;
  label: string;
  data_type: 'text' | 'number';
  width: number;
  sort_order: number;
  is_predefined: boolean;
  show_in_list: boolean;
  show_in_form: boolean;
  created_at: string;
  updated_at: string | null;
}
export interface SampleInfoType {
  id: number;
  type_key: string;
  label: string;
  description: string;
  color: string;
  sort_order: number;
  is_active: number;
  created_at: string;
}

// ========== v0.4.35: 全 UI 自定义系统 ==========
export interface SystemSetting {
  key: string;
  value: string; // JSON string
  updated_at: string | null;
}

export interface ThemeSettings {
  primaryColor: string;
  secondaryColor: string;
  bgColor: string;
  cardRadius: number;
  loginBg: string;
  loginButtonColor: string;
  logoText: string;
}

export interface HomeCard {
  key: string;
  title: string;
  subtitle: string;
  path: string;
  perm: string;
  icon: string;
  gradient: string;
  border: string;
  titleColor: string;
}

export interface PortalStyle {
  sampleColor: string;
  workloadColor: string;
  brandName: string;
}

export interface ManageTab {
  key: string;
  label: string;
  icon: string;
  perm: string;
  enabled: boolean;
}

export interface StatCard {
  key: string;
  label: string;
  color: string;
  gradient: string;
}

export interface MasterImportIssue {
  sheet: string;
  row: number;
  entity_type: string;
  name: string;
  action: string;
  level: 'info' | 'warning' | 'error';
  message: string;
}

export interface MasterImportCounts {
  total_rows: number;
  departments: number;
  labs: number;
  method_types: number;
  instruments: number;
  methods: number;
  projects: number;
  relations: number;
  creates: number;
  updates: number;
  skips: number;
  errors: number;
  warnings: number;
}

export interface MasterImportPreview {
  valid: boolean;
  mode: 'upsert' | 'skip';
  counts: MasterImportCounts;
  issues: MasterImportIssue[];
}

export interface MasterImportResult {
  success: boolean;
  created: number;
  updated: number;
  skipped: number;
  relation_sets: number;
  message: string;
}

export type GovernanceModule = 'work' | 'rd' | 'sample-info';

export interface GovernanceModuleSummary {
  module: 'work' | 'rd' | 'sample_info';
  label: string;
  total: number;
  active: number;
  deleted: number;
  earliest: string | null;
  latest: string | null;
  attachment_count: number;
  attachment_bytes: number;
}

export interface GovernanceSummary {
  db_size: number;
  attachment_bytes: number;
  modules: GovernanceModuleSummary[];
}

export interface GovernanceImportPreview {
  module: string;
  file_sha256: string;
  total_rows: number;
  new_rows: number;
  duplicate_rows: number;
  invalid_rows: number;
  issues: string[];
}

export interface GovernancePurgePreview {
  module: string;
  start: string;
  end: string;
  active_count: number;
  deleted_count: number;
  attachment_count: number;
  attachment_bytes: number;
  confirmation_token: string;
  expires_at: string;
}

export interface GovernancePurgeResult {
  module: string;
  moved_count: number;
  preserved_attachment_count: number;
  export_file: string;
  export_sha256: string;
}

export interface TrashEntry {
  id: number;
  entity_type: string;
  table_name: string;
  record_id: number;
  category: 'records' | 'master' | 'config' | 'access' | 'files' | string;
  module: 'work' | 'rd' | 'sample_info' | 'shared' | string;
  display_name: string;
  business_no: string;
  snapshot: Record<string, unknown> | null;
  delete_reason: string;
  deleted_by_user_id: number | null;
  deleted_by_username: string;
  owner_user_id: number | null;
  owner_group_id: number | null;
  deleted_at: string;
  dependency_summary: string;
  can_purge: boolean;
}

export interface TrashPrecheck {
  entity_type: string;
  table_name: string;
  record_id: number;
  display_name: string;
  dependency_summary: string;
  can_purge: boolean;
}

export interface LogMaintenancePolicy {
  session_retention_days: number;
  runtime_log_online_retention_days: number;
  runtime_log_archive_retention_days: number;
  audit_auto_archive_enabled: boolean;
  audit_online_retention_days: number;
  audit_archive_retention_days: number;
  archive_time: string;
  database_maintenance_enabled: boolean;
  maintenance_day: number;
  maintenance_time: string;
}

export interface LogMaintenancePolicyView {
  policy: LogMaintenancePolicy;
  runtime_log_enabled: boolean;
  runtime_log_max_size_mb: number;
}

export interface LogArchiveBatch {
  id: number;
  archive_type: string;
  start_at: string | null;
  end_at: string | null;
  file_path: string;
  sha256: string;
  record_count: number;
  file_size: number;
  status: string;
  created_at: string;
  verified_at: string | null;
}

export interface MaintenanceJob {
  id: number;
  job_type: string;
  status: string;
  detail: string;
  started_at: string;
  completed_at: string | null;
}

export interface MaintenanceJobResult {
  detail: string;
  affected: number;
}

export interface LogMaintenanceStatus {
  policy: LogMaintenancePolicy;
  db_size: number;
  audit_online_count: number;
  expired_session_count: number;
  runtime_log_bytes: number;
  archive_count: number;
  archives: LogArchiveBatch[];
  recent_jobs: MaintenanceJob[];
  runtime_log_enabled: boolean;
  runtime_log_max_size_mb: number;
  data_dir: string;
}

export interface NotificationChannel { id: number; name: string; channel_type: string; webhook_url_masked: string; has_secret: boolean; is_active: boolean; created_at: string; updated_at: string; }
export interface NotificationRule { id: number; name: string; event_type: string; group_id: number | null; group_name: string | null; project_name: string; detection_type: string; channel_id: number | null; channel_name: string | null; recipient_user_ids: number[]; is_default: boolean; is_active: boolean; created_at: string; }
export interface NotificationDelivery { id: number; event_id: number; channel_name: string; status: string; attempts: number; response_summary: string; last_error: string; sent_at: string | null; created_at: string; }
export interface NotificationSummary { pending_count: number; failed_count: number; sent_today_count: number; skipped_count: number; }
export interface InAppNotification { id: number; title: string; body: string; target_url: string; is_read: boolean; created_at: string; }
