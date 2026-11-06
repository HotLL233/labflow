/* LabFlow offline layout references. All data and writes are synthetic and in memory. */
(() => {
  'use strict';
  const $ = (q, root = document) => root.querySelector(q);
  const $$ = (q, root = document) => [...root.querySelectorAll(q)];
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const ico = name => `<svg class="ico" aria-hidden="true"><use href="#ref-${name}"/></svg>`;
  const button = (label, attrs = '', style = '') => `<button type="button" class="button ${style}" ${attrs}>${label}</button>`;
  const tool = (name, title, attrs = '', cls = '') => `<button type="button" class="icon-button ${cls}" title="${esc(title)}" aria-label="${esc(title)}" ${attrs}>${ico(name)}</button>`;
  const scheme = document.body.dataset.layout;
  const groups = [
    {name:'主数据', icon:'storage', items:[['projects','研发项目','list'],['groups','实验室','folder'],['divisions','部门','people'],['methods','检测方法','science'],['auxiliary','辅助工作','list'],['sampleinfo','样品信息登记','science'],['master','主数据管理','storage']]},
    {name:'权限与人员', icon:'people', items:[['users','用户','people'],['roles','角色与权限','key'],['sessions','登录会话','list']]},
    {name:'数据治理', icon:'timeline', items:[['governance','业务数据管理','storage'],['audit','审计日志','timeline'],['backup','数据备份','cloud'],['notifications','通知中心','bell'],['trash','回收站','trash']]},
    {name:'系统配置', icon:'settings', items:[['layouts','页面布局','grid'],['announcements','系统公告','campaign'],['fields','录入表单','table'],['personnel','人员反馈字段','people'],['exports','导出模板','list'],['help','教程与帮助','help'],['stats','统计管理','timeline'],['maintenance','日志与维护','settings']]}
  ];
  const titles = Object.fromEntries(groups.flatMap(g => g.items).map(x => [x[0], x[1]]));
  titles.overview = '管理工作区';
  const demoViews = new Set(['overview','methods','master','auxiliary','roles','notifications','fields']);
  const firstViews = ['methods','roles','notifications','fields'];
  const methods = Array.from({length:32}, (_, i) => {
    const type = i<4?'液相':i<6?'气相':i<8?'理化':['液相','气相','理化','ICP','热分析'][i % 5];
    const names = ['D002-260624-DAD','D006-260921-VWD','HYLY-230106-1-低温8℃-VWD','环糊精-250310-RID','J001-260706','甲乙醇-410实验室','氟离子（直接检测）','水分-特殊处理'];
    const generated = type==='液相'?`${['FB01','Q008','D006','T001'][i%4]}-26${600+i}-DAD`:type==='气相'?`顶空残留溶剂-26${600+i}`:type==='理化'?`${['酸价','总酸','氯离子','干燥失重'][i%4]}-${i+1}`:type==='ICP'?`ICP-OES-金属元素-${i+1}`:`DSC-热分析-${i+1}`;
    return {id:i+1, code:`M-${String(i+1).padStart(8,'0')}`, name:i < 8 ? names[i] : generated, type, instrument:i===0?'LC-20':type === '液相' ? `LC-${String(i%20+1).padStart(2,'0')}` : type === '气相' ? `GC-${String(i%5+1).padStart(2,'0')}` : type === '理化' ? '普通滴定' : type === 'ICP' ? 'ICP-OES-01' : 'DSC-01', coefficient:i === 0 ? 28 : i === 1 ? 0 : 8+(i%7)*4, multiplier:1, amount:15+(i%5)*10, enabled:i%9 !== 8, notes:i===0?'与当前方法身份关联，历史快照保留':''};
  });
  const areas = ['液相区域','气相区域','理化区域','公共区域'];
  const auxiliary = [
    ['流动相配制','液相区域','日常准备',6],['流动相瓶清洗','液相区域','器皿清洗',2],['进样针维护','气相区域','仪器维护',4],['水分仪标定','理化区域','仪器校准',8],
    ['天平校验','公共区域','仪器校准',3],['危废整理','公共区域','现场维护',5],['溶剂补充','液相区域','日常准备',0],['气路检漏','气相区域','仪器维护',5]
  ].map((r,i)=>({id:i+1,code:`A-${String(i+1).padStart(8,'0')}`,name:r[0],area:r[1],category:r[2],coefficient:r[3],enabled:true,order:i+1,notes:''}));
  const roles = [
    {id:1,name:'系统管理员',source:'内置角色',description:'系统配置与全局维护',count:'全部',system:true},
    {id:2,name:'分析检测组长',source:'分析组长模板',description:'分析业务及授权范围维护',count:42},
    {id:3,name:'分析检测员',source:'分析人员模板',description:'检测录入和本人工作量',count:26},
    {id:4,name:'研发送样组长',source:'研发组长模板',description:'关联实验室送样维护',count:29},
    {id:5,name:'研发送样员',source:'研发人员模板',description:'本实验室送样与记录',count:18}
  ];
  const events = Array.from({length:12}, (_,i)=>({id:i+1,no:`TZ-1106-${String(18-i).padStart(3,'0')}`,time:`2026-11-06 ${String(10-Math.floor(i/6)).padStart(2,'0')}:${String(50-i*3).padStart(2,'0')}`,name:['送样提交','研发送样驳回','送样提交','人员变动反馈'][i%4],source:['410-研究院 · D002','412-研究院 · FB01','417-研究院 · HYLY'][i%3],channel:i%4===1?'程序内通知':'分析检测群',status:i===0?'发送失败':i===2?'待发送':'已发送',reason:i===0?'网络连接超时':i===2?'等待队列处理':i===4?'已取样，相关个人通知已读':'投递成功'}));
  const makeFields = () => [
    {key:'project_name',label:'项目代号',type:'下拉选择',width:120,enabled:true,form:true,list:true,export:true,required:true},
    {key:'sample_name',label:'样品名称',type:'文本',width:160,enabled:true,form:true,list:true,export:true,required:true},
    {key:'batch_no',label:'批号',type:'文本',width:140,enabled:true,form:true,list:true,export:true,required:false},
    {key:'quantity',label:'数量',type:'数字',width:80,enabled:true,form:true,list:true,export:true,required:true},
    {key:'status',label:'取样状态',type:'系统状态',width:100,enabled:true,form:false,list:true,export:true,required:false},
    {key:'notes',label:'注意事项',type:'多行文本',width:180,enabled:true,form:true,list:true,export:true,required:false}
  ];
  const fieldConfigs = Object.fromEntries(['rd','work','sample-理化','sample-热稳定性','sample-微量含量'].map(k=>[k,makeFields()]));
  fieldConfigs['sample-热稳定性'][5].list=false;
  const template = {title:'研发送样通知',footer:'请及时取样',fields:['项目代号','样品名称','数量','注意事项'].map((label,i)=>({key:['project_name','sample_name','quantity','notes'][i],label,visible:true,bold:i===0,order:i+1}))};
  const initial = new URL(location.href).searchParams.get('view');
  const state = {
    view:demoViews.has(initial)?initial:'methods', group:0, collapsed:false, openGroups:new Set([0]), visited:['methods','auxiliary','notifications'],
    filters:{}, page:1, pageSize:20, masterKind:'检测方法', notifyTab:'events', fieldBusiness:'rd', sampleType:'理化',
    columns:{instrument:true,type:true,coefficient:true,multiplier:true,amount:true,code:false,status:true},
    settings:{density:40,font:14,outer:'#dce3ea',outerWidth:1,inner:'#e8edf2',innerWidth:1},
    templateDraft:null, templateDirty:false, dialogSave:null, dialogGuard:false, dialogInitial:''
  };
  let toastTimer;
  function groupFor(view) { const n=groups.findIndex(g=>g.items.some(i=>i[0]===view));return n<0?state.group:n; }
  function filters() { return state.filters[state.view] ||= {query:'',type:'',status:''}; }
  function toast(text) { clearTimeout(toastTimer);$('#toast').textContent=text;$('#toast').hidden=false;toastTimer=setTimeout(()=>$('#toast').hidden=true,3200); }
  function navItem(item) {
    return `<button class="nav-item ${state.view===item[0]?'active':''}" ${state.view===item[0]?'aria-current="page"':''} data-go="${item[0]}" title="${item[1]}">${ico(item[2])}<span class="nav-label">${item[1]}</span>${demoViews.has(item[0])?'<span class="demo-dot" aria-hidden="true"></span>':''}</button>`;
  }
  function sidebarContents(mobile=false) {
    const indices = scheme==='a' || mobile ? groups.map((_,i)=>i) : [state.group];
    return indices.map(i=>`<section class="nav-group"><button type="button" class="group-heading" data-toggle-group="${i}" aria-expanded="${mobile||scheme!=='a'||state.openGroups.has(i)}"><span>${groups[i].name}</span>${ico('down')}</button><div ${!mobile&&scheme==='a'&&!state.openGroups.has(i)?'hidden':''}>${groups[i].items.map(navItem).join('')}</div></section>`).join('');
  }
  function renderShell() {
    state.group=groupFor(state.view);state.openGroups.add(state.group);
    $('#application').innerHTML=`<div class="app-shell">
      <aside class="module-rail" aria-label="管理模块"><div class="rail-brand"><img src="assets/labflow-192.png" alt="知微"></div><div class="rail-items">${groups.map((g,i)=>`<button type="button" class="rail-item ${i===state.group?'active':''}" data-group="${i}" aria-label="${g.name}" ${i===state.group?'aria-current="true"':''}>${ico(g.icon)}<span>${g.name==='权限与人员'?'人员权限':g.name}</span></button>`).join('')}</div></aside>
      <aside class="sidebar" aria-label="管理功能导航"><div class="side-brand">${scheme==='a'?'<img src="assets/labflow-192.png" alt="知微">':''}<div><div class="brand-title">${scheme==='c'?groups[state.group].name:'样品管理系统'}</div><div class="side-caption">${scheme==='c'?'管理工作区':'知微 · 管理后台'}</div></div></div><div class="side-top"><span class="small muted">${scheme==='a'?'全部管理功能':'当前模块'}</span>${tool(state.collapsed?'right':'left',state.collapsed?'展开导航':'收起导航','data-collapse','flat small-button')}</div><nav class="side-scroll">${navItem(['overview','管理工作区','home'])}${sidebarContents()}</nav><div class="side-foot">实心小点标记本稿展开的页面<br>其余入口保留原信息架构</div></aside>
      <header class="topbar"><div class="row grow">${tool('menu','打开管理导航','data-mobile-menu','mobile-menu-button flat')}<div class="top-brand"><img src="assets/labflow-192.png" alt="知微"><span>样品管理系统</span></div><div class="breadcrumbs"><span>管理工作区</span>${ico('right')}<span>${groups[state.group].name}</span>${ico('right')}<span class="current-context">${titles[state.view]}</span></div></div><nav class="top-groups" aria-label="管理模块">${groups.map((g,i)=>`<button type="button" class="top-group ${i===state.group?'active':''}" data-group="${i}">${g.name}</button>`).join('')}</nav><div class="top-tools">${button(ico('home')+'主页','data-info="主页"','quiet top-global')}${button(ico('help')+'帮助','data-info="帮助与反馈"','quiet top-global')}${tool('campaign','系统公告','data-info="系统公告"','flat')}<button type="button" class="icon-button flat bell-button" title="我的通知" aria-label="我的通知，3条未读" data-info="我的通知">${ico('bell')}<span class="badge">3</span></button><span class="user-name">管理员</span></div></header>
      <nav class="work-tabs" aria-label="工作页面">${state.visited.map(view=>`<span class="work-tab ${view===state.view?'active':''}"><button type="button" data-go="${view}" ${view===state.view?'aria-current="page"':''}>${titles[view]}</button>${state.visited.length>1?`<button type="button" class="close-tab" data-close-tab="${view}" title="关闭${titles[view]}" aria-label="关闭${titles[view]}">${ico('close')}</button>`:''}</span>`).join('')}</nav>
      <main class="workspace" id="workspace"><div class="page-head"><div class="page-title-line"><h1>${titles[state.view]}</h1><span class="page-desc">${({methods:'方法、仪器与计量参数',master:'预览与导入导出',auxiliary:'独立事项与区域',roles:'角色与数据范围',notifications:'事件、投递与配置',fields:'字段范围与列表显示',overview:'23个现有管理入口'})[state.view]}</span></div><div class="actions">${state.view==='overview'?'':button(ico('back')+'<span>返回</span>','data-go="overview" title="返回管理工作区" aria-label="返回管理工作区"','back-button')}${pageActions()}${['fields','overview'].includes(state.view)?'':tool('more','更多管理操作','data-more','more-button')}</div></div><section class="screen" aria-label="${titles[state.view]}示例">${pageContent()}</section></main>
    </div>`;
    document.body.classList.toggle('slim-sidebar',state.collapsed);
    bindInputs();
  }
  function go(view) {
    if(!demoViews.has(view)){info(titles[view]||view);return;}
    if($('#panel').open){closePanel();if($('#panel').open)return;}
    state.view=view;state.page=1;state.group=groupFor(view);state.openGroups.add(state.group);
    if(!state.visited.includes(view))state.visited.push(view);
    if($('#mobile-navigation').open)$('#mobile-navigation').close();
    renderShell();
  }
  function pageActions() {
    if(state.view==='methods')return button(ico('add')+'新增方法','data-edit="method:new"','primary');
    if(state.view==='auxiliary')return button('区域管理','data-areas','secondary-page-action')+button(ico('add')+'新增事项','data-edit="aux:new"','primary');
    if(state.view==='roles')return button(ico('add')+'新增角色','data-info="新增角色"','primary');
    if(state.view==='fields')return button(ico('save')+'保存配置','data-save-fields','primary');
    if(state.view==='notifications')return button('处理待发送','data-process','primary');
    if(state.view==='master')return button(ico('upload')+'导入预检','data-import','primary')+button(ico('download')+'导出示例','data-export','secondary-page-action');
    return '';
  }
  function options(values,selected='',empty='全部') {return `<option value="">${empty}</option>${values.map(v=>`<option value="${esc(v)}" ${v===selected?'selected':''}>${esc(v)}</option>`).join('')}`;}
  function filterBand(extra='') {
    const f=filters();const typeValues=state.view==='auxiliary'?areas:['液相','气相','理化','ICP','热分析'];
    const placeholder=state.view==='auxiliary'?'搜索事项、区域或类别':state.view==='notifications'?'搜索事件、编号或来源':state.view==='master'?`搜索${state.masterKind}`:'搜索名称、仪器或方法ID';
    const statusField=state.view==='master'?'':`<label class="field desktop-filter"><span>状态</span><select id="status-filter" aria-label="状态筛选">${options(state.view==='notifications'?['发送失败','待发送','已发送']:['启用','停用','已删除'],f.status,'全部状态')}</select></label>`;
    return `<div class="filter-band"><label class="field search-field"><span>关键词</span><span class="search-control">${ico('search')}<input id="query" type="search" aria-label="搜索全部示例数据" placeholder="${placeholder}" value="${esc(f.query)}"></span></label>
      ${extra||`<label class="field desktop-filter"><span>${state.view==='auxiliary'?'区域':'检测类型'}</span><select id="type-filter" aria-label="${state.view==='auxiliary'?'区域筛选':'检测类型筛选'}">${options(typeValues,f.type,state.view==='auxiliary'?'全部区域':'全部类型')}</select></label>`}
      ${statusField}<div class="filter-actions">${button('重置','data-reset','quiet')}</div>${tool('filter','筛选条件','data-mobile-filter','mobile-filter-button')}</div>`;
  }
  function status(label) {const tone=['启用','已发送'].includes(label)?'good':['发送失败','已删除'].includes(label)?'bad':['待发送','部分失败'].includes(label)?'warn':'';return `<span class="status ${tone}">${esc(label)}</span>`;}
  function toolbar(count,all,extra='') {
    const label=state.view==='master'?esc(state.masterKind):({methods:'方法实例',auxiliary:'辅助事项',roles:'角色列表',notifications:state.notifyTab==='deliveries'?'投递记录':'事件记录'})[state.view]||'当前结果';
    const showColumns=state.view==='methods'||(state.view==='master'&&state.masterKind==='检测方法');
    return `<div class="table-toolbar"><div class="table-meta"><strong>${label}</strong><span class="num">${count} / ${all} 条</span><span class="meta-details">示例数据</span>${extra}</div><div class="row">${tool('refresh','刷新示例','data-refresh','flat')}${showColumns?tool('table','列显示设置','data-columns','flat'):''}${tool('tune','显示设置','data-display','flat')}</div></div>`;
  }
  function pagination(total) {
    const max=Math.max(1,Math.ceil(total/state.pageSize));state.page=Math.min(state.page,max);
    return `<div class="pagination"><span class="num">${total?(state.page-1)*state.pageSize+1:0}–${Math.min(state.page*state.pageSize,total)} / ${total}</span><div class="row"><label>每页 <select id="page-size" aria-label="每页记录条数">${[10,20,50].map(v=>`<option ${v===state.pageSize?'selected':''}>${v}</option>`).join('')}</select></label><span class="num">${state.page} / ${max}</span>${tool('left','上一页',`data-page="-1" ${state.page===1?'disabled':''}`,'small-button')}${tool('right','下一页',`data-page="1" ${state.page===max?'disabled':''}`,'small-button')}</div></div>`;
  }
  function table(headers,rows,classes='desktop-table',minWidth=760) {
    return `<div class="table-host ${classes}" data-table><table style="min-width:${minWidth}px"><thead><tr>${headers.map(h=>`<th class="${h.numeric?'numeric ':''}${h.action?'actions-cell':''}">${h.label||h}</th>`).join('')}</tr></thead><tbody>${rows||`<tr><td colspan="${headers.length}" class="empty-state">暂无符合条件的数据</td></tr>`}</tbody></table></div>`;
  }
  function rowEditAction(kind,item) {
    return item.deleted?tool('more',`恢复操作：${item.name}`,`data-row-menu="${kind}:${item.id}"`):tool('edit',`编辑${item.name}`,`data-edit="${kind}:${item.id}"`);
  }
  function methodRows(readOnly=false) {
    const f=filters();
    const matches=methods.filter(m=>(!m.deleted||f.status==='已删除')&&(!f.query||`${m.name} ${m.instrument} ${m.code}`.toLowerCase().includes(f.query.trim().toLowerCase()))&&(!f.type||m.type===f.type)&&(!f.status||(f.status==='已删除'?m.deleted:(m.enabled?'启用':'停用')===f.status)));
    const max=Math.max(1,Math.ceil(matches.length/state.pageSize));state.page=Math.min(state.page,max);
    const shown=matches.slice((state.page-1)*state.pageSize,state.page*state.pageSize);
    const cols=[['instrument','仪器'],['type','类型'],['coefficient','系数'],['multiplier','倍率'],['amount','单价'],['code','方法ID'],['status','状态']].filter(([key])=>state.columns[key]);
    const headers=[{label:'方法名称'},...cols.map(([key,label])=>({label,numeric:['coefficient','amount','multiplier'].includes(key)})),{label:readOnly?'':'操作',action:true}];
    const actions=m=>readOnly?'<span class="small muted">只读</span>':`${m.deleted?'':rowEditAction('method',m)}${tool('more',`更多操作：${m.name}`,`data-row-menu="method:${m.id}"`)}`;
    const value=(m,key)=>key==='status'?status(m.deleted?'已删除':m.enabled?'启用':'停用'):key==='amount'?Number(m.amount).toFixed(2):key==='multiplier'?Number(m.multiplier).toFixed(1):esc(m[key]);
    return `${toolbar(matches.length,methods.filter(m=>!m.deleted).length,readOnly?'<span class="label-tag">只读预览</span>':'')}${table(headers,shown.map(m=>`<tr data-record="${m.id}"><td class="method-name"><button type="button" title="${esc(m.name)}" data-detail="method:${m.id}">${esc(m.name)}</button></td>${cols.map(([key])=>`<td class="${['coefficient','amount','multiplier'].includes(key)?'numeric':key==='code'?'method-id':'nowrap'}">${value(m,key)}</td>`).join('')}<td class="actions-cell"><div class="cell-actions">${actions(m)}</div></td></tr>`).join(''))}
      <div class="mobile-records">${shown.map(m=>`<article class="mobile-record" data-record="${m.id}"><div class="record-layout"><div><button type="button" class="record-name" data-detail="method:${m.id}">${esc(m.name)}</button><div class="record-meta"><span>${esc(m.type)} · ${esc(m.instrument)}</span>${status(m.deleted?'已删除':m.enabled?'启用':'停用')}</div><div class="record-meta num"><span>系数 ${m.coefficient}</span><span>倍率 ${Number(m.multiplier).toFixed(1)}</span><span>单价 ${Number(m.amount).toFixed(2)}</span></div></div><div class="record-side">${readOnly?'<span class="small muted">只读</span>':rowEditAction('method',m)}</div></div></article>`).join('')||'<div class="empty-state">暂无符合条件的数据</div>'}</div>${pagination(matches.length)}`;
  }
  function auxiliaryContent() {
    const f=filters();const rows=auxiliary.filter(a=>(!a.deleted||f.status==='已删除')&&(!f.query||`${a.name} ${a.area} ${a.category}`.includes(f.query.trim()))&&(!f.type||a.area===f.type)&&(!f.status||(f.status==='已删除'?a.deleted:(a.enabled?'启用':'停用')===f.status)));
    const actions=a=>`${a.deleted?'':rowEditAction('aux',a)}${tool('more',`更多操作：${a.name}`,`data-row-menu="aux:${a.id}"`)}`;
    return `${filterBand()}${toolbar(rows.length,auxiliary.length)}${table(['辅助事项','区域','类别',{label:'系数',numeric:true},'状态',{label:'操作',action:true}],rows.map(a=>`<tr data-record="${a.id}"><td class="method-name"><button type="button" data-detail="aux:${a.id}">${esc(a.name)}</button></td><td class="nowrap">${esc(a.area)}</td><td class="nowrap">${esc(a.category)}</td><td class="numeric">${a.coefficient}</td><td>${status(a.deleted?'已删除':a.enabled?'启用':'停用')}</td><td class="actions-cell"><div class="cell-actions">${actions(a)}</div></td></tr>`).join(''),'desktop-table',660)}
      <div class="mobile-records">${rows.map(a=>`<article class="mobile-record" data-record="${a.id}"><div class="record-layout"><div><button type="button" class="record-name" data-detail="aux:${a.id}">${esc(a.name)}</button><div class="record-meta"><span>${esc(a.area)} · ${esc(a.category)}</span></div><div class="record-meta"><span>系数 ${a.coefficient}</span>${status(a.deleted?'已删除':a.enabled?'启用':'停用')}</div></div><div class="record-side">${rowEditAction('aux',a)}</div></div></article>`).join('')||'<div class="empty-state">暂无符合条件的事项</div>'}</div><p class="hint-line">每项独立配置；辅助工作不参与实验室归属或金额计算，历史快照保持不变</p>`;
  }
  function masterContent() {
    const kinds=['部门','实验室','检测类型','仪器','检测方法','研发项目','项目关联','辅助区域','辅助工作'];
    const extra=`<label class="field desktop-filter"><span>数据对象</span><select id="master-kind" aria-label="主数据对象">${kinds.map(k=>`<option ${state.masterKind===k?'selected':''}>${k}</option>`).join('')}</select></label>`;
    if(state.masterKind==='检测方法')return `${filterBand(extra)}${methodRows(true)}`;
    const data={
      部门:[['研究院','启用'],['质量部','启用'],['分析中心','启用']],
      实验室:[['410-研究院','研究院'],['412-研究院','研究院'],['415-研究院','研究院'],['417-研究院','研究院'],['805QA','质量部']],
      检测类型:[['液相','启用'],['气相','启用'],['理化','启用'],['ICP','启用'],['热分析','启用']],
      仪器:[['LC-20','液相色谱仪'],['GC-01','气相色谱仪'],['DSC-01','热分析仪器']],
      研发项目:[['D002','进行中'],['D006','进行中'],['HYLY','进行中'],['J001','进行中']],
      项目关联:[['D002','410-研究院'],['D006','412-研究院'],['HYLY','417-研究院']],
      辅助区域:areas.map(a=>[a,'启用']),
      辅助工作:auxiliary.map(a=>[a.name,a.area,a.category,String(a.coefficient)])
    };
    const rows=data[state.masterKind].filter(r=>r.join(' ').includes(filters().query.trim()));
    return `${filterBand(extra)}${toolbar(rows.length,data[state.masterKind].length,'<span class="label-tag">只读预览</span>')}${table(state.masterKind==='辅助工作'?['事项名称','区域','类别','系数']:[state.masterKind,'关联 / 状态'],rows.map(r=>`<tr>${r.map(v=>`<td>${esc(v)}</td>`).join('')}</tr>`).join(''),'',state.masterKind==='辅助工作'?560:400)}<p class="hint-line">示例快照 2026-11-06 10:36 · 主数据在线预览不直接编辑数据</p>`;
  }
  function rolesContent() {
    const actions=r=>`${tool('key',`查看${r.name}权限`,`data-role="${r.id}"`)}${!r.system?tool('edit',`编辑${r.name}`,`data-info="编辑角色"`):''}`;
    return `${toolbar(roles.length,roles.length)}${table(['角色名称','来源','说明',{label:'权限数',numeric:true},{label:'操作',action:true}],roles.map(r=>`<tr><td>${r.name}</td><td>${r.source}</td><td>${r.description}</td><td class="numeric">${r.count}</td><td class="actions-cell"><div class="cell-actions">${actions(r)}</div></td></tr>`).join(''),'desktop-table',660)}<div class="mobile-records">${roles.map(r=>`<article class="mobile-record"><div class="record-layout"><div><strong class="record-name">${r.name}</strong><div class="record-meta"><span>${r.source}</span><span>${r.count==='全部'?'全部权限':r.count+'项权限'}</span></div><div class="record-meta">${r.description}</div></div><div class="record-side">${tool('key',`查看${r.name}权限`,`data-role="${r.id}"`)}</div></div></article>`).join('')}</div><p class="hint-line">导航可见、数据范围与维护权限继续分别校验；内置角色保护保留</p>`;
  }
  function notificationContent() {
    const tabs=[['events','通知事件'],['deliveries','发送记录'],['config','渠道与规则'],['templates','内容模板'],['contacts','钉钉绑定']];
    let html=`<nav class="tabs" aria-label="通知中心页面">${tabs.map(t=>`<button type="button" class="tab ${state.notifyTab===t[0]?'active':''}" data-notify-tab="${t[0]}" aria-pressed="${state.notifyTab===t[0]}">${t[1]}</button>`).join('')}</nav>`;
    if(['events','deliveries'].includes(state.notifyTab)){
      const f=filters();const rows=events.filter(e=>(!f.query||`${e.name} ${e.no} ${e.source}`.includes(f.query.trim()))&&(!f.status||e.status===f.status));
      html+=`<div class="notification-summary">${[['待发送',events.filter(e=>e.status==='待发送').length,''],['发送失败',events.filter(e=>e.status==='发送失败').length,'bad'],['已发送',events.filter(e=>e.status==='已发送').length,'good']].map(s=>`<div class="summary-item"><span>${s[0]}</span><strong class="${s[2]}">${s[1]}</strong></div>`).join('')}</div>${filterBand('<label class="field desktop-filter"><span>记录范围</span><select disabled aria-label="示例记录范围"><option>全部示例记录</option></select></label>')}${toolbar(rows.length,events.length)}`;
      const actions=e=>`${button('详情',`data-event="${e.id}"`,'quiet')}${e.status==='发送失败'?button('补发',`data-retry="${e.id}"`):''}`;
      html+=table(state.notifyTab==='events'?['时间 / 编号','事件 / 来源','状态','处理说明',{label:'操作',action:true}]:['时间','渠道','状态','处理说明',{label:'操作',action:true}],rows.map(e=>`<tr><td class="nowrap small num">${e.time}<span class="scope-text">${e.no}</span></td><td>${state.notifyTab==='events'?e.name:e.channel}<span class="scope-text">${e.source}</span></td><td>${status(e.status)}</td><td class="small">${e.reason}</td><td class="actions-cell"><div class="cell-actions">${actions(e)}</div></td></tr>`).join(''),'desktop-table',800);
      html+=`<div class="mobile-records">${rows.map(e=>`<article class="mobile-record"><div class="record-layout"><div class="between" style="grid-column:1/-1"><strong class="record-name">${e.name}</strong>${status(e.status)}</div><div class="record-reason">${e.source} · ${e.time.slice(5)}</div><div class="record-reason">${e.reason}</div><div class="record-bottom"><span class="small muted">${e.no}</span><div class="row">${actions(e)}</div></div></div></article>`).join('')||'<div class="empty-state">暂无符合条件的事件</div>'}</div><p class="hint-line">正式通知事件继续保持当前匹配范围及权限；补发只处理失败渠道，必须填写原因</p>`;
    }else if(state.notifyTab==='config'){
      html+=`<div class="config-rows"><section class="config-row"><div class="grow"><h2>钉钉通知渠道</h2><p class="small muted">机器人地址与密钥不回显</p></div>${button('新增渠道','data-info="新增通知渠道"')}</section>${table(['渠道','状态','操作'],['分析检测群','管理员通知群'].map(n=>`<tr><td>${n}</td><td>${status('启用')}</td><td>${button('编辑','data-info="编辑通知渠道"','quiet')}</td></tr>`).join(''),'',440)}<section class="config-row"><div class="grow"><h2>通知规则</h2><p class="small muted">送样、驳回与人员变动分别设置</p></div>${button('新增规则','data-info="新增通知规则"')}</section>${table(['规则','渠道','接收范围','操作'],[['研发常规送样','分析检测群','本规则分析人员'],['人员变动反馈','管理员通知群','有详情权限的管理员与组长']].map(r=>`<tr>${r.map(v=>`<td>${v}</td>`).join('')}<td>${button('编辑','data-info="编辑通知规则"','quiet')}</td></tr>`).join(''),'',680)}</div>`;
    }else if(state.notifyTab==='templates'){
      html+=`<div class="config-rows"><section class="config-row"><div class="grow"><h2>研发送样通知</h2><p class="small muted">项目、数量、注意事项等字段${state.templateDirty?' · 有未保存草稿':''}</p></div>${button('字段配置与预览','data-template')}</section><div class="inline-note">候选字段继续来自实际业务表单配置；草稿、显示顺序、隐藏字段恢复和跟随源名称均保留</div></div>`;
    }else{
      html+=`<div class="config-rows"><section class="config-row"><div class="grow"><h2>钉钉身份绑定</h2><p class="small muted">仅有通知管理权限的账号维护</p></div>${button('绑定 / 修改','data-info="钉钉身份绑定"')}</section><p class="small secondary">沿用现有稳定账号关联与手机号绑定，不根据姓名猜测成员身份</p></div>`;
    }
    return html;
  }
  function fieldKey(){return state.fieldBusiness==='sample'?`sample-${state.sampleType}`:state.fieldBusiness;}
  function fieldContent() {
    const fields=fieldConfigs[fieldKey()];
    const rows=fields.map((f,i)=>`<tr><td><strong style="font-weight:500">${esc(f.label)}</strong><span class="scope-text">${esc(f.type)}${!f.enabled?' · 已停用':''}</span></td>${['form','list','export','required'].map(k=>`<td><input type="checkbox" aria-label="${esc(f.label+{form:'表单显示',list:'列表显示',export:'导出显示',required:'必填'}[k])}" data-field-index="${i}" data-field-prop="${k}" ${f[k]?'checked':''} ${!f.enabled||(k==='required'&&!f.form)?'disabled':''}></td>`).join('')}<td class="actions-cell">${tool('edit',`编辑字段${f.label}`,`data-edit-field="${i}"`)}</td></tr>`).join('');
    const preview=fields.filter(f=>f.enabled&&f.form).map(f=>`<div class="preview-line"><span>${esc(f.label)}${f.required?' *':''}</span><span>${({project_name:'D002',sample_name:'中间体样品',batch_no:'20261106-01',quantity:'2',notes:'避光保存'})[f.key]||'—'}</span></div>`).join('');
    return `<nav class="tabs" aria-label="表单业务">${[['rd','研发送样'],['sample','样品信息登记'],['work','分析检测']].map(t=>`<button type="button" class="tab ${state.fieldBusiness===t[0]?'active':''}" data-field-business="${t[0]}" aria-pressed="${state.fieldBusiness===t[0]}">${t[1]}</button>`).join('')}</nav>
      <div class="table-toolbar"><div class="row wrap">${state.fieldBusiness==='sample'?`<label class="field"><select id="sample-type" aria-label="样品登记检测类型">${['理化','热稳定性','微量含量'].map(t=>`<option ${t===state.sampleType?'selected':''}>${t}</option>`).join('')}</select></label>`:'<strong style="font-size:13px">字段与显示范围</strong>'}<span class="small muted">${fields.length}个字段</span></div>${button('新增字段','data-info="新增自定义字段"')}</div>
      <div class="field-table-and-preview">${table(['字段名称','表单','列表','导出','必填',{label:'操作',action:true}],rows,'',580)}<aside class="preview-pane"><h2>表单预览</h2><div id="field-preview">${preview}</div><p class="hint-line">每个检测类型独立保存范围，当前选择不会覆盖其他类型</p></aside></div><p class="hint-line">列宽、排序、启用状态与完整类型范围继续在字段编辑中维护</p>`;
  }
  function overviewContent() {
    return `<div class="directory"><p class="inline-note" style="margin-bottom:16px">六类页面已展开交互样例；其他管理入口保留位置，点击可查看本稿范围</p>${groups.map((g,i)=>`<section class="directory-row"><h2>${g.name}</h2><div class="directory-links">${g.items.map(item=>`<button type="button" class="directory-link" data-go="${item[0]}">${ico(item[2])}${item[1]}</button>`).join('')}</div></section>`).join('')}</div>`;
  }
  function pageContent() {
    if(state.view==='methods')return filterBand()+methodRows();
    if(state.view==='master')return masterContent();
    if(state.view==='auxiliary')return auxiliaryContent();
    if(state.view==='roles')return rolesContent();
    if(state.view==='notifications')return notificationContent();
    if(state.view==='fields')return fieldContent();
    return overviewContent();
  }
  function bindInputs() {
    $('#query')?.addEventListener('input',e=>{
      const value=e.target.value;filters().query=value;state.page=1;
      const screen=$('.screen');screen.innerHTML=pageContent();bindInputs();const input=$('#query');input.focus();
    });
    $('#type-filter')?.addEventListener('change',e=>{filters().type=e.target.value;state.page=1;refreshContent();});
    $('#status-filter')?.addEventListener('change',e=>{filters().status=e.target.value;state.page=1;refreshContent();});
    $('#master-kind')?.addEventListener('change',e=>{state.masterKind=e.target.value;Object.assign(filters(),{query:'',type:'',status:''});state.page=1;refreshContent();});
    $('#page-size')?.addEventListener('change',e=>{state.pageSize=Number(e.target.value);state.page=1;refreshContent();});
    $('#sample-type')?.addEventListener('change',e=>{state.sampleType=e.target.value;refreshContent();});
    $$('[data-field-index]').forEach(input=>input.addEventListener('change',()=>{
      const field=fieldConfigs[fieldKey()][Number(input.dataset.fieldIndex)];field[input.dataset.fieldProp]=input.checked;
      if(!field.form)field.required=false;
      refreshContent();
    }));
  }
  function refreshContent(){ $('.screen').innerHTML=pageContent();bindInputs(); }
  function formSnapshot(form){return JSON.stringify([...new FormData(form).entries()]);}
  function closePanel(force=false) {
    const form=$('#panel-form');
    if(!force&&state.dialogGuard&&form&&formSnapshot(form)!==state.dialogInitial&&!window.confirm('当前示例有未保存修改，确认放弃？'))return;
    $('#panel').close();state.dialogSave=null;state.dialogGuard=false;
  }
  function openPanel(title,body,save=null,label='保存示例',drawer=true,guard=false) {
    if($('#panel').open)$('#panel').close();
    state.dialogSave=save;state.dialogGuard=guard;
    $('#panel').className=drawer?'drawer':'';
    $('#panel').innerHTML=`<form id="panel-form" style="display:contents"><div class="panel-head"><h2 id="panel-title">${esc(title)}</h2>${tool('close','关闭窗口','data-close-panel')}</div><div class="panel-body">${body}<div id="panel-error" class="panel-error" role="alert" hidden></div></div><div class="panel-foot">${button(save?'取消':'关闭','data-close-panel')}${save?`<button type="submit" class="button primary" id="panel-submit">${label}</button>`:''}</div></form>`;
    $('#panel-form').addEventListener('submit',event=>{
      event.preventDefault();if(!event.currentTarget.reportValidity())return;
      if(state.dialogSave?.(new FormData(event.currentTarget))!==false)closePanel(true);
    });
    $('#panel').showModal();state.dialogInitial=formSnapshot($('#panel-form'));
  }
  function panelError(text){$('#panel-error').hidden=false;$('#panel-error').textContent=text;}
  function info(name){openPanel(name,`<p class="secondary">此入口在现有后台中保留。本稿仅展开检测方法、主数据管理、辅助工作、角色权限、通知中心和录入表单六类页面。</p><div class="inline-note" style="margin-top:16px">正式实施须继续使用现有权限、业务接口和配置；本稿没有连接任何业务服务</div>`,null,'',false);}
  function edit(kind,id) {
    const isNew=id==='new';const isAux=kind==='aux';
    const item=isNew?(isAux?{name:'',area:areas[0],category:'',coefficient:1,enabled:true,order:auxiliary.length+1,notes:''}:{name:'',instrument:'LC-20',type:'液相',coefficient:1,multiplier:1,amount:0,enabled:true,notes:''}):(isAux?auxiliary:methods).find(x=>x.id===Number(id));
    if(!item)return;
    const specific=isAux?`
      <label class="field full">区域<div class="row"><select name="area">${areas.map(a=>`<option ${a===item.area?'selected':''}>${esc(a)}</option>`).join('')}</select>${button('管理','data-areas')}</div></label>
      <label class="field full">类别<input name="category" list="category-options" value="${esc(item.category)}"><datalist id="category-options">${[...new Set(auxiliary.map(a=>a.category))].map(c=>`<option>${esc(c)}</option>`).join('')}</datalist></label>`:`
      <label class="field">绑定仪器<input name="instrument" required value="${esc(item.instrument)}"></label>
      <label class="field">检测类型<select name="type">${['液相','气相','理化','ICP','热分析'].map(t=>`<option ${t===item.type?'selected':''}>${t}</option>`).join('')}</select></label>`;
    const numeric=isAux?`<label class="field">排序<input name="order" type="number" step="1" value="${item.order}"></label>`:`
      <label class="field">金额倍率<input name="multiplier" type="number" min="0" step="any" required value="${item.multiplier}"></label>
      <label class="field">单价<input name="amount" type="number" min="0" step="0.01" required value="${item.amount}"></label>
      ${!isNew?`<label class="field">方法ID<input value="${esc(item.code)}" readonly></label>`:''}`;
    const body=`<div class="form-grid">
      <label class="field full">${isAux?'辅助事项':'方法名称'}<input name="name" required maxlength="180" value="${esc(item.name)}"></label>
      ${specific}<label class="field">工作量系数<input name="coefficient" type="number" min="0" step="any" required value="${item.coefficient}"><span class="help">0为合法配置</span></label>
      ${numeric}<label class="check full"><input type="checkbox" name="enabled" ${item.enabled?'checked':''}>启用</label>
      <label class="field full">备注<textarea name="notes" rows="3">${esc(item.notes)}</textarea></label>
      </div><div class="inline-note" style="margin-top:16px">${isAux?'每项独立配置；区域不复用检测类型，不分配实验室或金额':'更名与参数变更仍归于原方法身份'}。保存不重算历史工作量</div>`;
    openPanel(isNew?(isAux?'新增辅助事项':'新增检测方法'):(isAux?'编辑辅助事项':'编辑检测方法'),body,form=>{
      const name=form.get('name').trim();if(!name){panelError('名称不能为空');return false;}
      const update={name,coefficient:Number(form.get('coefficient')),enabled:form.has('enabled'),notes:form.get('notes')};
      if(isAux)Object.assign(update,{area:form.get('area'),category:form.get('category').trim(),order:Number(form.get('order'))});
      else Object.assign(update,{instrument:form.get('instrument').trim(),type:form.get('type'),multiplier:Number(form.get('multiplier')),amount:Number(form.get('amount'))});
      if(!Number.isFinite(update.coefficient)||update.coefficient<0){panelError('系数应为有效非负数');return false;}
      if(isNew){const list=isAux?auxiliary:methods;const next=Math.max(...list.map(x=>x.id))+1;list.push({...update,id:next,code:`${isAux?'A':'M'}-${String(next).padStart(8,'0')}`});}else Object.assign(item,update);
      refreshContent();toast('示例配置已保存，仅本页有效');return true;
    },'保存示例',true,true);
  }
  function detail(kind,id) {
    const isAux=kind==='aux';const item=(isAux?auxiliary:methods).find(x=>x.id===Number(id));if(!item)return;
    const values=isAux?[['事项',item.name],['区域',item.area],['类别',item.category],['系数',item.coefficient],['状态',item.enabled?'启用':'停用'],['备注',item.notes||'—']]:[['方法',item.name],['方法ID',item.code],['仪器',item.instrument],['类型',item.type],['系数',item.coefficient],['倍率',item.multiplier],['单价',Number(item.amount).toFixed(2)],['状态',item.enabled?'启用':'停用'],['备注',item.notes||'—']];
    openPanel(isAux?'辅助事项详情':'检测方法详情',values.map(r=>`<div class="preview-line"><span>${r[0]}</span><span style="overflow-wrap:anywhere">${esc(r[1])}</span></div>`).join('')+`<div style="margin-top:20px">${button(ico('edit')+'编辑',`data-edit="${kind}:${item.id}"`,'primary')}</div>`);
  }
  function areasPanel() {
    let dialog=$('#areas-panel');
    if(!dialog){dialog=document.createElement('dialog');dialog.id='areas-panel';dialog.setAttribute('aria-label','独立辅助区域');document.body.append(dialog);}
    dialog.innerHTML=`<form id="areas-form" style="display:contents"><div class="panel-head"><h2>独立辅助区域</h2>${tool('close','关闭区域管理','data-close-areas')}</div><div class="panel-body">${table(['区域名称','状态'],areas.map(a=>`<tr><td>${esc(a)}</td><td>${status('启用')}</td></tr>`).join(''),'',300)}<label class="field" style="margin-top:16px">新增区域<input name="area-name" required maxlength="120" placeholder="例如：制备区域"></label><div id="area-error" class="panel-error" role="alert" hidden></div><p class="hint-line">辅助区域独立管理；打开或关闭区域窗口均保留正在编辑的事项</p></div><div class="panel-foot">${button('关闭','data-close-areas')}<button type="submit" class="button primary">新增区域</button></div></form>`;
    $('#areas-form').addEventListener('submit',event=>{
      event.preventDefault();if(!event.currentTarget.reportValidity())return;
      const name=new FormData(event.currentTarget).get('area-name').trim();
      if(!name||areas.includes(name)){$('#area-error').hidden=false;$('#area-error').textContent=name?'该区域已存在':'请输入区域名称';return;}
      areas.push(name);
      const original=$('#panel select[name=area]');
      if(original){const option=document.createElement('option');option.value=name;option.textContent=name;original.append(option);}
      refreshContent();dialog.close();toast('已新增示例区域；原事项草稿保持不变');
    });
    dialog.showModal();
  }
  function displayPanel() {
    const s=state.settings;
    openPanel('显示设置',`<section class="settings-section"><h3>密度与字号</h3><label class="setting-line"><span>桌面行高</span><select name="density"><option value="36" ${s.density===36?'selected':''}>紧凑 36px</option><option value="40" ${s.density===40?'selected':''}>标准 40px</option><option value="48" ${s.density===48?'selected':''}>舒适 48px</option></select></label><label class="setting-line"><span>记录字号</span><select name="font">${[12,13,14,15,16].map(n=>`<option ${n===s.font?'selected':''}>${n}</option>`).join('')}</select></label></section><section class="settings-section"><h3>边框独立设置</h3><label class="setting-line"><span>外边框颜色</span><input type="color" name="outer" value="${s.outer}"></label><label class="setting-line"><span>外边框粗细</span><input name="outerWidth" type="number" min="0" max="3" step="1" value="${s.outerWidth}"></label><label class="setting-line"><span>内边框颜色</span><input name="inner" type="color" value="${s.inner}"></label><label class="setting-line"><span>内边框粗细</span><input name="innerWidth" type="number" min="0" max="3" step="1" value="${s.innerWidth}"></label></section><div class="inline-note">原型仅演示部分配置。正式系统仍保留既有字号范围、按钮前后状态配色、顺序、尺寸与列配置</div>`,form=>{
      const next={density:Number(form.get('density')),font:Number(form.get('font')),outer:form.get('outer'),inner:form.get('inner'),outerWidth:Number(form.get('outerWidth')),innerWidth:Number(form.get('innerWidth'))};
      Object.assign(state.settings,next);const style=document.documentElement.style;
      for(const [key,value] of [['--row-height',`${next.density}px`],['--data-font',`${next.font}px`],['--outer-color',next.outer],['--inner-color',next.inner],['--outer-width',`${next.outerWidth}px`],['--inner-width',`${next.innerWidth}px`]])style.setProperty(key,value);
      toast('显示设置已应用于本页示例');return true;
    },'应用示例',false);
  }
  function columnPanel(){
    const labels={instrument:'仪器',type:'类型',coefficient:'系数',multiplier:'倍率',amount:'单价',code:'方法ID',status:'状态'};
    openPanel('列表列显示',`<p class="small muted" style="margin-bottom:12px">方法名称固定保留，其他列按需显示</p>${Object.entries(labels).map(([k,label])=>`<label class="check"><input type="checkbox" name="${k}" ${state.columns[k]?'checked':''}>${label}</label>`).join('')}<p class="hint-line">辅助事项ID不在日常页面显示；正式Excel回导识别字段仍保留</p>`,form=>{for(const k of Object.keys(labels))state.columns[k]=form.has(k);refreshContent();return true;},'应用列显示',false);
  }
  function mobileFilter() {
    const f=filters();const values=state.view==='auxiliary'?areas:['液相','气相','理化','ICP','热分析'];
    const statusField=state.view==='master'?'':`<label class="field full">状态<select name="status">${options(state.view==='notifications'?['发送失败','待发送','已发送']:['启用','停用','已删除'],f.status,'全部状态')}</select></label>`;
    openPanel('筛选条件',`<div class="form-grid">${state.view==='master'?`<label class="field full">数据对象<select name="master-kind">${['部门','实验室','检测类型','仪器','检测方法','研发项目','项目关联','辅助区域','辅助工作'].map(k=>`<option ${k===state.masterKind?'selected':''}>${k}</option>`).join('')}</select></label>`:state.view==='notifications'?'':`<label class="field full">${state.view==='auxiliary'?'区域':'检测类型'}<select name="type">${options(values,f.type,'全部')}</select></label>`}${statusField}</div>`,form=>{f.type=form.get('type')||'';f.status=form.get('status')||'';if(form.has('master-kind'))state.masterKind=form.get('master-kind');state.page=1;refreshContent();return true;},'应用筛选',false);
  }
  function importPanel() {
    openPanel('主数据导入预检',`<div class="form-grid"><label class="field full">导入文件<input name="workbook" type="file" accept=".xlsx,.xls" required></label><label class="field full">导入方式<select name="mode"><option value="merge">合并更新</option><option value="skip">跳过已有</option><option value="replace">全量覆盖</option></select></label><div class="inline-note full">正式逻辑继续按稳定ID匹配，保留引用保护和预检确认。合并更新保留遗漏项，跳过已有不更新系数；不自动重算历史工作量</div><p class="small muted full">此处只演示文件选择与预检结果的排版，不解析或上传文件</p></div>`,form=>{
      const file=form.get('workbook');if(!file?.name){panelError('请先选择文件');return false;}
      const mode=form.get('mode');const label={merge:'合并更新',skip:'跳过已有',replace:'全量覆盖'}[mode];
      openPanel('预检结果布局示意',`<p class="small secondary" style="overflow-wrap:anywhere">${esc(file.name)} · ${label}</p><div class="inline-note" style="margin:16px 0">尚未读取所选文件。下方为预检表格结构，不代表真实预检通过</div>${table(['工作表 / 行','对象','处理','结果'],[['检测方法 / 示例','方法ID定位','待确认','等待后端预检'],['辅助工作 / 示例','事项ID定位','待确认','等待后端预检']].map(r=>`<tr>${r.map(v=>`<td>${v}</td>`).join('')}</tr>`).join(''),'',560)}<div style="margin-top:16px">${button('确认执行导入','disabled title="参考稿不调用真实导入接口"','primary')}</div>`,null,'',false);
      return false;
    },'查看预检布局',false);
  }
  function templatePanel() {
    state.templateDraft ||= structuredClone(template);
    const d=state.templateDraft;
    openPanel('研发送样 · 通知模板',`<div class="form-grid"><label class="field full">消息标题<input name="template-title" value="${esc(d.title)}" required></label><label class="field full">底部提示<input name="template-footer" value="${esc(d.footer)}"></label></div><div class="settings-section" style="margin-top:20px"><h3>通知字段</h3>${d.fields.map((f,i)=>`<div class="setting-line"><span>${f.label}</span><div class="check-row"><label class="check small"><input type="checkbox" name="visible-${i}" ${f.visible?'checked':''}>显示</label><label class="check small"><input type="checkbox" name="bold-${i}" ${f.bold?'checked':''}>加粗</label></div></div>`).join('')}</div><div class="inline-note">关闭后保留本页草稿；原字段来源、名称跟随、顺序及隐藏字段恢复功能在正式实现中保留</div>`,form=>{
      d.title=form.get('template-title').trim();if(!d.title){panelError('请输入标题');return false;}d.footer=form.get('template-footer');d.fields.forEach((f,i)=>{f.visible=form.has(`visible-${i}`);f.bold=form.has(`bold-${i}`);});Object.assign(template,structuredClone(d));state.templateDirty=false;refreshContent();toast('已保存示例通知模板');return true;
    });
    $('#panel-form').addEventListener('input',()=>{const form=new FormData($('#panel-form'));d.title=form.get('template-title');d.footer=form.get('template-footer');d.fields.forEach((f,i)=>{f.visible=form.has(`visible-${i}`);f.bold=form.has(`bold-${i}`);});state.templateDirty=true;});
  }
  function editField(index){
    const field=fieldConfigs[fieldKey()][index];
    openPanel(`编辑字段 · ${field.label}`,`<div class="form-grid"><label class="field full">显示名称<input name="label" required value="${esc(field.label)}"></label><label class="field">列宽（px）<input name="width" type="number" min="40" max="800" value="${field.width}"></label><label class="check"><input name="enabled" type="checkbox" ${field.enabled?'checked':''}>启用字段</label><div class="full check-row">${[['form','表单'],['list','列表'],['export','导出'],['required','必填']].map(([k,label])=>`<label class="check"><input name="${k}" type="checkbox" ${field[k]?'checked':''}>${label}</label>`).join('')}</div></div><div class="inline-note" style="margin-top:16px">仅更新当前业务及所选检测类型，其他类型保留各自配置</div>`,form=>{
      const name=form.get('label').trim();if(!name){panelError('显示名称不能为空');return false;}Object.assign(field,{label:name,width:Number(form.get('width')),enabled:form.has('enabled'),form:form.has('form'),list:form.has('list'),export:form.has('export'),required:form.has('required')&&form.has('form')&&form.has('enabled')});refreshContent();toast('字段示例已更新');return true;
    },'保存字段示例',true,true);
  }
  function exportExample(){
    const csv=value=>{let s=String(value??'');if(/^\s*[=+\-@]|^[\t\r\n]/.test(s))s="'"+s;return '"'+s.replaceAll('"','""')+'"';};
    const rows=state.view==='auxiliary'?[['事项名称','区域','类别','系数','事项ID'],...auxiliary.filter(a=>!a.deleted).map(a=>[a.name,a.area,a.category,a.coefficient,a.code])]:[['方法名称','仪器','类型','系数','单价','方法ID'],...methods.filter(m=>!m.deleted).map(m=>[m.name,m.instrument,m.type,m.coefficient,m.amount,m.code])];
    const url=URL.createObjectURL(new Blob(['\ufeff'+rows.map(r=>r.map(csv).join(',')).join('\r\n')],{type:'text/csv;charset=utf-8'}));const a=document.createElement('a');a.href=url;a.download=`布局示例-${state.view==='auxiliary'?'辅助事项':'检测方法'}.csv`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);toast('已导出虚构示例CSV；正式Excel模板保持原实现');
  }
  function mobileNavigation(){
    $('#mobile-navigation').innerHTML=`<div class="panel-head"><h2>管理导航</h2>${tool('close','关闭导航','data-close-nav')}</div><div class="mobile-nav-content">${navItem(['overview','管理工作区','home'])}${sidebarContents(true)}</div>`;
    $('#mobile-navigation').showModal();
  }
  document.addEventListener('click',event=>{
    const target=event.target.closest('button,a');if(!target)return;
    if(target.hasAttribute('data-scheme-link')){const u=new URL(target.href);u.searchParams.set('view',state.view);target.href=u.href;return;}
    if(target.dataset.go){go(target.dataset.go);return;}
    if(target.dataset.info){info(target.dataset.info);return;}
    if(target.dataset.group!==undefined){go(firstViews[Number(target.dataset.group)]);return;}
    if(target.hasAttribute('data-collapse')){state.collapsed=!state.collapsed;renderShell();return;}
    if(target.dataset.toggleGroup!==undefined){const i=Number(target.dataset.toggleGroup);if(state.openGroups.has(i))state.openGroups.delete(i);else state.openGroups.add(i);const expanded=target.getAttribute('aria-expanded')==='true';target.setAttribute('aria-expanded',String(!expanded));target.nextElementSibling.hidden=expanded;return;}
    if(target.hasAttribute('data-mobile-menu')){mobileNavigation();return;}
    if(target.hasAttribute('data-close-nav')){$('#mobile-navigation').close();return;}
    if(target.hasAttribute('data-close-areas')){$('#areas-panel').close();return;}
    if(target.hasAttribute('data-close-panel')){closePanel();return;}
    if(target.hasAttribute('data-display')){displayPanel();return;}
    if(target.hasAttribute('data-columns')){columnPanel();return;}
    if(target.hasAttribute('data-mobile-filter')){mobileFilter();return;}
    if(target.hasAttribute('data-reset')){Object.assign(filters(),{query:'',type:'',status:''});state.page=1;refreshContent();return;}
    if(target.hasAttribute('data-refresh')){toast('示例视图已刷新，未访问业务服务');return;}
    if(target.dataset.page){state.page+=Number(target.dataset.page);refreshContent();return;}
    if(target.dataset.edit){edit(...target.dataset.edit.split(':'));return;}
    if(target.dataset.detail){detail(...target.dataset.detail.split(':'));return;}
    if(target.hasAttribute('data-areas')){areasPanel();return;}
    if(target.dataset.closeTab){const view=target.dataset.closeTab;state.visited=state.visited.filter(x=>x!==view);if(state.view===view)go(state.visited.at(-1));else renderShell();return;}
    if(target.hasAttribute('data-more')){
      let actions='';
      if(state.view==='methods')actions=button('检测类型管理','data-info="检测类型管理"')+button('仪器管理','data-info="仪器管理"');
      if(state.view==='auxiliary')actions=button('区域管理','data-areas');
      if(['methods','auxiliary','master'].includes(state.view))actions+=button('主数据预览','data-go="master"')+button('主数据导入','data-import')+button('导出示例CSV','data-export');
      if(state.view==='roles')actions=button('从模板创建角色','data-info="从模板创建角色"')+button('用户管理','data-info="用户管理"');
      if(state.view==='notifications')actions=button('通知模板配置','data-template')+button('钉钉身份绑定','data-info="钉钉身份绑定"');
      openPanel('更多管理操作',`<div style="display:grid;gap:8px">${actions}</div><p class="hint-line">将低频维护操作从筛选行分离，原有入口均保留</p>`,null,'',false);return;
    }
    if(target.hasAttribute('data-export')){exportExample();return;}
    if(target.hasAttribute('data-import')){importPanel();return;}
    if(target.dataset.rowMenu){const [kind,id]=target.dataset.rowMenu.split(':');const item=(kind==='aux'?auxiliary:methods).find(x=>x.id===Number(id));openPanel('记录操作',`<p style="overflow-wrap:anywhere">${esc(item.name)}</p><div style="display:grid;gap:8px;margin-top:16px">${button('查看详情',`data-detail="${kind}:${id}"`)}${button(item.deleted?'恢复示例':'移入回收站',`data-recycle="${kind}:${id}"`,'danger')}</div>`,null,'',false);return;}
    if(target.dataset.recycle){const [kind,id]=target.dataset.recycle.split(':');const item=(kind==='aux'?auxiliary:methods).find(x=>x.id===Number(id));openPanel(item.deleted?'恢复示例':'移入回收站',`<p style="overflow-wrap:anywhere">${esc(item.name)}</p><label class="field" style="margin-top:16px">操作原因<textarea name="reason" required rows="3"></textarea></label><div class="inline-note" style="margin-top:16px">仅改变本页虚构数据；正式实现仍执行依赖预检、原权限与软删除保护</div>`,form=>{if(!form.get('reason').trim()){panelError('请填写操作原因');return false;}item.deleted=!item.deleted;refreshContent();toast('示例状态已更新，未修改任何真实记录');return true;},'确认示例操作',false);return;}
    if(target.dataset.notifyTab){state.notifyTab=target.dataset.notifyTab;filters().query='';filters().status='';refreshContent();return;}
    if(target.hasAttribute('data-template')){templatePanel();return;}
    if(target.hasAttribute('data-process')){toast('示例队列视图；不会向真实群发送消息');return;}
    if(target.dataset.event){const e=events.find(x=>x.id===Number(target.dataset.event));openPanel('通知发送详情',[['编号',e.no],['事件',e.name],['来源',e.source],['状态',e.status],['处理说明',e.reason]].map(r=>`<div class="preview-line"><span>${r[0]}</span><span>${esc(r[1])}</span></div>`).join('')+'<p class="hint-line">群投递状态、业务状态和个人通知已读状态分开保存</p>');return;}
    if(target.dataset.retry){const e=events.find(x=>x.id===Number(target.dataset.retry));openPanel('确认补发通知',`<p>${e.no}</p><label class="field" style="margin-top:16px">补发原因<textarea name="reason" required rows="3"></textarea></label><div class="inline-note" style="margin-top:16px">只处理失败渠道，不重发成功渠道；正式操作继续写入审计</div>`,form=>{if(!form.get('reason').trim()){panelError('请填写补发原因');return false;}e.status='待发送';e.reason='已加入示例补发队列';refreshContent();toast('示例状态已更新，未发送真实通知');return true;},'确认补发',false);return;}
    if(target.dataset.role){const r=roles.find(x=>x.id===Number(target.dataset.role));openPanel(r.name+' · 权限范围',`<div class="preview-line"><span>来源</span><span>${r.source}</span></div><div class="preview-line"><span>权限数量</span><span>${r.count}</span></div><div class="inline-note" style="margin-top:16px">保留原有角色模板、数据范围和内置角色保护。此稿仅比较布局，不改权限</div>`);return;}
    if(target.dataset.fieldBusiness){state.fieldBusiness=target.dataset.fieldBusiness;refreshContent();return;}
    if(target.dataset.editField!==undefined){editField(Number(target.dataset.editField));return;}
    if(target.hasAttribute('data-save-fields')){toast('当前类型的示例配置已保留在本页，未写入真实配置');return;}
  });
  $('#panel').addEventListener('cancel',event=>{event.preventDefault();closePanel();});
  renderShell();
  window.__labReference={state,methods,auxiliary,events,fieldConfigs,go};
})();
