from pathlib import Path
from docx import Document
from docx.shared import Inches, Pt, RGBColor
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.enum.table import WD_TABLE_ALIGNMENT, WD_CELL_VERTICAL_ALIGNMENT
from docx.enum.section import WD_SECTION
from docx.oxml import OxmlElement
from docx.oxml.ns import qn


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs"
OUT.mkdir(exist_ok=True)


def set_cell_shading(cell, fill):
    tc_pr = cell._tc.get_or_add_tcPr()
    shd = tc_pr.find(qn("w:shd"))
    if shd is None:
        shd = OxmlElement("w:shd")
        tc_pr.append(shd)
    shd.set(qn("w:fill"), fill)


def set_cell_text(cell, text, bold=False, color=None):
    cell.text = ""
    p = cell.paragraphs[0]
    p.paragraph_format.space_after = Pt(0)
    r = p.add_run(str(text))
    r.bold = bold
    r.font.size = Pt(9.5)
    if color:
        r.font.color.rgb = RGBColor(*color)
    cell.vertical_alignment = WD_CELL_VERTICAL_ALIGNMENT.CENTER


def add_table(doc, headers, rows, widths=None):
    table = doc.add_table(rows=1, cols=len(headers))
    table.style = "Table Grid"
    table.alignment = WD_TABLE_ALIGNMENT.CENTER
    for i, header in enumerate(headers):
        set_cell_text(table.rows[0].cells[i], header, bold=True, color=(255, 255, 255))
        set_cell_shading(table.rows[0].cells[i], "1F4E79")
    for row in rows:
        cells = table.add_row().cells
        for i, value in enumerate(row):
            set_cell_text(cells[i], value)
            if len(table.rows) % 2 == 0:
                set_cell_shading(cells[i], "EAF2F8")
    if widths:
        for row in table.rows:
            for i, width in enumerate(widths):
                row.cells[i].width = Inches(width)
    doc.add_paragraph()
    return table


def add_bullets(doc, items):
    for item in items:
        p = doc.add_paragraph(style="List Bullet")
        p.paragraph_format.space_after = Pt(2)
        p.add_run(item)


def add_steps(doc, steps):
    for index, (title, body) in enumerate(steps, 1):
        p = doc.add_paragraph(style="Heading 3")
        p.add_run(f"步骤 {index}：{title}")
        p = doc.add_paragraph(body)
        p.paragraph_format.left_indent = Inches(0.22)
        p.paragraph_format.space_after = Pt(5)


def add_heading(doc, text, level=1):
    doc.add_heading(text, level=level)


def setup_doc(doc, role):
    section = doc.sections[0]
    section.page_width = Inches(8.5)
    section.page_height = Inches(11)
    section.top_margin = Inches(0.72)
    section.bottom_margin = Inches(0.72)
    section.left_margin = Inches(0.85)
    section.right_margin = Inches(0.85)
    normal = doc.styles["Normal"]
    normal.font.name = "Microsoft YaHei"
    normal._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
    normal.font.size = Pt(10.5)
    for style_name, size in [("Heading 1", 16), ("Heading 2", 13), ("Heading 3", 11.5)]:
        style = doc.styles[style_name]
        style.font.name = "Microsoft YaHei"
        style._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
        style.font.size = Pt(size)
        style.font.bold = True
        style.font.color.rgb = RGBColor(31, 78, 121)
    for style_name in ["List Bullet", "List Number"]:
        style = doc.styles[style_name]
        style.font.name = "Microsoft YaHei"
        style._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
        style.font.size = Pt(10.5)

    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    p.paragraph_format.space_before = Pt(75)
    r = p.add_run("实验室流程管理系统")
    r.font.name = "Microsoft YaHei"
    r._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
    r.font.size = Pt(24)
    r.bold = True
    r.font.color.rgb = RGBColor(31, 78, 121)
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    r = p.add_run(f"{role}使用说明书")
    r.font.name = "Microsoft YaHei"
    r._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
    r.font.size = Pt(20)
    r.bold = True
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    r = p.add_run("V2.0.0  |  岗位操作与权限边界")
    r.font.size = Pt(12)
    r.font.color.rgb = RGBColor(89, 89, 89)
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    p.paragraph_format.space_before = Pt(26)
    p.add_run("文档说明：本文档按照 v2.0.0 实际页面和角色权限编写。页面截图、账号、项目和实验室名称以现场部署数据为准。\n").font.size = Pt(10)
    p.add_run("人员变动通知仅用于通知和审批，不替代管理员或分析检测组长对实际人员、项目和方法的手工维护。").bold = True
    doc.add_page_break()


def common_intro(doc, role, can_notify, can_approve, scope):
    if role == "研发送样组长":
        summary_permission = "查看、导出本人关联实验室；不能导入或维护"
    elif role == "研发分析组长（系统角色：分析检测组长）":
        summary_permission = "查看、导入、导出全部实验室"
    elif role == "管理员（系统角色：系统管理员）":
        summary_permission = "查看、导入、导出全部实验室并维护"
    else:
        summary_permission = "无本模块维护权限；需要时联系组长或管理员"
    notification_permission = f"发起：{'是' if can_notify else '否'}；审批：{'是' if can_approve else '否'}"
    if role.startswith("研发分析员"):
        notification_permission = "不参与人员变动通知"
    add_heading(doc, "一、权限范围与工作边界", 1)
    doc.add_paragraph(f"当前登录角色：{role}。本角色只能在系统分配的数据范围内操作；看不到的菜单或按钮属于权限控制，并非系统故障。")
    add_table(doc, ["功能模块", "本角色权限", "数据范围/边界"], [
        ("登录与个人信息", "登录、确认账号和角色、退出登录", "仅操作本人账号；发现角色不符应联系系统管理员"),
        ("研发送样", "按角色执行新建、查看或处理", scope),
        ("样品信息登记", "按角色执行登记、查询或处理", "只处理已授权的样品和项目；必填项未完成不能提交"),
        ("人员变动通知", notification_permission, "独立通知数据，不写入程序 PostgreSQL；审批通过后才更新项目及人员汇总表"),
        ("项目及人员汇总表", summary_permission, "研发送样组长仅限本人关联实验室；系统管理员、分析检测组长可查看全部"),
        ("系统管理", "按角色显示菜单", "不能操作的配置不应通过其他入口绕过"),
    ], [1.3, 2.0, 3.6])
    add_heading(doc, "二、登录与首次使用", 1)
    add_steps(doc, [
        ("打开登录页", "使用部署地址打开系统登录页。v2.0.0 登录页不显示注册入口；新账号由系统管理员创建。"),
        ("输入账号和密码", "输入管理员分配的用户名和密码，点击登录。密码不要通过截图或聊天工具传播。"),
        ("确认身份", "登录后查看右上角账号、角色和组织信息，确认与岗位一致。角色、实验室或项目不正确时先退出并联系管理员。"),
        ("检查首页菜单", "首页只显示当前角色可用入口。首次使用先确认页面可打开，再开始业务操作。"),
    ])
    add_heading(doc, "三、通用操作约定", 1)
    add_bullets(doc, [
        "带红色星号（*）的字段为必填项；保存前检查项目、实验室、方法和人员是否选对。",
        "列表页可使用查询条件、日期范围、分页和刷新；提交后以系统提示和列表状态为准。",
        "浏览器刷新或关闭页面前，先保存草稿或提交；未保存内容通常不会保留。",
        "涉及文件导入时只使用系统导出的模板，保持列名和格式不变；导入后核对预览和错误行。",
        "人员变动通知与汇总表是独立功能。通知提交不会直接修改用户、项目、方法等主数据。",
    ])


def add_personnel_section(doc, role, mode):
    add_heading(doc, "六、人员变动通知与项目人员汇总表", 1)
    if mode == "sender":
        doc.add_paragraph("本角色可以发起人员变动通知，但不能审批。通知默认展示近 7 天记录；提交后等待系统管理员或分析检测组长处理。")
        add_steps(doc, [
            ("进入发起通知", "从首页进入“人员变动通知”，打开“发起通知”。选择本人关联的实验室；研发送样组长只显示本人关联实验室。"),
            ("选择变动类型", "选择系统配置的变动类型，例如员工加入、员工离职、实验室内人员调动、跨实验室人员调动、实验室新增项目、实验室新增方法。"),
            ("填写人员和项目", "已有人员会显示其已关联项目；涉及项目关联的变动默认勾选已关联项目。取消勾选表示取消该项目关联。新人员按所选实验室列出全部项目，可多选。"),
            ("填写特殊字段", "新增项目需填写项目代号、检测方法以及是否为高新项目（是时填写名称）；新增方法需选择项目并逐条填写方法名称。方法名称不得包含逗号、顿号、分号或换行。"),
            ("提交并跟踪", "核对通知内容后提交。在“近 7 天通知记录”查看状态、审批意见和处理时间；发现填写错误时按当前状态联系审批人处理。"),
        ])
    elif mode == "viewer":
        doc.add_paragraph("本角色不能发起人员变动通知，但可查看本人权限范围内的近 7 天记录和项目人员汇总表。")
        add_steps(doc, [
            ("查看通知记录", "进入“人员变动通知”，打开“近 7 天通知记录”，按实验室、类型、状态和日期筛选。分析检测组长可查看全部实验室。"),
            ("审批通知", "打开待审批记录，核对实验室、人员、项目和方法。点击“通过”或“驳回”并填写必要意见。仅系统管理员和分析检测组长有审批权限。"),
            ("查看汇总表", "进入“项目及人员汇总表”，按实验室、负责人、项目代号、人员和检测方法核对。分析检测组长可导入、导出并维护全部范围数据。"),
        ])
    elif mode == "none":
        doc.add_paragraph("本角色不参与人员变动通知的发起、查看审批或汇总表维护。人员、项目和方法关联如需调整，请联系系统管理员或分析检测组长。")
    else:
        doc.add_paragraph("本角色可以发起、查看和审批人员变动通知，也可以维护项目及人员汇总表和相关配置。")
        add_steps(doc, [
            ("处理通知", "在近 7 天记录中筛选待审批项，核对通知人、实验室、项目、人员、变动类型及附件信息，选择通过或驳回并填写意见。"),
            ("维护汇总表", "导入前使用系统模板并做唯一性检查；导入后抽查实验室负责人、项目代号、人员和检测方法。必要时导出留存。"),
            ("维护配置", "从管理入口进入系统配置，维护人员反馈字段和可选变动类型。调整后用一条测试通知验证表单显示。"),
        ])
    add_heading(doc, "人员变动字段规则", 2)
    add_table(doc, ["变动类型", "填写要点", "审批/汇总表影响"], [
        ("员工加入", "填写新员工和所属实验室，可多选项目", "通过后新增人员与项目、方法关联"),
        ("员工离职", "选择已有人员并核对其项目", "通过后按审批内容取消关联；实际账号和主数据由管理员处理"),
        ("实验室内人员调动", "项目下拉显示课题组长关联实验室的全部项目；默认勾选已关联项目，取消勾选即取消关联", "通过后按项目勾选结果调整"),
        ("跨实验室人员调动", "不填写目标实验室、目标项目；项目留空表示取消当前实验室全部项目关联", "通过后更新当前实验室关联，目标实验室由管理员另行维护"),
        ("实验室新增项目", "填写项目代号、方法，说明是否为高新项目；高新项目填写名称", "通过后新增项目行及人员/方法信息"),
        ("实验室新增方法", "选择项目，方法逐条填写；禁止逗号、顿号、分号和换行", "通过后在对应项目方法列表中增加方法"),
    ], [1.45, 3.5, 2.0])
    doc.add_paragraph("注意：人员变动通知本身不写入后端数据库。审批通过只更新独立的项目及人员汇总表；真实人员、实验室、项目和方法主数据仍由管理员或分析检测组长按系统流程手工调整。")


def add_sample_info(doc):
    add_heading(doc, "五、样品信息登记", 1)
    doc.add_paragraph("样品信息登记是正式业务数据流程，与人员变动通知相互独立。提交后的登记信息按系统权限保存并供后续查询、检测和统计使用。")
    add_steps(doc, [
        ("进入登记入口", "从首页打开“样品信息登记”，选择新增登记或进入已有记录。"),
        ("选择项目和样品", "按页面顺序填写项目、样品编号/名称、来源、数量、单位、接收信息等字段；先选项目再选择该项目可用的检测方法。"),
        ("填写检测信息", "按实际任务填写方法、仪器、优先级、备注和附件。附件上传完成后确认文件名和预览可用。"),
        ("保存或提交", "信息未齐全时保存草稿；确认无误后提交。提交后查看列表状态，避免重复提交。"),
        ("查询和修订", "在登记列表使用编号、项目、日期和状态查询。只有当前角色授权的记录可编辑；已进入后续环节的记录按页面提示处理。"),
    ])
    add_heading(doc, "样品信息登记检查清单", 2)
    add_bullets(doc, [
        "项目、实验室和检测方法三者匹配，样品编号无重复。",
        "必填字段已完成，数量和单位与实物/送样单一致。",
        "附件能正常打开，备注没有把多个样品信息混写。",
        "提交后状态已更新；需要后续处理时及时通知对应分析检测人员。",
    ])


def add_troubleshooting(doc, role):
    add_heading(doc, "七、日常检查与常见问题", 1)
    add_table(doc, ["现象", "排查与处理"], [
        ("登录后菜单少于预期", "核对右上角角色、组织和实验室范围；权限由管理员配置，不要重复注册账号。"),
        ("列表没有数据", "检查日期范围、状态和实验室筛选条件，点击刷新；仍无数据时联系管理员核对数据权限。"),
        ("提交按钮不可用", "检查必填项、格式校验、附件上传和网络状态；按页面提示逐项修正。"),
        ("通知审批后汇总表未变化", "确认审批状态为“通过”，刷新汇总表；实际主数据调整仍需管理员或分析检测组长手工完成。"),
        ("方法名称无法提交", "逐条输入方法名称，不得包含逗号、顿号、分号或换行；不要把多个方法合并到一个输入框。"),
        ("导入汇总表报错", "使用系统导出模板，保持列名、编码和必填列；清理重复项目/人员行后重新导入并查看错误明细。"),
        ("页面卡顿或提示网络错误", "先保存当前内容，刷新后重新登录；持续出现时记录时间、页面和操作步骤并提交管理员。"),
    ], [2.2, 4.7])
    add_heading(doc, "七、岗位交接与安全要求", 1)
    add_bullets(doc, [
        "每日结束前确认本人提交的记录状态，未完成事项通过系统通知或组织规定渠道交接。",
        "不得共用账号、代替他人审批或使用他人角色操作；所有审批和业务记录均保留操作人信息。",
        "导出文件属于业务数据，按组织规定保存和传递，使用完毕及时清理本地临时副本。",
        "版本升级后先确认页面显示 v2.0.0，再按本手册操作；发现与手册不一致时以系统实际提示为准并反馈管理员。",
    ])
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    p.paragraph_format.space_before = Pt(20)
    r = p.add_run("—— 本手册完 ——")
    r.font.color.rgb = RGBColor(89, 89, 89)


def make_manual(role, filename, can_notify, can_approve, scope, personnel_mode, extra_heading=None, extra_body=None):
    doc = Document()
    setup_doc(doc, role)
    common_intro(doc, role, can_notify, can_approve, scope)
    if extra_heading:
        add_heading(doc, extra_heading, 1)
        doc.add_paragraph(extra_body)
    if role.startswith("研发送样"):
        add_heading(doc, "四、研发送样业务流程", 1)
        add_steps(doc, [
            ("进入研发送样", "从首页进入“研发送样”，点击新建任务或打开已有任务。组长可查看本实验室记录，普通送样员主要处理本人相关记录。"),
            ("填写送样信息", "选择实验室、项目、样品和检测方法，填写送样目的、数量、优先级、截止时间和备注，按页面提示上传附件。"),
            ("保存并提交", "信息未齐全时保存草稿；确认后提交。提交后记录进入对应处理流程，避免重复创建相同任务。"),
            ("跟踪状态", "在研发送样记录中按日期、项目、状态查询。组长按本实验室查看统计和待处理事项。"),
        ])
        add_sample_info(doc)
    elif role.startswith("研发分析员"):
        add_heading(doc, "四、分析检测业务流程", 1)
        add_steps(doc, [
            ("进入分析检测", "从首页进入“分析检测”，查看分配到本人范围内的待取样、检测中和待完成任务。"),
            ("取样", "打开任务核对样品编号、项目、方法和数量，确认实际样品后执行取样，系统记录取样人和时间。"),
            ("完成检测", "按任务要求完成检测，录入结果、单位、异常说明和附件，核对后点击完成检测。"),
            ("查看工作量", "从工作量入口按系统开放的范围查看个人或授权范围统计；统计结果用于工作安排，不替代原始检测记录。"),
            ("查询历史", "在分析检测记录中按编号、项目、日期和状态查询；异常结果按组织流程反馈，不要擅自修改原始数据。"),
        ])
        add_sample_info(doc)
    else:
        add_heading(doc, "四、分析检测管理与业务流程", 1)
        add_steps(doc, [
            ("查看工作台", "进入分析检测和研发送样门户，按状态、日期、实验室和项目查看全部授权范围任务。"),
            ("处理送样任务", "按需要执行取样、退回、完成检测；退回时填写明确原因，完成前核对样品、方法、结果和附件。"),
            ("复核统计", "进入分析检测统计、研发送样统计和记录列表，核对任务量、状态和人员归属，必要时导出留存。"),
        ])
        add_sample_info(doc)
        add_heading(doc, "管理入口补充操作", 2)
        add_steps(doc, [
            ("维护基础资料", "在管理入口按授权维护用户、角色、组织、实验室、项目、检测方法、仪器等主数据；调整前先核对现有关联记录。"),
            ("维护通知中心", "进入通知中心配置通知规则、接收对象和启停状态。人员变动反馈使用独立通知规则；规则字段按表单字段和样品信息登记自定义列同步。"),
            ("维护人员反馈字段", "从管理入口的系统配置进入人员反馈字段配置，新增、编辑、停用或删除允许的变动类型和表单字段；保存后用测试记录核对页面。"),
            ("备份与审计", "按组织要求执行备份、查看审计日志和回收站记录。涉及恢复、批量删除或数据治理操作时，先完成审批和留痕。"),
        ])
    add_personnel_section(doc, role, personnel_mode)
    add_troubleshooting(doc, role)
    doc.core_properties.title = f"{role}使用说明书 V2.0.0"
    doc.core_properties.subject = "实验室流程管理系统岗位操作手册"
    doc.core_properties.author = "LabFlow"
    doc.core_properties.comments = "基于 v2.0.0 实际角色权限编写"
    path = OUT / filename
    doc.save(path)
    return path


def main():
    manuals = [
        ("研发送样员", "研发送样员使用手册_v2.0.0.docx", True, False, "本人相关的送样任务；不能代替组长审批或处理其他人员任务", "sender"),
        ("研发送样组长", "研发送样组长使用手册_v2.0.0.docx", True, False, "本实验室研发送样记录和统计；只能查看本人关联实验室", "sender"),
        ("研发分析员（系统角色：分析检测员）", "研发分析员使用手册_v2.0.0.docx", False, False, "本人授权范围内的分析检测任务；研发送样记录通常只读", "none"),
        ("研发分析组长（系统角色：分析检测组长）", "研发分析组长使用手册_v2.0.0.docx", False, True, "全部授权范围的研发送样和分析检测记录；可跨实验室查看和管理", "viewer"),
        ("管理员（系统角色：系统管理员）", "管理员使用手册_v2.0.0.docx", True, True, "全部组织、实验室、项目、方法、用户和业务记录", "admin"),
    ]
    for args in manuals:
        print(make_manual(*args))


if __name__ == "__main__":
    main()
