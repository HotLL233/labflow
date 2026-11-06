import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Alert, Autocomplete, Box, Button, Checkbox, Chip, Dialog, DialogActions, DialogContent, DialogTitle,
  FormControl, FormControlLabel, Grid, InputLabel, MenuItem, Paper, Select, Stack, Switch,
  Tab, Tabs, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, TextField, Typography,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import AddIcon from '@mui/icons-material/Add';
import SendIcon from '@mui/icons-material/Send';
import RefreshIcon from '@mui/icons-material/Refresh';
import EditIcon from '@mui/icons-material/Edit';
import { useUser } from '../UserContext';
import {
  createNotificationChannel, createNotificationRule, getGroups, getNotificationChannels,
  getNotificationDeliveryPage, getNotificationRules, getNotificationSummary, processNotifications, getMethodTypes,
  testNotificationChannel, updateNotificationChannel, updateNotificationRule, getNotificationRecipients, getSampleInfoTypesAll,
  getNotificationEvents, retryNotificationEvent, getDingTalkContacts,
} from '../api/client';
import type { NotificationChannel, NotificationDelivery, NotificationEventStatus, NotificationRecipient, NotificationRule, NotificationRuleTarget, NotificationSummary, ProjectGroup, SampleInfoType } from '../types';
import NotificationTemplateEditor from '../components/NotificationTemplateEditor';
import DingTalkMentions, { DingTalkMentionFields } from '../components/DingTalkMentions';
import type { DingTalkContact } from '../types';
import ManageWorkspace, { ManagePageHeader } from '../components/ManageWorkspace';

const emptyChannel = { name: '', webhook_url: '', secret: '', is_active: true };
const emptyRule = { name: '', group_ids: [] as string[], project_name: '', detection_type: '', sample_info_type_key: '', targetKeys: [] as string[], channel_id: '', recipient_user_ids: [] as number[], dingtalk_mention_user_ids: [] as number[], dingtalk_mention_mode: 'none' as 'none' | 'selected' | 'follow', is_default: false, is_active: true };
const eventLabels: Record<string, string> = { sample_submitted: '送样提交', rd_record_rejected: '研发送样驳回', sample_info_rejected: '样品登记驳回', rd_record_resubmitted: '退回后重新提交', personnel_change_feedback: '人员变动反馈', personnel_change_feedback_rejected: '人员变动驳回', personnel_change_feedback_approved: '人员变动审批通过' };
const statusLabels: Record<string, string> = { pending: '待发送', processing: '处理中', retry: '待重试', partial: '部分失败', failed: '发送失败', sent: '已发送', skipped: '已跳过', cancelled: '已失效' };

const NotificationPage: React.FC = () => {
  const navigate = useNavigate();
  const { user, hasPermission } = useUser();
  const canManage = Boolean(user?.is_admin || hasPermission('manage:notifications'));
  const [channels, setChannels] = useState<NotificationChannel[]>([]);
  const [rules, setRules] = useState<NotificationRule[]>([]);
  const [deliveries, setDeliveries] = useState<NotificationDelivery[]>([]);
  const [deliveryPage, setDeliveryPage] = useState(0);
  const [deliveryPageSize, setDeliveryPageSize] = useState(50);
  const [deliveryTotal, setDeliveryTotal] = useState(0);
  const [deliveryDraft, setDeliveryDraft] = useState({ event_id: '', start_date: '', end_date: '' });
  const [deliveryFilters, setDeliveryFilters] = useState({ event_id: '', start_date: '', end_date: '' });
  const [detectionTypes, setDetectionTypes] = useState<string[]>([]);
  const [events, setEvents] = useState<NotificationEventStatus[]>([]);
  const [eventStatus, setEventStatus] = useState('');
  const [eventKeyword, setEventKeyword] = useState('');
  const [eventSearchDraft, setEventSearchDraft] = useState('');
  const [summary, setSummary] = useState<NotificationSummary | null>(null);
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [users, setUsers] = useState<NotificationRecipient[]>([]);
  const [contacts, setContacts] = useState<DingTalkContact[]>([]);
  const [sampleTypes, setSampleTypes] = useState<SampleInfoType[]>([]);
  const [channelDialog, setChannelDialog] = useState(false);
  const [ruleDialog, setRuleDialog] = useState(false);
  const [editingChannel, setEditingChannel] = useState<NotificationChannel | null>(null);
  const [editingRule, setEditingRule] = useState<NotificationRule | null>(null);
  const [channelForm, setChannelForm] = useState(emptyChannel);
  const [ruleForm, setRuleForm] = useState(emptyRule);
  const [message, setMessage] = useState<{ text: string; error?: boolean } | null>(null);
  const [templateRefreshSignal, setTemplateRefreshSignal] = useState(0);
  const [templateOpen, setTemplateOpen] = useState(false);
  const [section, setSection] = useState('events');
  const [retryTarget, setRetryTarget] = useState<NotificationEventStatus | null>(null);
  const [retryReason, setRetryReason] = useState('');
  const [retrying, setRetrying] = useState(false);
  const retryBusy = useRef(false);
  const loadRequest = useRef(0);

  const load = useCallback(async () => {
    const request = ++loadRequest.current;
    try {
      const [s, c, r, d, g, u, t, e, contactResult, methodTypes] = await Promise.all([getNotificationSummary(), getNotificationChannels(), getNotificationRules(), getNotificationDeliveryPage({ page: deliveryPage + 1, page_size: deliveryPageSize, event_id: deliveryFilters.event_id ? Number(deliveryFilters.event_id) : undefined, start_date: deliveryFilters.start_date || undefined, end_date: deliveryFilters.end_date || undefined }), getGroups(), getNotificationRecipients(), getSampleInfoTypesAll(), getNotificationEvents({ status: eventStatus, keyword: eventKeyword }), getDingTalkContacts(), getMethodTypes()]);
      if (request !== loadRequest.current) return;
      if (s.code === 0) setSummary(s.data || null);
      if (c.code === 0) setChannels(c.data || []);
      if (r.code === 0) setRules(r.data || []);
      if (d.code !== 0 || !d.data) throw new Error(d.message || '发送记录加载失败');
      if (d.data) {
        const lastPage = Math.max(0, Math.ceil(d.data.total / deliveryPageSize) - 1);
        if (deliveryPage > lastPage) setDeliveryPage(lastPage);
        else { setDeliveries(d.data.items); setDeliveryTotal(d.data.total); }
      }
      if (g.code === 0) setGroups(g.data || []);
      if (u.code === 0) setUsers(u.data || []);
      if (contactResult.code === 0) setContacts(contactResult.data || []);
      if (t.code === 0) setSampleTypes(t.data || []);
      if (e.code === 0) setEvents(e.data || []);
      if (methodTypes.code === 0) setDetectionTypes((methodTypes.data || []).map(item => item.name));
    } catch (error: any) { if (request === loadRequest.current) setMessage({ text: error?.message || '通知中心加载失败', error: true }); }
  }, [eventStatus, eventKeyword, deliveryPage, deliveryPageSize, deliveryFilters]);
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
  const personnelRule = ruleForm.targetKeys.some(key => key.startsWith('personnel_change_feedback'));
  const recipientUsers = users.filter(item => personnelRule ? item.personnel : item.analysis);
  const openRule = (rule?: NotificationRule) => {
    setEditingRule(rule || null);
    const targetKeys = rule?.targets?.map(target => target.target_kind === 'sample_info_rejected' ? (target.sample_info_type_key ? `sample_info_rejected:${target.sample_info_type_key}` : target.target_kind) : target.target_kind === 'rd_work_record' || target.target_kind === 'rd_work_record_rejected' || target.target_kind === 'rd_work_record_resubmitted' || target.target_kind === 'personnel_change_feedback' || target.target_kind === 'personnel_change_feedback_rejected' || target.target_kind === 'personnel_change_feedback_approved' ? target.target_kind : `sample_info:${target.sample_info_type_key || '*'}`) || [];
    setRuleForm(rule ? { name: rule.name, group_ids: (rule.group_ids?.length ? rule.group_ids : rule.group_id ? [rule.group_id] : []).map(String), project_name: rule.project_name, detection_type: rule.detection_type, sample_info_type_key: rule.sample_info_type_key || '', targetKeys, channel_id: rule.channel_id ? String(rule.channel_id) : '', recipient_user_ids: rule.recipient_user_ids, dingtalk_mention_user_ids: rule.dingtalk_mention_user_ids || [], dingtalk_mention_mode: rule.dingtalk_follow_recipients ? 'follow' : rule.dingtalk_mention_user_ids?.length ? 'selected' : 'none', is_default: rule.is_default, is_active: rule.is_active } : emptyRule);
    setRuleDialog(true);
  };
  const saveRule = async () => {
    if (!ruleForm.name.trim()) { setMessage({ text: '请填写规则名称', error: true }); return; }
    try {
      if (ruleForm.channel_id && ruleForm.dingtalk_mention_mode === 'selected' && !ruleForm.dingtalk_mention_user_ids.length) { setMessage({ text: '请选择至少一位钉钉 @ 人员，或改为不 @', error: true }); return; }
      const targets: NotificationRuleTarget[] = ruleForm.targetKeys.map(value => value.startsWith('sample_info_rejected:') ? { target_kind: 'sample_info_rejected' as const, sample_info_type_key: value.slice('sample_info_rejected:'.length) } : value === 'rd_work_record' || value === 'rd_work_record_rejected' || value === 'sample_info_rejected' || value === 'rd_work_record_resubmitted' || value === 'personnel_change_feedback' || value === 'personnel_change_feedback_rejected' || value === 'personnel_change_feedback_approved'
        ? { target_kind: value, sample_info_type_key: '' }
        : { target_kind: 'sample_info' as const, sample_info_type_key: value.replace(/^sample_info:/, '') === '*' ? '' : value.replace(/^sample_info:/, '') });
      const data = { ...ruleForm, targets, group_ids: ruleForm.group_ids.map(Number), channel_id: ruleForm.channel_id ? Number(ruleForm.channel_id) : null, recipient_user_ids: ruleForm.recipient_user_ids, dingtalk_mention_user_ids: ruleForm.channel_id && ruleForm.dingtalk_mention_mode === 'selected' ? ruleForm.dingtalk_mention_user_ids : [], dingtalk_follow_recipients: Boolean(ruleForm.channel_id) && ruleForm.dingtalk_mention_mode === 'follow' };
      const response = editingRule ? await updateNotificationRule(editingRule.id, data) : await createNotificationRule(data);
      if (response.code !== 0) throw new Error(response.message);
      setRuleDialog(false); setMessage({ text: editingRule ? '通知规则已更新' : '通知规则已创建' }); await load();
    } catch (error: any) { setMessage({ text: error?.message || '保存失败', error: true }); }
  };
  const test = async (id: number) => { try { const r = await testNotificationChannel(id); if (r.code !== 0) throw new Error(r.message); setMessage({ text: r.message || '测试消息已发送' }); await load(); } catch (e: any) { setMessage({ text: e?.message || '测试发送失败', error: true }); } };
  const process = async () => { try { const r = await processNotifications(); if (r.code !== 0) throw new Error(r.message); setMessage({ text: r.message || '已处理待发送通知' }); await load(); } catch (e: any) { setMessage({ text: e?.message || '处理失败', error: true }); } };
  const queryDeliveries = () => {
    const id = deliveryDraft.event_id.trim();
    if ((id && (!/^[1-9]\d*$/.test(id) || !Number.isSafeInteger(Number(id)))) || (deliveryDraft.start_date && deliveryDraft.end_date && deliveryDraft.start_date > deliveryDraft.end_date)) {
      setMessage({ text: '请填写有效的事件编号和起止日期', error: true }); return;
    }
    setDeliveryPage(0); setDeliveryFilters({ ...deliveryDraft, event_id: id });
  };
  const openDeliveries = (event: NotificationEventStatus) => {
    const filters = { event_id: String(event.id), start_date: '', end_date: '' };
    setDeliveryDraft(filters); setDeliveryFilters(filters); setDeliveryPage(0);
    setSection('deliveries');
  };
  const confirmRetry = async () => {
    if (!canManage || !retryTarget || retryBusy.current) return;
    const reason = retryReason.trim();
    if (!reason || Array.from(reason).length > 500) { setMessage({ text: '请填写1至500字符的补发原因', error: true }); return; }
    retryBusy.current = true;
    setRetrying(true);
    try {
      const response = await retryNotificationEvent(retryTarget.id, reason);
      if (response.code !== 0) throw new Error(response.message);
      setRetryTarget(null);
      setMessage({ text: response.message || '已加入补发队列；成功渠道和已生成的个人通知不会重发' });
      await load();
    } catch (error: any) { setMessage({ text: error?.message || '补发失败', error: true }); }
    finally { retryBusy.current = false; setRetrying(false); }
  };
  const eventActions = (event: NotificationEventStatus) => <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
    <Button size="small" onClick={() => openDeliveries(event)}>发送详情</Button>
    {canManage && ['failed', 'partial', 'skipped'].includes(event.status) && <Button size="small" variant="outlined" onClick={() => { setRetryTarget(event); setRetryReason(''); }}>补发</Button>}
  </Box>;

  return <ManageWorkspace activeKey="notifications">
    <ManagePageHeader title="通知中心" description="送样、驳回、人员变动的通知渠道、模板与发送状态" actions={<>
      <Button variant="outlined" startIcon={<RefreshIcon />} onClick={() => { setTemplateRefreshSignal(value => value + 1); void load(); }}>刷新</Button>
      {canManage && <Button variant="contained" startIcon={<SendIcon />} onClick={() => void process()}>处理待发送</Button>}
    </>} />
    <Box id="notification-summary" sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(3,minmax(0,1fr))', md: 'repeat(5,minmax(0,1fr))' }, gap: 1.5, pb: 2 }}>
      {[['待发送', summary?.pending_count || 0, 'text.primary'], ['发送失败', summary?.failed_count || 0, 'error.main'], ['部分失败', summary?.partial_count || 0, 'warning.main'], ['今日已发送', summary?.sent_today_count || 0, 'success.main'], ['未配置规则', summary?.skipped_count || 0, 'text.secondary']].map(([label, value, color]) => <Box key={String(label)}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography variant="h5" sx={{ fontWeight: 500, color, fontVariantNumeric: 'tabular-nums' }}>{value}</Typography></Box>)}
    </Box>
    <Tabs value={section} onChange={(_, value) => setSection(value)} variant="scrollable" scrollButtons="auto" aria-label="通知中心任务" sx={{ borderBottom: '1px solid', borderColor: 'divider', mb: 1.5 }}>
      <Tab value="events" label="通知事件" /><Tab value="deliveries" label="发送记录" /><Tab value="rules" label="规则与渠道" /><Tab value="templates" label="内容模板" />{canManage && <Tab value="contacts" label="身份绑定" />}
    </Tabs>
    {message && <Alert severity={message.error ? 'error' : 'success'} onClose={() => setMessage(null)} sx={{ mb: 2 }}>{message.text}</Alert>}
    {summary?.legacy_webhook_configured && <Alert severity="warning" sx={{ mb: 2 }}>检测到旧版人员变动 Webhook 环境变量。本版不再直接发送；请将原渠道与接收范围配置到下方通知渠道、规则和模板中。旧配置未自动迁移，请核对规则后显式补发未发送事件。</Alert>}
    {canManage && <Box id="notification-contacts" hidden={section !== 'contacts'}><DingTalkMentions contacts={contacts} channels={channels} onChanged={load} /></Box>}
    <Box id="notification-template" sx={{ display: section === 'templates' ? 'flex' : 'none', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 1.5, py: 1.5 }}>
      <Box><Typography fontWeight={600}>通知内容模板</Typography><Typography variant="caption" color="text.secondary">按来源维护显示字段、名称、顺序及加粗</Typography></Box>
      <Button variant="outlined" onClick={() => setTemplateOpen(true)}>字段配置与预览</Button>
    </Box>
    <Dialog open={templateOpen} keepMounted onClose={() => setTemplateOpen(false)} fullWidth maxWidth="lg">
      <DialogTitle>通知模板配置</DialogTitle>
      <DialogContent>{message && <Alert severity={message.error ? 'error' : 'success'} onClose={() => setMessage(null)} sx={{ mb: 1.5 }}>{message.text}</Alert>}<NotificationTemplateEditor canManage={canManage} onMessage={setMessage} refreshSignal={templateRefreshSignal} /></DialogContent>
      <DialogActions><Typography variant="caption" color="text.secondary" sx={{ mr: 'auto' }}>关闭窗口保留本页草稿；离开通知中心前请保存</Typography><Button onClick={() => setTemplateOpen(false)}>关闭</Button></DialogActions>
    </Dialog>
    <Box id="notification-channels" hidden={section !== 'rules'} sx={{ py: 2, mb: 1, borderBottom: '1px solid', borderColor: 'divider', '& .MuiTable-root': { minWidth: 560 } }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 1, mb: 1.5 }}><Box sx={{ flex: '1 1 200px', minWidth: 0 }}><Typography fontWeight={600}>钉钉通知渠道</Typography><Typography variant="caption" color="text.secondary">机器人地址和加签密钥不会在页面中回显。</Typography></Box>{canManage && <Button size="small" variant="contained" startIcon={<AddIcon />} onClick={() => openChannel()}>新增渠道</Button>}</Box>
      <TableContainer><Table size="small"><TableHead><TableRow><TableCell>名称</TableCell><TableCell>Webhook</TableCell><TableCell>加签</TableCell><TableCell>状态</TableCell><TableCell align="right">操作</TableCell></TableRow></TableHead><TableBody>{channels.map(channel => <TableRow key={channel.id}><TableCell>{channel.name}</TableCell><TableCell sx={{ fontFamily: 'monospace', fontSize: 12 }}>{channel.webhook_url_masked}</TableCell><TableCell>{channel.has_secret ? '已配置' : '未配置'}</TableCell><TableCell><Chip size="small" label={channel.is_active ? '启用' : '停用'} color={channel.is_active ? 'success' : 'default'} /></TableCell><TableCell align="right">{canManage && <><Button size="small" onClick={() => void test(channel.id)}>测试</Button><Button size="small" startIcon={<EditIcon />} onClick={() => openChannel(channel)}>编辑</Button></>}</TableCell></TableRow>)}{channels.length === 0 && <TableRow><TableCell colSpan={5} align="center">尚未配置钉钉机器人</TableCell></TableRow>}</TableBody></Table></TableContainer>
    </Box>
    <Box id="notification-rules" hidden={section !== 'rules'} sx={{ py: 2, mb: 1, '& .MuiTable-root': { minWidth: 720 } }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 1, mb: 1.5 }}><Box sx={{ flex: '1 1 240px', minWidth: 0 }}><Typography fontWeight={600}>通知规则</Typography><Typography variant="caption" color="text.secondary">普通送样、人员变动反馈和审批通知均可独立配置；人员变动业务明细仍保存在独立台账。</Typography></Box>{canManage && <Button size="small" variant="contained" startIcon={<AddIcon />} onClick={() => openRule()}>新增规则</Button>}</Box>
      <TableContainer>
<Table size="small">
<TableHead>
<TableRow>
<TableCell>规则</TableCell>
<TableCell>通知事件 / 匹配范围</TableCell>
<TableCell>目标渠道</TableCell>
<TableCell>人员范围</TableCell>
<TableCell>状态</TableCell>
<TableCell align="right">操作</TableCell>
</TableRow>
</TableHead>
<TableBody>{rules.map(rule => { const targets = (rule.targets || []).map(target => target.target_kind === 'personnel_change_feedback_approved' ? '人员变动审批通过' : target.target_kind === 'personnel_change_feedback_rejected' ? '人员通知反馈驳回' : target.target_kind === 'personnel_change_feedback' ? '人员变动反馈' : target.target_kind === 'sample_info_rejected' ? `样品登记驳回${target.sample_info_type_key ? ` · ${sampleTypes.find(type => type.type_key === target.sample_info_type_key)?.label || target.sample_info_type_key}` : '（全部类型）'}` : target.target_kind === 'rd_work_record_rejected' ? '研发送样驳回' : target.target_kind === 'rd_work_record_resubmitted' ? '退回后重新提交' : target.target_kind === 'rd_work_record' ? '研发送样' : target.sample_info_type_key ? (sampleTypes.find(type => type.type_key === target.sample_info_type_key)?.label || target.sample_info_type_key) : '样品信息登记（全部类型）'); const rejection = ['rd_record_rejected', 'sample_info_rejected'].includes(rule.event_type); const personnelRejection = targets.includes('人员通知反馈驳回') || targets.includes('人员变动审批通过'); const personnel = personnelRejection || targets.includes('人员变动反馈'); return <TableRow key={rule.id}>
<TableCell>{rule.name}{rule.is_default && <Chip size="small" label="默认" sx={{ ml: 0.75 }} />}</TableCell>
<TableCell>
<Typography variant="body2" fontWeight={rejection || personnelRejection ? 700 : 400}>{[targets.join('、'), rule.group_names?.length ? rule.group_names.join('、') : rule.group_name, rule.project_name, rule.detection_type ? `检测类型：${rule.detection_type}` : ''].filter(Boolean).join(' / ') || '全部送样'}</Typography>
</TableCell>
<TableCell>{rule.channel_name || '仅程序内通知'}{rule.channel_id && <Typography variant="caption" display="block" color="text.secondary">{rule.dingtalk_follow_recipients ? '@ 本规则站内接收人' : rule.dingtalk_mention_user_ids?.length ? '@ ' + rule.dingtalk_mention_user_ids.map(id => contacts.find(contact => contact.user_id === id)?.username || ('账号' + id)).join('、') : '不 @'}</Typography>}</TableCell>
<TableCell>{rejection ? `原送样人${rule.recipient_user_ids.length ? ` + 指定 ${rule.recipient_user_ids.length} 人` : ''}` : personnelRejection ? `发起人${rule.recipient_user_ids.length ? ` + 指定 ${rule.recipient_user_ids.length} 人` : ''}` : rule.recipient_user_ids.length ? `${rule.recipient_user_ids.length} 人` : personnel ? '有详情权限的管理员与组长' : '全部分析人员'}</TableCell>
<TableCell>
<Chip size="small" label={rule.is_active ? '启用' : '停用'} color={rule.is_active ? 'success' : 'default'} />
</TableCell>
<TableCell align="right">{canManage && <Button size="small" startIcon={<EditIcon />} onClick={() => openRule(rule)}>编辑</Button>}</TableCell>
</TableRow>})}{rules.length === 0 && <TableRow>
<TableCell colSpan={6} align="center">尚未配置规则。建议先建立普通送样规则、驳回通知规则或人员变动反馈规则。</TableCell>
</TableRow>}</TableBody>
</Table>
</TableContainer>
    </Box>
    <Box id="notification-events" hidden={section !== 'events'} sx={{ py: 1 }}>
      <Typography fontWeight={600}>通知事件</Typography>
      <Typography variant="caption" color="text.secondary">显示符合筛选条件的最近200条事件；可筛选待处理的问题事件查找历史失败。成功渠道与已生成的个人通知保留，补发原因记入审计日志。</Typography>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1} sx={{ my: 1.5 }}><FormControl size="small" sx={{ minWidth: 180 }}><InputLabel>事件状态</InputLabel><Select label="事件状态" value={eventStatus} onChange={event => setEventStatus(event.target.value)}><MenuItem value="">全部状态</MenuItem><MenuItem value="attention">需处理（失败 / 跳过）</MenuItem>{Object.entries(statusLabels).map(([key, label]) => <MenuItem key={key} value={key}>{label}</MenuItem>)}</Select></FormControl><TextField size="small" label="通知编号或关键词" value={eventSearchDraft} onChange={event => setEventSearchDraft(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') setEventKeyword(eventSearchDraft.trim()); }} /><Button variant="outlined" onClick={() => setEventKeyword(eventSearchDraft.trim())}>查询事件</Button></Stack>
      <TableContainer sx={{ display: { xs: 'none', sm: 'block' } }}><Table size="small" sx={{ minWidth: 680 }}><TableHead><TableRow><TableCell>时间 / 编号</TableCell><TableCell>事件</TableCell><TableCell>状态</TableCell><TableCell>处理说明</TableCell><TableCell>操作</TableCell></TableRow></TableHead><TableBody>
        {events.map(event => <TableRow key={event.id}><TableCell>{event.created_at}<Typography variant="caption" display="block">{event.business_no || `事件 ${event.id}`}</Typography></TableCell><TableCell>{eventLabels[event.event_type] || event.event_type}</TableCell><TableCell><Chip size="small" label={statusLabels[event.status] || event.status} color={event.status === 'sent' ? 'success' : event.status === 'failed' ? 'error' : ['retry', 'partial'].includes(event.status) ? 'warning' : 'default'} /></TableCell><TableCell sx={{ maxWidth: 360, overflowWrap: 'anywhere' }}>{event.reason || '-'}</TableCell><TableCell>{eventActions(event)}</TableCell></TableRow>)}
        {!events.length && <TableRow><TableCell colSpan={5} align="center">暂无通知事件</TableCell></TableRow>}
      </TableBody></Table></TableContainer>
      <Box sx={{ display: { xs: 'grid', sm: 'none' }, gap: 1 }}>
        {events.map(event => <Paper key={event.id} variant="outlined" sx={{ p: 1.5, minWidth: 0 }}>
          <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1 }}><Typography variant="body2" fontWeight={600}>{eventLabels[event.event_type] || event.event_type}</Typography><Chip size="small" variant="outlined" label={statusLabels[event.status] || event.status} color={event.status === 'failed' ? 'error' : event.status === 'sent' ? 'success' : 'default'} /></Box>
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', my: 1, overflowWrap: 'anywhere' }}>{event.created_at} · {event.business_no || `事件 ${event.id}`}</Typography>
          <Typography variant="body2" sx={{ overflowWrap: 'anywhere', mb: 1 }}>{event.reason || '-'}</Typography>
          {eventActions(event)}
        </Paper>)}
        {!events.length && <Typography variant="body2" color="text.secondary" sx={{ p: 2 }}>暂无通知事件</Typography>}
      </Box>
    </Box>
    <Box id="notification-deliveries" hidden={section !== 'deliveries'} sx={{ py: 1, '& .MuiTable-root': { minWidth: 640 } }}><Typography fontWeight={600} sx={{ mb: 1.5 }}>发送记录</Typography>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mb: 1.5 }}>
        <TextField size="small" label="事件编号" value={deliveryDraft.event_id} onChange={event => setDeliveryDraft(value => ({ ...value, event_id: event.target.value }))} inputProps={{ inputMode: 'numeric' }} sx={{ width: 160 }} />
        <TextField size="small" type="date" label="开始日期" value={deliveryDraft.start_date} onChange={event => setDeliveryDraft(value => ({ ...value, start_date: event.target.value }))} InputLabelProps={{ shrink: true }} />
        <TextField size="small" type="date" label="结束日期" value={deliveryDraft.end_date} onChange={event => setDeliveryDraft(value => ({ ...value, end_date: event.target.value }))} InputLabelProps={{ shrink: true }} />
        <Button size="small" variant="outlined" onClick={queryDeliveries}>查询发送记录</Button>
        <Button size="small" onClick={() => { const empty = { event_id: '', start_date: '', end_date: '' }; setDeliveryDraft(empty); setDeliveryFilters(empty); setDeliveryPage(0); }}>清除筛选</Button>
      </Box>
      <TableContainer><Table size="small"><TableHead><TableRow><TableCell>时间 / 事件编号</TableCell><TableCell>渠道</TableCell><TableCell>状态</TableCell><TableCell>尝试次数</TableCell><TableCell>结果</TableCell></TableRow></TableHead><TableBody>{deliveries.map(item => <TableRow key={item.id}><TableCell>{item.sent_at || item.created_at}<Typography variant="caption" display="block">事件 {item.event_id}</Typography></TableCell><TableCell>{item.channel_name}</TableCell><TableCell><Chip size="small" label={item.status === 'sent' ? '已发送' : item.status === 'retry' ? '重试中' : item.status === 'cancelled' ? '已失效' : '失败'} color={item.status === 'sent' ? 'success' : item.status === 'retry' ? 'warning' : 'error'} /></TableCell><TableCell>{item.attempts}</TableCell><TableCell sx={{ maxWidth: 420, whiteSpace: 'normal', wordBreak: 'break-word' }}>{item.last_error || item.response_summary || '-'}</TableCell></TableRow>)}{deliveries.length === 0 && <TableRow><TableCell colSpan={5} align="center">暂无发送记录；仅站内回执或被跳过的事件可能没有群投递记录，请结合事件处理说明查看。</TableCell></TableRow>}</TableBody></Table></TableContainer>
      <TablePagination component="div" count={deliveryTotal} page={deliveryPage} rowsPerPage={deliveryPageSize} onPageChange={(_, next) => setDeliveryPage(next)} onRowsPerPageChange={event => { setDeliveryPage(0); setDeliveryPageSize(Number(event.target.value)); }} rowsPerPageOptions={[20, 50, 100]} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { flexWrap: 'wrap' } }} />
    </Box>
    <Dialog open={Boolean(retryTarget)} onClose={() => { if (!retryBusy.current) setRetryTarget(null); }} fullWidth maxWidth="sm"><DialogTitle>确认补发通知</DialogTitle><DialogContent><Stack spacing={2} sx={{ pt: 1 }}><Alert severity="info">{retryTarget?.business_no || `事件 ${retryTarget?.id || ''}`} 将重新进入发送队列。已成功的渠道和个人通知不会重复发送。请先检查渠道和规则；补发原因将保留在审计日志。</Alert><TextField autoFocus required multiline minRows={2} label="补发原因" value={retryReason} onChange={event => setRetryReason(event.target.value)} disabled={retrying} fullWidth /></Stack></DialogContent><DialogActions><Button disabled={retrying} onClick={() => setRetryTarget(null)}>取消</Button><Button variant="contained" disabled={retrying || !retryReason.trim()} onClick={() => void confirmRetry()}>{retrying ? '处理中…' : '确认补发'}</Button></DialogActions></Dialog>
    <Dialog open={channelDialog} onClose={() => setChannelDialog(false)} fullWidth maxWidth="sm"><DialogTitle>{editingChannel ? '编辑钉钉通知渠道' : '新增钉钉通知渠道'}</DialogTitle><DialogContent><Stack spacing={2} sx={{ pt: 1 }}><TextField label="渠道名称" value={channelForm.name} onChange={e => setChannelForm(v => ({ ...v, name: e.target.value }))} fullWidth /><TextField label="钉钉 Webhook" placeholder="https://oapi.dingtalk.com/robot/send?access_token=..." value={channelForm.webhook_url} onChange={e => setChannelForm(v => ({ ...v, webhook_url: e.target.value }))} fullWidth helperText={editingChannel ? '留空保留原 Webhook；填写新地址才会替换。' : ''} /><TextField label="加签密钥（可选）" type="password" value={channelForm.secret} onChange={e => setChannelForm(v => ({ ...v, secret: e.target.value }))} fullWidth helperText={editingChannel ? '留空保留原密钥；填写新密钥才会替换。' : '未启用钉钉加签时留空。'} /><FormControlLabel control={<Switch checked={channelForm.is_active} onChange={e => setChannelForm(v => ({ ...v, is_active: e.target.checked }))} />} label="启用渠道" /></Stack></DialogContent><DialogActions><Button onClick={() => setChannelDialog(false)}>取消</Button><Button variant="contained" onClick={() => void saveChannel()}>保存</Button></DialogActions></Dialog>
    <Dialog open={ruleDialog} onClose={() => setRuleDialog(false)} fullWidth maxWidth="sm">
<DialogTitle>{editingRule ? '编辑通知规则' : '新增通知规则'}</DialogTitle>
<DialogContent>
<Stack spacing={2} sx={{ pt: 1 }}>
{message?.error && <Alert severity="error">{message.text}</Alert>}
<TextField label="规则名称" value={ruleForm.name} onChange={e => setRuleForm(v => ({ ...v, name: e.target.value }))} fullWidth />
<FormControl fullWidth>
<InputLabel>通知来源与类型</InputLabel>
<Select multiple label="通知来源与类型" value={ruleForm.targetKeys} onChange={e => { const values = e.target.value as string[]; const special = values.find(value => value === 'rd_work_record_rejected' || value.startsWith('sample_info_rejected') || value === 'rd_work_record_resubmitted' || value === 'personnel_change_feedback' || value === 'personnel_change_feedback_rejected' || value === 'personnel_change_feedback_approved'); const normalized = special ? [special] : values; setRuleForm(v => ({ ...v, targetKeys: normalized })); }} renderValue={values => values.length ? values.map(value => value === 'rd_work_record_rejected' ? '研发送样驳回' : value.startsWith('sample_info_rejected') ? `样品登记驳回 · ${value === 'sample_info_rejected' ? '全部类型' : sampleTypes.find(type => `sample_info_rejected:${type.type_key}` === value)?.label || value}` : value === 'rd_work_record_resubmitted' ? '退回后重新提交' : value === 'rd_work_record' ? '研发送样' : value === 'personnel_change_feedback' ? '人员变动反馈' : value === 'personnel_change_feedback_approved' ? '人员变动审批通过' : value === 'personnel_change_feedback_rejected' ? '人员通知反馈驳回' : value === 'sample_info:*' ? '样品信息登记（全部类型）' : sampleTypes.find(type => `sample_info:${type.type_key}` === value)?.label || value).join('、') : (ruleForm.is_default ? '全部来源' : '请选择')}>
<MenuItem value="rd_work_record_rejected">
<Checkbox checked={ruleForm.targetKeys.includes('rd_work_record_rejected')} />研发送样驳回</MenuItem>
<MenuItem value="sample_info_rejected">
<Checkbox checked={ruleForm.targetKeys.includes('sample_info_rejected')} />样品登记驳回（全部类型）</MenuItem>
{sampleTypes.map(type => <MenuItem key={`rejected:${type.type_key}`} value={`sample_info_rejected:${type.type_key}`} disabled={!type.is_active}><Checkbox checked={ruleForm.targetKeys.includes(`sample_info_rejected:${type.type_key}`)} />样品登记驳回 · {type.label}{type.is_active ? '' : '（已停用）'}</MenuItem>)}
<MenuItem value="rd_work_record_resubmitted">
<Checkbox checked={ruleForm.targetKeys.includes('rd_work_record_resubmitted')} />退回后重新提交（独立规则）</MenuItem>
<MenuItem value="rd_work_record">
<Checkbox checked={ruleForm.targetKeys.includes('rd_work_record')} />研发送样</MenuItem>
<MenuItem value="personnel_change_feedback">
<Checkbox checked={ruleForm.targetKeys.includes('personnel_change_feedback')} />人员变动反馈</MenuItem>
<MenuItem value="personnel_change_feedback_rejected">
<Checkbox checked={ruleForm.targetKeys.includes('personnel_change_feedback_rejected')} />人员通知反馈驳回</MenuItem>
<MenuItem value="personnel_change_feedback_approved">
<Checkbox checked={ruleForm.targetKeys.includes('personnel_change_feedback_approved')} />人员变动审批通过</MenuItem>
<MenuItem value="sample_info:*">
<Checkbox checked={ruleForm.targetKeys.includes('sample_info:*')} />样品信息登记（全部类型）</MenuItem>{sampleTypes.map(type => <MenuItem key={type.type_key} value={`sample_info:${type.type_key}`} disabled={!type.is_active}>
<Checkbox checked={ruleForm.targetKeys.includes(`sample_info:${type.type_key}`)} />{type.label}{type.is_active ? '' : '（已停用）'}</MenuItem>)}</Select>
</FormControl>
<FormControl fullWidth>
<InputLabel>所属实验室</InputLabel>
<Select multiple label="所属实验室" value={ruleForm.group_ids} onChange={e => setRuleForm(v => ({ ...v, group_ids: e.target.value as string[] }))} renderValue={values => values.length ? values.map(value => groups.find(group => String(group.id) === value)?.name || value).join('、') : '全部实验室'}>{groups.map(group => <MenuItem key={group.id} value={String(group.id)}>
<Checkbox checked={ruleForm.group_ids.includes(String(group.id))} />{group.name}</MenuItem>)}</Select>
</FormControl>
<TextField label="项目名称（可选）" value={ruleForm.project_name} onChange={e => setRuleForm(v => ({ ...v, project_name: e.target.value }))} fullWidth />
<Autocomplete freeSolo options={detectionTypes} inputValue={ruleForm.detection_type} onInputChange={(_event, value) => setRuleForm(form => ({ ...form, detection_type: value }))} renderInput={params => <TextField {...params} label="检测类型 / 仪器类型（可选）" helperText="按名称精确匹配，如液相、气相、理化；留空不限。" />} />
<FormControl fullWidth>
<InputLabel>钉钉通知渠道</InputLabel>
<Select label="钉钉通知渠道" value={ruleForm.channel_id} onChange={e => setRuleForm(v => ({ ...v, channel_id: e.target.value as string }))}>
<MenuItem value="">仅程序内通知</MenuItem>{channels.map(channel => <MenuItem key={channel.id} value={String(channel.id)}>{channel.name}</MenuItem>)}</Select>
</FormControl>
<FormControl fullWidth>
<InputLabel>程序内通知人员</InputLabel>
<Select multiple label="程序内通知人员" value={ruleForm.recipient_user_ids} onChange={e => setRuleForm(v => ({ ...v, recipient_user_ids: e.target.value as number[] }))} renderValue={values => values.length ? `${values.length} 人` : (ruleForm.targetKeys.some(key => key === 'rd_work_record_rejected' || key.startsWith('sample_info_rejected')) ? '原送样人（可额外指定人员）' : personnelRule ? '留空按人员变动详情权限匹配' : '留空按分析检测权限匹配')}>{recipientUsers.map(item => <MenuItem key={item.id} value={item.id}>
<Checkbox checked={ruleForm.recipient_user_ids.includes(item.id)} />{item.username}</MenuItem>)}</Select>
</FormControl>
<DingTalkMentionFields channelId={ruleForm.channel_id} mode={ruleForm.dingtalk_mention_mode} selected={ruleForm.dingtalk_mention_user_ids} contacts={contacts} rejection={ruleForm.targetKeys.some(key => key === 'rd_work_record_rejected' || key.startsWith('sample_info_rejected'))} onChange={(mode, ids) => setRuleForm(value => ({ ...value, dingtalk_mention_mode: mode, dingtalk_mention_user_ids: ids }))} />
<FormControlLabel control={<Switch checked={ruleForm.is_default} onChange={e => setRuleForm(v => ({ ...v, is_default: e.target.checked }))} />} label="作为当前通知事件未匹配时的默认规则" />
<FormControlLabel control={<Switch checked={ruleForm.is_active} onChange={e => setRuleForm(v => ({ ...v, is_active: e.target.checked }))} />} label="启用规则" />{ruleForm.targetKeys.some(key => key === 'rd_work_record_rejected' || key.startsWith('sample_info_rejected')) && <Alert severity="info">驳回后自动向原送样账号发送站内回执，包含驳回原因；可额外指定分析人员，钉钉群通知为可选。停用或无当前记录权限的账号不会收到详情。</Alert>}{personnelRule && <Alert severity="info">仅发送给仍有该条详情权限的管理员和组长；研发送样组长只接收自己发起的申请。通过或驳回结果始终包含原发起账号，不受指定人员限制；停用或撤销权限的账号不会收到详情。</Alert>}{ruleForm.targetKeys.includes('rd_work_record_resubmitted') && <Alert severity="info">退回后重新提交是独立通知事件，可单独配置钉钉群和程序内分析检测人员。</Alert>}</Stack>
</DialogContent>
<DialogActions>
<Button onClick={() => setRuleDialog(false)}>取消</Button>
<Button variant="contained" onClick={() => void saveRule()}>保存</Button>
</DialogActions>
</Dialog>
  </ManageWorkspace>;
};
export default NotificationPage;
