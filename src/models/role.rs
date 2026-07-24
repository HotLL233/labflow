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
}

/// 角色及其权限点（聚合返回）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoleWithPermissions {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_system: i32,
    pub sort_order: i32,
    pub permissions: Vec<String>,
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

/// 权限点定义（用于前端权限矩阵展示）
#[derive(Debug, Serialize, Clone)]
pub struct PermissionDef {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

/// 通配权限（系统管理员使用）
pub const ALL_PERMISSION: &str = "*";

/// 全部权限点常量（与前端 constants/permissions.ts 保持一致）
pub const PERMISSIONS: &[PermissionDef] = &[
    // 门户入口
    PermissionDef {
        key: "entry:sample",
        label: "研发送样",
        group: "门户入口",
    },
    PermissionDef {
        key: "entry:workload",
        label: "分析检测",
        group: "门户入口",
    },
    PermissionDef {
        key: "entry:sample-info",
        label: "样品信息登记",
        group: "门户入口",
    },
    PermissionDef {
        key: "sample-info:collect",
        label: "样品信息-取样操作",
        group: "门户入口",
    },
    PermissionDef {
        key: "sample-info:complete",
        label: "样品信息-完成检测",
        group: "门户入口",
    },
    PermissionDef {
        key: "records:rd:view-all",
        label: "查看全部研发送样记录",
        group: "数据范围",
    },
    PermissionDef {
        key: "records:rd:view-lab",
        label: "查看本实验室研发送样记录",
        group: "数据范围",
    },
    // 系统管理
    PermissionDef {
        key: "manage:projects",
        label: "研发项目管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:groups",
        label: "实验室管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:divisions",
        label: "部门管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:methods",
        label: "检测方法管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:instruments",
        label: "仪器管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:master-import",
        label: "主数据导入",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:trash",
        label: "回收站",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:audit",
        label: "审计日志",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:backup",
        label: "数据备份",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:help",
        label: "教程与帮助",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:sampleinfo",
        label: "样品信息登记管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:users",
        label: "用户管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:roles",
        label: "角色管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:stats",
        label: "统计管理",
        group: "系统管理",
    },
    PermissionDef {
        key: "manage:data-governance:view",
        label: "数据治理查看",
        group: "数据治理",
    },
    PermissionDef {
        key: "manage:data-governance:import",
        label: "业务数据导入",
        group: "数据治理",
    },
    PermissionDef {
        key: "manage:data-governance:export",
        label: "原始数据导出",
        group: "数据治理",
    },
    PermissionDef {
        key: "manage:data-governance:purge",
        label: "永久清理数据",
        group: "数据治理",
    },
    PermissionDef {
        key: "stats:workload:access",
        label: "分析检测统计入口",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:portal:workload",
        label: "查看分析检测统计门户",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:portal:rd",
        label: "查看研发送样统计门户",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:portal:sample-info",
        label: "查看样品信息统计门户",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:week",
        label: "按周统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:month",
        label: "按月统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:user-log",
        label: "检测人记录",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:division",
        label: "事业部统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet1",
        label: "实验室-项目-方法",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet2",
        label: "仪器汇总",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet3",
        label: "项目汇总（含金额）",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet4",
        label: "实验室汇总（含金额）",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet5",
        label: "检测人汇总（原始记录）",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet6",
        label: "检测人汇总表（含系数）",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet7",
        label: "实验室总表",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet8",
        label: "项目总表",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet9",
        label: "仪器类型汇总",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:sheet10",
        label: "理化汇总",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:workload:view-all",
        label: "查看全部统计数据",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:rd:access",
        label: "研发送样统计入口",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:rd:view-all",
        label: "查看全部研发送样统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:rd:view-lab",
        label: "查看本实验室送样统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:sample-info:view-all",
        label: "查看全部样品信息统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "stats:sample-info:view-lab",
        label: "查看本实验室样品信息统计",
        group: "统计管理",
    },
    PermissionDef {
        key: "help:edit",
        label: "编辑帮助文档",
        group: "内容管理",
    },
    PermissionDef {
        key: "help:view",
        label: "帮助与反馈查看",
        group: "内容管理",
    },
    PermissionDef {
        key: "feedback:personnel:view",
        label: "人员变动反馈查看",
        group: "内容管理",
    },
    // 研发送样操作权限
    PermissionDef {
        key: "sample:collect",
        label: "研发送样-取样操作",
        group: "门户入口",
    },
    PermissionDef {
        key: "sample:complete",
        label: "研发送样-完成检测",
        group: "门户入口",
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
