use serde::{Deserialize, Serialize};

/// 角色基础信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// 系统内置角色标记（1=系统角色，不可改名/删除）
    pub is_system: i32,
    pub sort_order: i32,
    pub template_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoleTemplate {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_system: i32,
    pub sort_order: i32,
    pub permissions: Vec<String>,
}

/// 角色及其权限点（聚合返回）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoleWithPermissions {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_system: i32,
    pub sort_order: i32,
    pub template_id: Option<i64>,
    pub template_name: Option<String>,
    pub permissions: Vec<String>,
    /// 角色可跨部门查看的数据范围。空列表不自动授予跨部门数据权限。
    pub division_scope_ids: Vec<i64>,
    /// 分析检测门户可查看的部门范围。
    pub work_division_scope_ids: Vec<i64>,
    /// 样品信息登记可查看的检测类型。空列表不自动授予跨类型数据权限。
    pub sample_info_type_scope_keys: Vec<String>,
}

/// 创建角色请求
#[derive(Debug, Deserialize)]
pub struct RoleCreate {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub template_id: Option<i64>,
}

/// 更新角色基础信息请求
#[derive(Debug, Deserialize)]
pub struct RoleUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}

/// 整体设置角色权限点请求
#[derive(Debug, Deserialize)]
pub struct RolePermissionSet {
    pub permissions: Vec<String>,
}

/// 角色的数据可见范围。功能权限与数据范围分开保存，防止将业务 ID 拼入权限字符串。
#[derive(Debug, Deserialize)]
pub struct RoleDataScopeSet {
    #[serde(default)]
    pub division_ids: Vec<i64>,
    /// 分析检测门户的部门范围；为空表示清空该门户的显式范围。
    #[serde(default)]
    pub work_division_ids: Vec<i64>,
    #[serde(default)]
    pub sample_info_type_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoleDataScope {
    pub role_id: i64,
    pub division_ids: Vec<i64>,
    pub work_division_ids: Vec<i64>,
    pub sample_info_type_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CurrentRoleDataScopeSummary {
    pub has_configured_scope: bool,
    pub has_division_scope: bool,
    pub has_work_division_scope: bool,
    pub has_sample_info_type_scope: bool,
    pub division_ids: Vec<i64>,
    pub work_division_ids: Vec<i64>,
    pub sample_info_type_keys: Vec<String>,
}

/// 权限点定义（用于前端权限矩阵展示）
#[derive(Debug, Serialize, Clone)]
pub struct PermissionDef {
    pub key: &'static str,
    pub label: &'static str,
    pub full_label: &'static str,
    pub group: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portal: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<&'static str>,
}

/// 通配权限（系统管理员使用）
pub const ALL_PERMISSION: &str = "*";

/// 全部权限点常量（与前端 constants/permissions.ts 保持一致）
pub const PERMISSIONS: &[PermissionDef] = &[
    // 门户入口
    PermissionDef {
        key: "entry:sample",
        label: "研发送样门户",
        full_label: "研发送样门户",
        group: "门户入口",
        portal: None,
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "entry:workload",
        label: "分析检测门户",
        full_label: "分析检测门户",
        group: "门户入口",
        portal: None,
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "entry:sample-info",
        label: "样品信息登记门户",
        full_label: "样品信息登记门户",
        group: "门户入口",
        portal: None,
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "sample-info:create",
        label: "创建送样记录",
        full_label: "样品信息送样-创建送样记录",
        group: "门户入口",
        portal: Some("样品信息登记门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "sample-info:edit-own",
        label: "编辑本人记录",
        full_label: "样品信息送样-编辑本人记录",
        group: "数据范围",
        portal: Some("样品信息登记门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "sample:collect",
        label: "取样（研发送样）",
        full_label: "分析检测-对研发送样取样",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample:withdraw",
        label: "撤回取样（研发送样）",
        full_label: "分析检测-撤回研发送样取样",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample:return",
        label: "退回（研发送样）",
        full_label: "分析检测-退回研发送样",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample:complete",
        label: "完成检测（研发送样）",
        full_label: "分析检测-完成研发送样检测",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample-info:collect",
        label: "取样（样品信息）",
        full_label: "分析检测-对样品信息取样",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample-info:complete",
        label: "完成检测（样品信息）",
        full_label: "分析检测-完成样品信息检测",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample-info:withdraw",
        label: "撤回取样（样品信息）",
        full_label: "分析检测-撤回样品信息取样",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "sample-info:return",
        label: "退回（样品信息）",
        full_label: "分析检测-退回样品信息",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "records:rd:view-all",
        label: "查看全部研发送样记录",
        full_label: "查看全部研发送样记录",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: None,
    },
    PermissionDef {
        key: "records:rd:view-lab",
        label: "查看本实验室研发送样记录",
        full_label: "查看本实验室研发送样记录",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: None,
    },
    PermissionDef {
        key: "records:rd:portal-all-labs",
        label: "研发送样门户查看全部实验室",
        full_label: "研发送样门户查看全部实验室",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: None,
    },
    PermissionDef {
        key: "records:rd:select-sender",
        label: "研发送样选择实际送样人",
        full_label: "研发送样选择实际送样人",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "records:rd:edit-created",
        label: "编辑本人提交的记录",
        full_label: "研发送样-编辑本人提交的记录",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "records:rd:edit-subject",
        label: "编辑本人送样记录",
        full_label: "研发送样-编辑本人作为送样人的记录",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "records:rd:return-confirm",
        label: "确认并处理退回",
        full_label: "研发送样-确认并处理退回",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "records:rd:return-delegate",
        label: "代确认并修改已退回的研发送样记录",
        full_label: "代确认并修改已退回的研发送样记录",
        group: "数据范围",
        portal: Some("研发送样门户"),
        actor: None,
    },
    PermissionDef {
        key: "sample-info:return-confirm",
        label: "确认并处理退回",
        full_label: "样品信息送样-确认并处理退回",
        group: "数据范围",
        portal: Some("样品信息登记门户"),
        actor: Some("送样人"),
    },
    PermissionDef {
        key: "records:work:portal-all-labs",
        label: "分析检测门户查看全部实验室",
        full_label: "分析检测门户查看全部实验室",
        group: "数据范围",
        portal: Some("分析检测门户"),
        actor: None,
    },
    PermissionDef {
        key: "records:work:portal-scoped",
        label: "分析检测公共账号人员选择门户",
        full_label: "分析检测公共账号人员选择门户",
        group: "门户入口",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "records:work:select-detector",
        label: "分析检测选择实际检测人",
        full_label: "分析检测选择实际检测人",
        group: "数据范围",
        portal: Some("分析检测门户"),
        actor: Some("检测人员"),
    },
    PermissionDef {
        key: "records:work:view-scope",
        label: "分析检测查看本范围人员工作量",
        full_label: "分析检测查看本范围人员工作量",
        group: "数据范围",
        portal: Some("分析检测门户"),
        actor: None,
    },
    PermissionDef {
        key: "records:work:view-all",
        label: "分析检测查看全公司工作量",
        full_label: "分析检测查看全公司工作量",
        group: "数据范围",
        portal: Some("分析检测门户"),
        actor: None,
    },
    PermissionDef {
        key: "session:extended",
        label: "延长登录空闲时长（1000分钟）",
        full_label: "延长登录空闲时长（1000分钟）",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:settings",
        label: "页面与录入表单配置",
        full_label: "页面与录入表单配置",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:notifications",
        label: "通知中心配置",
        full_label: "通知中心配置",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    // 系统管理
    PermissionDef {
        key: "manage:projects",
        label: "研发项目管理",
        full_label: "研发项目管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:groups",
        label: "实验室管理",
        full_label: "实验室管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:divisions",
        label: "部门管理",
        full_label: "部门管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:methods",
        label: "检测方法管理",
        full_label: "检测方法管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:instruments",
        label: "仪器管理",
        full_label: "仪器管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:master-import",
        label: "主数据管理",
        full_label: "主数据管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:trash",
        label: "回收站",
        full_label: "回收站",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:audit",
        label: "审计日志",
        full_label: "审计日志",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:backup",
        label: "数据备份",
        full_label: "数据备份",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:help",
        label: "教程与帮助",
        full_label: "教程与帮助",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:sampleinfo",
        label: "样品信息登记管理",
        full_label: "样品信息登记管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:users",
        label: "用户管理",
        full_label: "用户管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:roles",
        label: "角色管理",
        full_label: "角色管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:stats",
        label: "统计管理",
        full_label: "统计管理",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:log-maintenance",
        label: "日志与维护",
        full_label: "日志与维护",
        group: "系统管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:data-governance:view",
        label: "数据治理查看",
        full_label: "数据治理查看",
        group: "数据治理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:data-governance:import",
        label: "业务数据导入",
        full_label: "业务数据导入",
        group: "数据治理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:data-governance:export",
        label: "原始数据导出",
        full_label: "原始数据导出",
        group: "数据治理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "manage:data-governance:purge",
        label: "永久清理数据",
        full_label: "永久清理数据",
        group: "数据治理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:access",
        label: "分析检测统计入口",
        full_label: "分析检测统计入口",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:portal:workload",
        label: "查看分析检测统计门户",
        full_label: "查看分析检测统计门户",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:portal:rd",
        label: "查看研发送样统计门户",
        full_label: "查看研发送样统计门户",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:portal:sample-info",
        label: "查看样品信息统计门户",
        full_label: "查看样品信息统计门户",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:week",
        label: "按周统计",
        full_label: "按周统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:month",
        label: "按月统计",
        full_label: "按月统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:user-log",
        label: "检测人记录",
        full_label: "检测人记录",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:division",
        label: "事业部统计",
        full_label: "事业部统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet1",
        label: "实验室-项目-方法",
        full_label: "实验室-项目-方法",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet2",
        label: "仪器汇总",
        full_label: "仪器汇总",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet3",
        label: "项目汇总（含金额）",
        full_label: "项目汇总（含金额）",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet4",
        label: "实验室汇总（含金额）",
        full_label: "实验室汇总（含金额）",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet5",
        label: "检测人汇总（原始记录）",
        full_label: "检测人汇总（原始记录）",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet6",
        label: "检测人汇总表（含系数）",
        full_label: "检测人汇总表（含系数）",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet7",
        label: "实验室总表",
        full_label: "实验室总表",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet8",
        label: "项目总表",
        full_label: "项目总表",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet9",
        label: "仪器类型汇总",
        full_label: "仪器类型汇总",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:sheet10",
        label: "理化汇总",
        full_label: "理化汇总",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:view-all",
        label: "查看全部统计数据",
        full_label: "查看全部统计数据",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:workload:export",
        label: "导出分析检测统计",
        full_label: "导出分析检测统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:rd:access",
        label: "研发送样统计入口",
        full_label: "研发送样统计入口",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:rd:view-all",
        label: "查看全部研发送样统计",
        full_label: "查看全部研发送样统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:rd:view-lab",
        label: "查看本实验室送样统计",
        full_label: "查看本实验室送样统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    // 研发送样统计卡片权限（细粒度控制）
    PermissionDef {
        key: "stats:rd:week",
        label: "按周统计",
        full_label: "研发送样统计-按周统计卡片",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:month",
        label: "按月统计",
        full_label: "研发送样统计-按月统计卡片",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:user-log",
        label: "送样人记录",
        full_label: "研发送样统计-送样人记录卡片",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:division",
        label: "事业部统计",
        full_label: "研发送样统计-事业部统计卡片",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet1",
        label: "Sheet1 实验室-项目-方法",
        full_label: "研发送样统计-Sheet1导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet2",
        label: "Sheet2 仪器汇总",
        full_label: "研发送样统计-Sheet2导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet3",
        label: "Sheet3 项目汇总",
        full_label: "研发送样统计-Sheet3导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet4",
        label: "Sheet4 实验室汇总",
        full_label: "研发送样统计-Sheet4导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet5",
        label: "Sheet5 送样人原始记录",
        full_label: "研发送样统计-Sheet5导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet6",
        label: "Sheet6 送样人汇总",
        full_label: "研发送样统计-Sheet6导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet7",
        label: "Sheet7 实验室总表",
        full_label: "研发送样统计-Sheet7导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet8",
        label: "Sheet8 项目总表",
        full_label: "研发送样统计-Sheet8导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet9",
        label: "Sheet9 仪器类型汇总",
        full_label: "研发送样统计-Sheet9导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet10",
        label: "Sheet10 理化汇总",
        full_label: "研发送样统计-Sheet10导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:rd:sheet11",
        label: "Sheet11 事业部汇总",
        full_label: "研发送样统计-Sheet11导出预览",
        group: "统计管理",
        portal: Some("研发送样统计门户"),
        actor: Some("统计员"),
    },
    PermissionDef {
        key: "stats:sample-info:view-all",
        label: "查看全部样品信息统计",
        full_label: "查看全部样品信息统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "stats:sample-info:view-lab",
        label: "查看本实验室样品信息统计",
        full_label: "查看本实验室样品信息统计",
        group: "统计管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "help:edit",
        label: "编辑帮助文档",
        full_label: "编辑帮助文档",
        group: "内容管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "help:view",
        label: "帮助与反馈查看",
        full_label: "帮助与反馈查看",
        group: "内容管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "feedback:personnel:view",
        label: "人员变动反馈查看",
        full_label: "人员变动反馈查看",
        group: "内容管理",
        portal: None,
        actor: None,
    },
    PermissionDef {
        key: "feedback:personnel:edit",
        label: "人员变动反馈编辑",
        full_label: "人员变动反馈编辑",
        group: "内容管理",
        portal: None,
        actor: None,
    },
];

/// 判断权限点列表是否包含某权限（支持 `*` 通配）
pub fn has_permission(perms: &[String], key: &str) -> bool {
    perms.iter().any(|p| p == ALL_PERMISSION || p == key)
}

/// 校验权限点合法性（用于写入前校验）
pub fn is_valid_permission(key: &str) -> bool {
    key == ALL_PERMISSION || PERMISSIONS.iter().any(|p| p.key == key)
}

#[cfg(test)]
mod tests {
    use super::is_valid_permission;

    #[test]
    fn seeded_management_permissions_are_accepted() {
        assert!(is_valid_permission("manage:notifications"));
        assert!(is_valid_permission("manage:log-maintenance"));
        assert!(is_valid_permission("sample-info:return"));
        assert!(is_valid_permission("sample-info:return-confirm"));
        assert!(is_valid_permission("records:rd:return-delegate"));
        assert!(is_valid_permission("*"));
    }
}
