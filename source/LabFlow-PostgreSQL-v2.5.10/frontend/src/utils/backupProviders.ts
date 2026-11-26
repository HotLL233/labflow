import type { BackupCloudInput, BackupCloudSettings } from '../api/backupCloud';

export const BACKUP_PROVIDERS = [
  { id: 'nutstore', label: '坚果云', endpoint: 'https://dav.jianguoyun.com/dav/LabFlowBackups/', placeholder: 'https://dav.jianguoyun.com/dav/同步文件夹/备份目录/', hint: '使用坚果云应用密码；服务根 /dav/ 不能直接存放文件，需具体备份目录。' },
  { id: 'onedrive', label: 'OneDrive（微软官方授权）', endpoint: '', placeholder: '', hint: '支持微软全球版个人及工作/学校账号。需注册允许公共客户端设备授权的微软应用，授权 User.Read、Files.ReadWrite 与 offline_access。世纪互联版暂不适用。' },
  { id: 'nextcloud', label: 'Nextcloud', endpoint: '', placeholder: 'https://服务器/remote.php/dav/files/用户名/LabFlowBackups/', hint: '使用 Nextcloud 的完整 WebDAV 用户目录和应用密码。' },
  { id: 'owncloud', label: 'ownCloud', endpoint: '', placeholder: 'https://服务器/remote.php/dav/files/用户名/LabFlowBackups/', hint: '使用 ownCloud 实际提供的 WebDAV 地址和应用密码。' },
  { id: 'openlist', label: 'OpenList 网关', endpoint: '', placeholder: 'https://网关/dav/挂载名/LabFlowBackups/', hint: '网关需先挂载并授权目标网盘；这里填写网关账号密码，目标目录需有写入权限。' },
  { id: 'baidu_gateway', label: '百度网盘（经 OpenList 网关）', endpoint: '', placeholder: 'https://网关/dav/百度网盘/LabFlowBackups/', hint: '这是已授权网关接入，不是百度账号原生登录；使用网关的 WebDAV 地址和账号密码。' },
  { id: 'aliyun_gateway', label: '阿里云盘（经 OpenList 网关）', endpoint: '', placeholder: 'https://网关/dav/阿里云盘/LabFlowBackups/', hint: '这是已授权网关接入；服务商与网关必须允许上传、下载和删除测试文件。' },
  { id: 'quark_gateway', label: '夸克网盘（经 OpenList 网关）', endpoint: '', placeholder: 'https://网关/dav/夸克网盘/LabFlowBackups/', hint: '需先在网关授权并启用 WebDAV 写入；使用网关账号密码。' },
  { id: 'webdav', label: '其他 HTTPS WebDAV', endpoint: '', placeholder: 'https://服务器/实际备份目录/', hint: '父目录需要已存在；测试或上传可创建最后一级备份目录。' },
] as const;

export const EMPTY_ONEDRIVE = { client_id: '', tenant: 'common', folder: 'LabFlowBackups', connected: false, account: '' };

export function cloudSettingsPayload(settings: BackupCloudSettings, password: string): BackupCloudInput {
  const drive = settings.onedrive || EMPTY_ONEDRIVE;
  return {
    provider: settings.provider || 'webdav', enabled: settings.enabled, auto_upload: settings.auto_upload,
    endpoint: settings.endpoint, username: settings.username, password: password || undefined,
    onedrive: settings.provider === 'onedrive' ? { client_id: drive.client_id, tenant: drive.tenant, folder: drive.folder } : undefined,
  };
}
