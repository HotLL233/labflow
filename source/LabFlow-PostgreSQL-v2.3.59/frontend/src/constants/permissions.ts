export interface PermissionDef {
  key: string;
  label: string;
  fullLabel: string;
  group: string;
  portal?: string;
  actor?: string;
}

export const PERMISSIONS: PermissionDef[] = [
  { key: 'entry:sample', label: '研发送样门户', fullLabel: '研发送样门户', group: '门户入口', actor: '送样人' },
  { key: 'entry:workload', label: '分析检测门户', fullLabel: '分析检测门户', group: '门户入口', actor: '检测人员' },
  { key: 'entry:sample-info', label: '样品信息登记门户', fullLabel: '样品信息登记门户', group: '门户入口', actor: '送样人' },
  { key: 'sample-info:create', label: '创建送样记录', fullLabel: '样品信息送样-创建送样记录', group: '门户入口', portal: '样品信息登记门户', actor: '送样人' },
  { key: 'sample-info:edit-own', label: '编辑本人记录', fullLabel: '样品信息送样-编辑本人记录', group: '数据范围', portal: '样品信息登记门户', actor: '送样人' },
  { key: 'sample:collect', label: '取样（研发送样）', fullLabel: '分析检测-对研发送样取样', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample:record-workload', label: '录入取样工作量（研发送样）', fullLabel: '分析检测-研发送样取样后录入工作量', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample:withdraw', label: '撤回取样（研发送样）', fullLabel: '分析检测-撤回研发送样取样', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample:return', label: '退回（研发送样）', fullLabel: '分析检测-退回研发送样', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample:complete', label: '完成检测（研发送样）', fullLabel: '分析检测-完成研发送样检测', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample-info:collect', label: '取样（样品信息）', fullLabel: '分析检测-对样品信息取样', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample-info:record-workload', label: '录入取样工作量（样品信息）', fullLabel: '分析检测-样品信息取样后录入工作量', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample-info:complete', label: '完成检测（样品信息）', fullLabel: '分析检测-完成样品信息检测', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample-info:withdraw', label: '撤回取样（样品信息）', fullLabel: '分析检测-撤回样品信息取样', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample-info:return', label: '退回（样品信息）', fullLabel: '分析检测-退回样品信息', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'sample-info:return-confirm', label: '确认并处理退回', fullLabel: '样品信息送样-确认并处理退回', group: '数据范围', portal: '样品信息登记门户', actor: '送样人' },
  { key: 'records:rd:view-all', label: '查看全部研发送样记录', fullLabel: '查看全部研发送样记录', group: '数据范围', portal: '研发送样门户' },
  { key: 'records:rd:view-lab', label: '查看本实验室研发送样记录', fullLabel: '查看本实验室研发送样记录', group: '数据范围', portal: '研发送样门户' },
  { key: 'records:rd:portal-all-labs', label: '研发送样门户查看全部实验室', fullLabel: '研发送样门户查看全部实验室', group: '数据范围', portal: '研发送样门户' },
  { key: 'records:rd:select-sender', label: '研发送样选择实际送样人', fullLabel: '研发送样选择实际送样人', group: '数据范围', portal: '研发送样门户', actor: '送样人' },
  { key: 'records:rd:edit-created', label: '编辑本人提交的记录', fullLabel: '研发送样-编辑本人提交的记录', group: '数据范围', portal: '研发送样门户', actor: '送样人' },
  { key: 'records:rd:edit-subject', label: '编辑本人送样记录', fullLabel: '研发送样-编辑本人作为送样人的记录', group: '数据范围', portal: '研发送样门户', actor: '送样人' },
  { key: 'records:rd:return-confirm', label: '确认并处理退回', fullLabel: '研发送样-确认并处理退回', group: '数据范围', portal: '研发送样门户', actor: '送样人' },
  { key: 'records:rd:return-delegate', label: '代确认并修改已退回的研发送样记录', fullLabel: '代确认并修改已退回的研发送样记录', group: '数据范围', portal: '研发送样门户' },
  { key: 'records:work:portal-all-labs', label: '分析检测门户查看全部实验室', fullLabel: '分析检测门户查看全部实验室', group: '数据范围', portal: '分析检测门户' },
  { key: 'records:work:portal-scoped', label: '分析检测公共账号人员选择门户', fullLabel: '分析检测公共账号人员选择门户', group: '门户入口', portal: '分析检测门户', actor: '检测人员' },
  { key: 'records:work:select-detector', label: '分析检测选择实际检测人', fullLabel: '分析检测选择实际检测人', group: '数据范围', portal: '分析检测门户', actor: '检测人员' },
  { key: 'records:work:view-scope', label: '分析检测查看本范围人员工作量', fullLabel: '分析检测查看本范围人员工作量', group: '数据范围', portal: '分析检测门户' },
  { key: 'records:work:view-all', label: '分析检测查看全公司工作量', fullLabel: '分析检测查看全公司工作量', group: '数据范围', portal: '分析检测门户' },

  { key: 'manage:projects', label: '研发项目管理', fullLabel: '研发项目管理', group: '系统管理' },
  { key: 'manage:groups', label: '实验室管理', fullLabel: '实验室管理', group: '系统管理' },
  { key: 'manage:divisions', label: '部门管理', fullLabel: '部门管理', group: '系统管理' },
  { key: 'manage:methods', label: '检测方法管理', fullLabel: '检测方法管理', group: '系统管理' },
  { key: 'manage:instruments', label: '仪器管理', fullLabel: '仪器管理', group: '系统管理' },
  { key: 'manage:master-import', label: '主数据管理', fullLabel: '主数据管理', group: '系统管理' },
  { key: 'manage:trash', label: '回收站', fullLabel: '回收站', group: '系统管理' },
  { key: 'manage:audit', label: '审计日志', fullLabel: '审计日志', group: '系统管理' },
  { key: 'manage:backup', label: '数据备份', fullLabel: '数据备份', group: '系统管理' },
  { key: 'manage:help', label: '帮助内容管理', fullLabel: '帮助内容管理', group: '系统管理' },
  { key: 'manage:sampleinfo', label: '样品信息登记管理', fullLabel: '样品信息登记管理', group: '系统管理' },
  { key: 'manage:users', label: '用户管理', fullLabel: '用户管理', group: '系统管理' },
  { key: 'manage:roles', label: '角色管理', fullLabel: '角色管理', group: '系统管理' },
  { key: 'manage:settings', label: '页面与录入表单配置', fullLabel: '页面与录入表单配置', group: '系统管理' },
  { key: 'manage:notifications', label: '通知中心配置', fullLabel: '通知中心配置', group: '系统管理' },
  { key: 'manage:stats', label: '统计管理', fullLabel: '统计管理', group: '系统管理' },
  { key: 'manage:log-maintenance', label: '日志与维护', fullLabel: '日志与维护', group: '系统管理' },
  { key: 'session:extended', label: '延长登录空闲时长（1000分钟）', fullLabel: '延长登录空闲时长（1000分钟）', group: '系统管理' },

  { key: 'manage:data-governance:view', label: '数据治理查看', fullLabel: '数据治理查看', group: '数据治理' },
  { key: 'manage:data-governance:import', label: '业务数据导入', fullLabel: '业务数据导入', group: '数据治理' },
  { key: 'manage:data-governance:export', label: '原始数据导出', fullLabel: '原始数据导出', group: '数据治理' },
  { key: 'manage:data-governance:purge', label: '永久清理数据', fullLabel: '永久清理数据', group: '数据治理' },

  { key: 'stats:workload:access', label: '分析检测统计入口', fullLabel: '分析检测统计入口', group: '统计管理' },
  { key: 'stats:portal:workload', label: '查看分析检测统计门户', fullLabel: '查看分析检测统计门户', group: '统计管理' },
  { key: 'stats:portal:rd', label: '查看研发送样统计门户', fullLabel: '查看研发送样统计门户', group: '统计管理' },
  { key: 'stats:portal:sample-info', label: '查看样品信息统计门户', fullLabel: '查看样品信息统计门户', group: '统计管理' },
  { key: 'stats:workload:week', label: '按周统计', fullLabel: '按周统计', group: '统计管理' },
  { key: 'stats:workload:month', label: '按月统计', fullLabel: '按月统计', group: '统计管理' },
  { key: 'stats:workload:user-log', label: '检测人记录', fullLabel: '检测人记录', group: '统计管理' },
  { key: 'stats:workload:division', label: '事业部统计', fullLabel: '事业部统计', group: '统计管理' },
  { key: 'stats:workload:sheet1', label: '实验室-项目-方法', fullLabel: '实验室-项目-方法', group: '统计管理' },
  { key: 'stats:workload:sheet2', label: '仪器汇总', fullLabel: '仪器汇总', group: '统计管理' },
  { key: 'stats:workload:sheet3', label: '项目汇总（含金额）', fullLabel: '项目汇总（含金额）', group: '统计管理' },
  { key: 'stats:workload:sheet4', label: '实验室汇总（含金额）', fullLabel: '实验室汇总（含金额）', group: '统计管理' },
  { key: 'stats:workload:sheet5', label: '检测人汇总（原始记录）', fullLabel: '检测人汇总（原始记录）', group: '统计管理' },
  { key: 'stats:workload:sheet6', label: '检测人汇总表（含系数）', fullLabel: '检测人汇总表（含系数）', group: '统计管理' },
  { key: 'stats:workload:sheet7', label: '实验室总表', fullLabel: '实验室总表', group: '统计管理' },
  { key: 'stats:workload:sheet8', label: '项目总表', fullLabel: '项目总表', group: '统计管理' },
  { key: 'stats:workload:sheet9', label: '仪器类型汇总', fullLabel: '仪器类型汇总', group: '统计管理' },
  { key: 'stats:workload:sheet10', label: '理化汇总', fullLabel: '理化汇总', group: '统计管理' },
  { key: 'stats:workload:view-all', label: '查看全部统计数据', fullLabel: '查看全部统计数据', group: '统计管理' },
  { key: 'stats:workload:export', label: '导出分析检测统计', fullLabel: '导出分析检测统计', group: '统计管理' },
  { key: 'stats:rd:access', label: '研发送样统计入口', fullLabel: '研发送样统计入口', group: '统计管理' },
  { key: 'stats:rd:view-all', label: '查看全部研发送样统计', fullLabel: '查看全部研发送样统计', group: '统计管理' },
  { key: 'stats:rd:view-lab', label: '查看本实验室送样统计', fullLabel: '查看本实验室送样统计', group: '统计管理' },
  // 研发送样统计卡片权限（细粒度控制）
  { key: 'stats:rd:week', label: '按周统计', fullLabel: '研发送样统计-按周统计卡片', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:month', label: '按月统计', fullLabel: '研发送样统计-按月统计卡片', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:user-log', label: '送样人记录', fullLabel: '研发送样统计-送样人记录卡片', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:division', label: '事业部统计', fullLabel: '研发送样统计-事业部统计卡片', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet1', label: 'Sheet1 实验室-项目-方法', fullLabel: '研发送样统计-Sheet1导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet2', label: 'Sheet2 仪器汇总', fullLabel: '研发送样统计-Sheet2导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet3', label: 'Sheet3 项目汇总', fullLabel: '研发送样统计-Sheet3导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet4', label: 'Sheet4 实验室汇总', fullLabel: '研发送样统计-Sheet4导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet5', label: 'Sheet5 送样人原始记录', fullLabel: '研发送样统计-Sheet5导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet6', label: 'Sheet6 送样人汇总', fullLabel: '研发送样统计-Sheet6导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet7', label: 'Sheet7 实验室总表', fullLabel: '研发送样统计-Sheet7导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet8', label: 'Sheet8 项目总表', fullLabel: '研发送样统计-Sheet8导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet9', label: 'Sheet9 仪器类型汇总', fullLabel: '研发送样统计-Sheet9导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet10', label: 'Sheet10 理化汇总', fullLabel: '研发送样统计-Sheet10导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:rd:sheet11', label: 'Sheet11 事业部汇总', fullLabel: '研发送样统计-Sheet11导出预览', group: '统计管理', portal: '研发送样统计门户', actor: '统计员' },
  { key: 'stats:sample-info:view-all', label: '查看全部样品信息统计', fullLabel: '查看全部样品信息统计', group: '统计管理' },
  { key: 'stats:sample-info:view-lab', label: '查看本实验室样品信息统计', fullLabel: '查看本实验室样品信息统计', group: '统计管理' },

  { key: 'help:view', label: '帮助与反馈查看', fullLabel: '帮助与反馈查看', group: '内容管理' },
  { key: 'feedback:personnel:view', label: '人员变动反馈查看', fullLabel: '人员变动反馈查看', group: '内容管理' },
  { key: 'feedback:personnel:edit', label: '人员变动反馈编辑', fullLabel: '人员变动反馈编辑', group: '内容管理' },
  { key: 'help:edit', label: '编辑帮助文档', fullLabel: '编辑帮助文档', group: '内容管理' },
];

export const PERMISSION_GROUPS: string[] = ['门户入口', '数据范围', '系统管理', '数据治理', '统计管理', '内容管理'];

export const ALL_PERMISSION = '*';

export const hasPermission = (perms: string[], key: string): boolean =>
  perms.includes(ALL_PERMISSION) || perms.includes(key);

export const hasAnyPrefix = (perms: string[], prefix: string): boolean =>
  perms.includes(ALL_PERMISSION) || perms.some((p) => p.startsWith(prefix));
