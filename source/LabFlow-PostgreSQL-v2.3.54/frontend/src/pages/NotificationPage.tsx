import React, { useCallback, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Alert, Box, Button, Checkbox, Chip, Dialog, DialogActions, DialogContent, DialogTitle,
  FormControl, FormControlLabel, Grid, InputLabel, MenuItem, Paper, Select, Stack, Switch,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, TextField, Typography,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import AddIcon from '@mui/icons-material/Add';
import SendIcon from '@mui/icons-material/Send';
import RefreshIcon from '@mui/icons-material/Refresh';
import EditIcon from '@mui/icons-material/Edit';
import { useUser } from '../UserContext';
import {
  createNotificationChannel, createNotificationRule, getGroups, getNotificationChannels,
  getNotificationDeliveries, getNotificationRules, getNotificationSummary, processNotifications,
  testNotificationChannel, updateNotificationChannel, updateNotificationRule, userList, getSampleInfoTypesAll,
} from '../api/client';
import type { NotificationChannel, NotificationDelivery, NotificationRule, NotificationSummary, ProjectGroup, User, SampleInfoType } from '../types';
import NotificationTemplateEditor from '../components/NotificationTemplateEditor';

const emptyChannel = { name: '', webhook_url: '', secret: '', is_active: true };
const emptyRule = { name: '', group_ids: [] as string[], project_name: '', detection_type: '', sample_info_type_key: '', targetKeys: [] as string[], channel_id: '', recipient_user_ids: [] as number[], is_default: false, is_active: true };

const NotificationPage: React.FC = () => {
  const navigate = useNavigate();
  const { user, hasPermission } = useUser();
  const canManage = Boolean(user?.is_admin || hasPermission('manage:notifications'));
  const [channels, setChannels] = useState<NotificationChannel[]>([]);
  const [rules, setRules] = useState<NotificationRule[]>([]);
  const [deliveries, setDeliveries] = useState<NotificationDelivery[]>([]);
  const [summary, setSummary] = useState<NotificationSummary | null>(null);
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [users, setUsers] = useState<User[]>([]);
  const [sampleTypes, setSampleTypes] = useState<SampleInfoType[]>([]);
  const [channelDialog, setChannelDialog] = useState(false);
  const [ruleDialog, setRuleDialog] = useState(false);
  const [editingChannel, setEditingChannel] = useState<NotificationChannel | null>(null);
  const [editingRule, setEditingRule] = useState<NotificationRule | null>(null);
  const [channelForm, setChannelForm] = useState(emptyChannel);
  const [ruleForm, setRuleForm] = useState(emptyRule);
  const [message, setMessage] = useState<{ text: string; error?: boolean } | null>(null);

  const load = useCallback(async () => {
    try {
      const [s, c, r, d, g, u, t] = await Promise.all([getNotificationSummary(), getNotificationChannels(), getNotificationRules(), getNotificationDeliveries(), getGroups(), userList(), getSampleInfoTypesAll()]);
      if (s.code === 0) setSummary(s.data || null);
      if (c.code === 0) setChannels(c.data || []);
      if (r.code === 0) setRules(r.data || []);
      if (d.code === 0) setDeliveries(d.data || []);
      if (g.code === 0) setGroups(g.data || []);
      if (u.code === 0) setUsers(u.data || []);
      if (t.code === 0) setSampleTypes(t.data || []);
    } catch (error: any) { setMessage({ text: error?.message || '通知中心加载失败', error: true }); }
  }, []);
  useEffect(() => { void load(); }, [load]);

  const openChannel = (channel?: NotificationChannel) => {
    setEditingChannel(channel || null);
    setChannelForm(channel ? { name: channel.name, webhook_url: '', secret: '', is_active: channel.is_active } : emptyChannel);
    setChannelDialog(true);
  };
  const saveChannel = async () => {
    if (!channelForm.name.trim() || (!editingChannel && !channelForm.webhook_url.trim())) { setMessage({ text: '请填写渠道名称和钉钉 Webhook 地址', error: true }); return; }
    try {
      const response = editingChannel
        ? await updateNotificationChannel(editingChannel.id, { name: channelForm.name, webhook_url: channelForm.webhook_url.trim() || undefined, secret: channelForm.secret.trim() || undefined, is_active: channelForm.is_active })
        : await createNotificationChannel({ ...channelForm, webhook_url: channelForm.webhook_url.trim() });
      if (response.code !== 0) throw new Error(response.message);
      setChannelDialog(false); setMessage({ text: editingChannel ? '通知渠道已更新' : '通知渠道已创建' }); await load();
    } catch (error: any) { setMessage({ text: error?.message || '保存失败', error: true }); }
  };
  const analysisUsers = users.filter(item => item.is_active && ((item.permissions || []).includes('*') || (item.permissions || []).includes('entry:workload')));
  const openRule = (rule?: NotificationRule) => {
    setEditingRule(rule || null);
    const targetKeys = rule?.targets?.map(target => target.target_kind === 'rd_work_record' || target.target_kind === 'rd_work_record_rejected' || target.target_kind === 'rd_work_record_resubmitted' || target.target_kind === 'personnel_change_feedback' || target.target_kind === 'personnel_change_feedback_rejected' ? target.target_kind : `sample_info:${target.sample_info_type_key || '*'}`) || [];
    setRuleForm(rule ? { name: rule.name, group_ids: (rule.group_ids?.length ? rule.group_ids : rule.group_id ? [rule.group_id] : []).map(String), project_name: rule.project_name, detection_type: rule.detection_type, sample_info_type_key: rule.sample_info_type_key || '', targetKeys, channel_id: rule.channel_id ? String(rule.channel_id) : '', recipient_user_ids: rule.recipient_user_ids, is_default: rule.is_default, is_active: rule.is_active } : emptyRule);
    setRuleDialog(true);
  };
  const saveRule = async () => {
    if (!ruleForm.name.trim()) { setMessage({ text: '请填写规则名称', error: true }); return; }
    try {
      const isRejectionRule = ruleForm.targetKeys.includes('rd_work_record_rejected');
      if (isRejectionRule && !ruleForm.channel_id) { setMessage({ text: '研发送样驳回通知必须选择钉钉群通知渠道', error: true }); return; }
      const targets: Array<{ target_kind: 'rd_work_record' | 'rd_work_record_rejected' | 'rd_work_record_resubmitted' | 'sample_info' | 'personnel_change_feedback' | 'personnel_change_feedback_rejected'; sample_info_type_key: string }> = ruleForm.targetKeys.map(value => value === 'rd_work_record' || value === 'rd_work_record_rejected' || value === 'rd_work_record_resubmitted' || value === 'personnel_change_feedback' || value === 'personnel_change_feedback_rejected'
        ? { target_kind: value, sample_info_type_key: '' }
        : { target_kind: 'sample_info' as const, sample_info_type_key: value.replace(/^sample_info:/, '') === '*' ? '' : value.replace(/^sample_info:/, '') });
      const data = { ...ruleForm, targets, group_ids: ruleForm.group_ids.map(Number), channel_id: ruleForm.channel_id ? Number(ruleForm.channel_id) : null, recipient_user_ids: isRejectionRule ? [] : ruleForm.recipient_user_ids };
      const response = editingRule ? await updateNotificationRule(editingRule.id, data) : await createNotificationRule(data);
      if (response.code !== 0) throw new Error(response.message);
      setRuleDialog(false); setMessage({ text: editingRule ? '通知规则已更新' : '通知规则已创建' }); await load();
    } catch (error: any) { setMessage({ text: error?.message || '保存失败', error: true }); }
  };
  const test = async (id: number) => { try { const r = await testNotificationChannel(id); if (r.code !== 0) throw new Error(r.message); setMessage({ text: r.message || '测试消息已发送' }); await load(); } catch (e: any) { setMessage({ text: e?.message || '测试发送失败', error: true }); } };
  const process = async () => { try { const r = await processNotifications(); if (r.code !== 0) throw new Error(r.message); setMessage({ text: r.message || '已处理待发送通知' }); await load(); } catch (e: any) { setMessage({ text: e?.message || '处理失败', error: true }); } };

  return <Box>
    <NotificationTemplateEditor canManage={canManage} onMessage={setMessage} />
    <Box sx={{ position: 'sticky', top: { xs: 56, md: 64 }, zIndex: 4, bgcolor: 'background.default', py: 1, mb: 2, borderBottom: '1px solid rgba(0,0,0,0.08)', display: 'flex', alignItems: 'center', gap: 1.25, flexWrap: 'wrap' }}>
      <Button variant="outlined" startIcon={<ArrowBackIcon />} onClick={() => navigate('/manage')}>返回管理工作区</Button>
      <Box sx={{ flex: 1 }}><Typography variant="h5" fontWeight={800}>通知中心</Typography><Typography variant="body2" color="text.secondary">送样成功后按实验室、项目或检测类型路由至分析检测钉钉群。</Typography></Box>
      <Button variant="outlined" startIcon={<RefreshIcon />} onClick={() => void load()}>刷新</Button>
      {canManage && <Button variant="contained" startIcon={<SendIcon />} onClick={() => void process()}>处理待发送</Button>}
    </Box>
    {message && <Alert severity={message.error ? 'error' : 'success'} onClose={() => setMessage(null)} sx={{ mb: 2 }}>{message.text}</Alert>}
    <Grid container spacing={1.5} sx={{ mb: 2 }}>
      {[['待发送', summary?.pending_count || 0, '#1565c0'], ['发送失败', summary?.failed_count || 0, '#c62828'], ['今日已发送', summary?.sent_today_count || 0, '#2e7d32'], ['未配置规则', summary?.skipped_count || 0, '#6b7280']].map(([label, value, color]) => <Grid item xs={12} sm={6} md={3} key={String(label)}><Paper variant="outlined" sx={{ p: 1.5, borderTop: `3px solid ${color}` }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography variant="h5" fontWeight={800}>{value}</Typography></Paper></Grid>)}
    </Grid>
    <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, mb: 1.5 }}><Box><Typography fontWeight={800}>钉钉通知渠道</Typography><Typography variant="caption" color="text.secondary">机器人地址和加签密钥不会在页面中回显。</Typography></Box>{canManage && <Button size="small" variant="contained" startIcon={<AddIcon />} onClick={() => openChannel()}>新增渠道</Button>}</Box>
      <TableContainer><Table size="small"><TableHead><TableRow><TableCell>名称</TableCell><TableCell>Webhook</TableCell><TableCell>加签</TableCell><TableCell>状态</TableCell><TableCell align="right">操作</TableCell></TableRow></TableHead><TableBody>{channels.map(channel => <TableRow key={channel.id}><TableCell>{channel.name}</TableCell><TableCell sx={{ fontFamily: 'monospace', fontSize: 12 }}>{channel.webhook_url_masked}</TableCell><TableCell>{channel.has_secret ? '已配置' : '未配置'}</TableCell><TableCell><Chip size="small" label={channel.is_active ? '启用' : '停用'} color={channel.is_active ? 'success' : 'default'} /></TableCell><TableCell align="right">{canManage && <><Button size="small" onClick={() => void test(channel.id)}>测试</Button><Button size="small" startIcon={<EditIcon />} onClick={() => openChannel(channel)}>编辑</Button></>}</TableCell></TableRow>)}{channels.length === 0 && <TableRow><TableCell colSpan={5} align="center">尚未配置钉钉机器人</TableCell></TableRow>}</TableBody></Table></TableContainer>
    </Paper>
    <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, mb: 1.5 }}><Box><Typography fontWeight={800}>通知规则</Typography><Typography variant="caption" color="text.secondary">普通送样、人员变动反馈和审批通知均可独立配置；人员变动业务明细仍保存在独立台账。</Typography></Box>{canManage && <Button size="small" variant="contained" startIcon={<AddIcon />} onClick={() => openRule()}>新增规则</Button>}</Box>
      <TableContainer><Table size="small"><TableHead><TableRow><TableCell>规则</TableCell><TableCell>通知事件 / 匹配范围</TableCell><TableCell>目标渠道</TableCell><TableCell>人员范围</TableCell><TableCell>状态</TableCell><TableCell align="right">操作</TableCell></TableRow></TableHead><TableBody>{rules.map(rule => { const targets = (rule.targets || []).map(target => target.target_kind === 'personnel_change_feedback_rejected' ? '人员通知反馈驳回' : target.target_kind === 'personnel_change_feedback' ? '人员变动反馈' : target.target_kind === 'rd_work_record_rejected' ? '研发送样驳回' : target.target_kind === 'rd_work_record_resubmitted' ? '退回后重新提交' : target.target_kind === 'rd_work_record' ? '研发送样' : target.sample_info_type_key ? (sampleTypes.find(type => type.type_key === target.sample_info_type_key)?.label || target.sample_info_type_key) : '样品信息登记（全部类型）'); const rejection = rule.event_type === 'rd_record_rejected' || targets.includes('研发送样驳回'); const personnelRejection = targets.includes('人员通知反馈驳回'); return <TableRow key={rule.id}><TableCell>{rule.name}{rule.is_default && <Chip size="small" label="默认" sx={{ ml: 0.75 }} />}</TableCell><TableCell><Typography variant="body2" fontWeight={rejection || personnelRejection ? 700 : 400}>{[targets.join('、'), rule.group_names?.length ? rule.group_names.join('、') : rule.group_name, rule.project_name].filter(Boolean).join(' / ') || '全部送样'}</Typography></TableCell><TableCell>{rule.channel_name || '仅程序内通知'}</TableCell><TableCell>{rejection ? '仅钉钉群' : personnelRejection && !rule.recipient_user_ids.length ? '反馈发起人' : rule.recipient_user_ids.length ? `${rule.recipient_user_ids.length} 人` : '全部分析人员'}</TableCell><TableCell><Chip size="small" label={rule.is_active ? '启用' : '停用'} color={rule.is_active ? 'success' : 'default'} /></TableCell><TableCell align="right">{canManage && <Button size="small" startIcon={<EditIcon />} onClick={() => openRule(rule)}>编辑</Button>}</TableCell></TableRow>})}{rules.length === 0 && <TableRow><TableCell colSpan={6} align="center">尚未配置规则。建议先建立普通送样规则、驳回通知规则或人员变动反馈规则。</TableCell></TableRow>}</TableBody></Table></TableContainer>
    </Paper>
    <Paper variant="outlined" sx={{ p: 2 }}><Typography fontWeight={800} sx={{ mb: 1.5 }}>发送记录</Typography><TableContainer><Table size="small"><TableHead><TableRow><TableCell>时间</TableCell><TableCell>渠道</TableCell><TableCell>状态</TableCell><TableCell>尝试次数</TableCell><TableCell>结果</TableCell></TableRow></TableHead><TableBody>{deliveries.map(item => <TableRow key={item.id}><TableCell>{item.sent_at || item.created_at}</TableCell><TableCell>{item.channel_name}</TableCell><TableCell><Chip size="small" label={item.status === 'sent' ? '已发送' : item.status === 'retry' ? '重试中' : '失败'} color={item.status === 'sent' ? 'success' : item.status === 'retry' ? 'warning' : 'error'} /></TableCell><TableCell>{item.attempts}</TableCell><TableCell sx={{ maxWidth: 420, whiteSpace: 'normal', wordBreak: 'break-word' }}>{item.last_error || item.response_summary || '-'}</TableCell></TableRow>)}{deliveries.length === 0 && <TableRow><TableCell colSpan={5} align="center">暂无发送记录</TableCell></TableRow>}</TableBody></Table></TableContainer></Paper>
    <Dialog open={channelDialog} onClose={() => setChannelDialog(false)} fullWidth maxWidth="sm"><DialogTitle>{editingChannel ? '编辑钉钉通知渠道' : '新增钉钉通知渠道'}</DialogTitle><DialogContent><Stack spacing={2} sx={{ pt: 1 }}><TextField label="渠道名称" value={channelForm.name} onChange={e => setChannelForm(v => ({ ...v, name: e.target.value }))} fullWidth /><TextField label="钉钉 Webhook" placeholder="https://oapi.dingtalk.com/robot/send?access_token=..." value={channelForm.webhook_url} onChange={e => setChannelForm(v => ({ ...v, webhook_url: e.target.value }))} fullWidth helperText={editingChannel ? '留空保留原 Webhook；填写新地址才会替换。' : ''} /><TextField label="加签密钥（可选）" type="password" value={channelForm.secret} onChange={e => setChannelForm(v => ({ ...v, secret: e.target.value }))} fullWidth helperText={editingChannel ? '留空保留原密钥；填写新密钥才会替换。' : '未启用钉钉加签时留空。'} /><FormControlLabel control={<Switch checked={channelForm.is_active} onChange={e => setChannelForm(v => ({ ...v, is_active: e.target.checked }))} />} label="启用渠道" /></Stack></DialogContent><DialogActions><Button onClick={() => setChannelDialog(false)}>取消</Button><Button variant="contained" onClick={() => void saveChannel()}>保存</Button></DialogActions></Dialog>
    <Dialog open={ruleDialog} onClose={() => setRuleDialog(false)} fullWidth maxWidth="sm"><DialogTitle>{editingRule ? '编辑通知规则' : '新增通知规则'}</DialogTitle><DialogContent><Stack spacing={2} sx={{ pt: 1 }}><TextField label="规则名称" value={ruleForm.name} onChange={e => setRuleForm(v => ({ ...v, name: e.target.value }))} fullWidth /><FormControl fullWidth><InputLabel>通知来源与类型</InputLabel><Select multiple label="通知来源与类型" value={ruleForm.targetKeys} onChange={e => { const values = e.target.value as string[]; const special = values.find(value => value === 'rd_work_record_rejected' || value === 'rd_work_record_resubmitted' || value === 'personnel_change_feedback' || value === 'personnel_change_feedback_rejected'); const normalized = special ? [special] : values.filter(value => value !== 'rd_work_record_rejected' && value !== 'rd_work_record_resubmitted' && value !== 'personnel_change_feedback' && value !== 'personnel_change_feedback_rejected'); setRuleForm(v => ({ ...v, targetKeys: normalized, recipient_user_ids: special === 'rd_work_record_rejected' ? [] : v.recipient_user_ids })); }} renderValue={values => values.length ? values.map(value => value === 'rd_work_record_rejected' ? '研发送样驳回' : value === 'rd_work_record_resubmitted' ? '退回后重新提交' : value === 'rd_work_record' ? '研发送样' : value === 'personnel_change_feedback' ? '人员变动反馈' : value === 'personnel_change_feedback_rejected' ? '人员通知反馈驳回' : value === 'sample_info:*' ? '样品信息登记（全部类型）' : sampleTypes.find(type => `sample_info:${type.type_key}` === value)?.label || value).join('、') : (ruleForm.is_default ? '全部来源' : '请选择')}><MenuItem value="rd_work_record_rejected"><Checkbox checked={ruleForm.targetKeys.includes('rd_work_record_rejected')} />研发送样驳回（仅钉钉群）</MenuItem><MenuItem value="rd_work_record_resubmitted"><Checkbox checked={ruleForm.targetKeys.includes('rd_work_record_resubmitted')} />退回后重新提交（独立规则）</MenuItem><MenuItem value="rd_work_record"><Checkbox checked={ruleForm.targetKeys.includes('rd_work_record')} />研发送样</MenuItem><MenuItem value="personnel_change_feedback"><Checkbox checked={ruleForm.targetKeys.includes('personnel_change_feedback')} />人员变动反馈</MenuItem><MenuItem value="personnel_change_feedback_rejected"><Checkbox checked={ruleForm.targetKeys.includes('personnel_change_feedback_rejected')} />人员通知反馈驳回</MenuItem><MenuItem value="sample_info:*"><Checkbox checked={ruleForm.targetKeys.includes('sample_info:*')} />样品信息登记（全部类型）</MenuItem>{sampleTypes.map(type => <MenuItem key={type.type_key} value={`sample_info:${type.type_key}`} disabled={!type.is_active}><Checkbox checked={ruleForm.targetKeys.includes(`sample_info:${type.type_key}`)} />{type.label}{type.is_active ? '' : '（已停用）'}</MenuItem>)}</Select></FormControl><FormControl fullWidth><InputLabel>所属实验室</InputLabel><Select multiple label="所属实验室" value={ruleForm.group_ids} onChange={e => setRuleForm(v => ({ ...v, group_ids: e.target.value as string[] }))} renderValue={values => values.length ? values.map(value => groups.find(group => String(group.id) === value)?.name || value).join('、') : '全部实验室'}>{groups.map(group => <MenuItem key={group.id} value={String(group.id)}><Checkbox checked={ruleForm.group_ids.includes(String(group.id))} />{group.name}</MenuItem>)}</Select></FormControl><TextField label="项目名称（可选）" value={ruleForm.project_name} onChange={e => setRuleForm(v => ({ ...v, project_name: e.target.value }))} fullWidth /><FormControl fullWidth><InputLabel>钉钉通知渠道</InputLabel><Select label="钉钉通知渠道" value={ruleForm.channel_id} onChange={e => setRuleForm(v => ({ ...v, channel_id: e.target.value as string }))}><MenuItem value="">仅程序内通知</MenuItem>{channels.map(channel => <MenuItem key={channel.id} value={String(channel.id)}>{channel.name}</MenuItem>)}</Select></FormControl><FormControl sx={{ display: ruleForm.targetKeys.includes('rd_work_record_rejected') ? 'none' : undefined }} fullWidth><InputLabel>程序内通知人员</InputLabel><Select multiple label="程序内通知人员" value={ruleForm.recipient_user_ids} onChange={e => setRuleForm(v => ({ ...v, recipient_user_ids: e.target.value as number[] }))} renderValue={values => values.length ? `${values.length} 人` : '人员通知反馈驳回留空则通知反馈发起人'}>{analysisUsers.map(item => <MenuItem key={item.id} value={item.id}><Checkbox checked={ruleForm.recipient_user_ids.includes(item.id)} />{item.username}</MenuItem>)}</Select></FormControl><FormControlLabel control={<Switch checked={ruleForm.is_default} onChange={e => setRuleForm(v => ({ ...v, is_default: e.target.checked }))} />} label="作为当前通知事件未匹配时的默认规则" /><FormControlLabel control={<Switch checked={ruleForm.is_active} onChange={e => setRuleForm(v => ({ ...v, is_active: e.target.checked }))} />} label="启用规则" />{ruleForm.targetKeys.includes('rd_work_record_rejected') && <Alert severity="info">驳回通知只发送到所选钉钉群，不发送程序内个人通知；驳回原因将完整显示。</Alert>}{ruleForm.targetKeys.includes('personnel_change_feedback_rejected') && <Alert severity="info">人员通知反馈驳回默认发送给反馈发起人，也可选择其他程序内通知人员并配置钉钉群。</Alert>}{ruleForm.targetKeys.includes('rd_work_record_resubmitted') && <Alert severity="info">退回后重新提交是独立通知事件，可单独配置钉钉群和程序内分析检测人员。</Alert>}</Stack></DialogContent><DialogActions><Button onClick={() => setRuleDialog(false)}>取消</Button><Button variant="contained" onClick={() => void saveRule()}>保存</Button></DialogActions></Dialog>
  </Box>;
};
export default NotificationPage;
