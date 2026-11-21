use crate::error::{AppError, Result};
use crate::service::step_workload::{self, Progress, StepEntry};
use postgres_compat::Connection;
use postgres_compat::OptionalExtension;
use std::collections::HashMap;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct NewQuantityGroup {
    pub name: String,
    pub quantity: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct StepSelection {
    pub template_id: i64,
    #[serde(default)]
    pub template_ids: Vec<i64>,
    #[serde(default)]
    pub detection_id: Option<i64>,
    #[serde(default)]
    pub detection_quantity: Option<i32>,
    #[serde(default)]
    pub group_id: Option<i64>,
    #[serde(default)]
    pub groups: Option<Vec<NewQuantityGroup>>,
    #[serde(default)]
    pub group_index: Option<usize>,
    #[serde(default)]
    pub rework: bool,
    #[serde(default)]
    pub extra_charge: bool,
}

pub(crate) struct PreparedStep {
    pub detection_id: i64,
    pub group_id: i64,
    pub step_id: i64,
    pub coefficient: f64,
    pub rework: bool,
    pub extra_charge: bool,
}

pub(crate) fn preview(
    conn: &Connection,
    kind: &str,
    source: i64,
    method: Option<i64>,
) -> Result<Option<serde_json::Value>> {
    let detection: Option<(i64, i64, f64, bool, Option<i64>, Option<i64>, i32, i64)> = conn.query_row(
        "SELECT id,method_id,multiplier,ordered,project_id,group_id,quantity,revision FROM step_detections WHERE (source_type=?1 AND source_record_id=?2) OR (?1='manual' AND id=?2 AND source_type IS NULL)",
        postgres_compat::params![kind,source], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
    ).optional()?;
    if let Some((
        id,
        frozen_method,
        multiplier,
        ordered,
        project_id,
        group_id,
        quantity,
        revision,
    )) = detection
    {
        let steps = conn.prepare("SELECT template_id,name_snapshot,coefficient_snapshot FROM detection_steps WHERE detection_id=?1 ORDER BY sort_order,id")?
            .query_map([id], |row| Ok(serde_json::json!({"id":row.get::<_,i64>(0)?,"name":row.get::<_,String>(1)?,"coefficient":row.get::<_,f64>(2)?})))?
            .collect::<postgres_compat::Result<Vec<_>>>()?;
        let groups = conn.prepare("SELECT id,name,quantity FROM detection_quantity_groups WHERE detection_id=?1 ORDER BY id")?
            .query_map([id], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,i32>(2)?)))?
            .collect::<postgres_compat::Result<Vec<_>>>()?;
        let mut group_values = Vec::with_capacity(groups.len());
        for (group_id, name, quantity) in groups {
            let progress = conn.prepare("SELECT s.template_id,COALESCE(SUM(w.quantity) FILTER (WHERE w.deleted_at IS NULL AND NOT e.rework),0)::BIGINT FROM detection_steps s LEFT JOIN step_work_entries e ON e.step_id=s.id AND e.group_id=?1 LEFT JOIN work_records w ON w.id=e.work_record_id WHERE s.detection_id=?2 GROUP BY s.id,s.template_id ORDER BY s.sort_order,s.id")?
                .query_map(postgres_compat::params![group_id,id], |row| Ok(serde_json::json!({"step_id":row.get::<_,i64>(0)?,"quantity":row.get::<_,i64>(1)?})))?
                .collect::<postgres_compat::Result<Vec<_>>>()?;
            group_values.push(serde_json::json!({"id":group_id,"name":name,"quantity":quantity,"progress":progress}));
        }
        return Ok(Some(
            serde_json::json!({"started":true,"detection_id":id,"revision":revision,"quantity":quantity,"method_id":frozen_method,"project_id":project_id,"group_id":group_id,"multiplier":multiplier,"ordered":ordered,"steps":steps,"groups":group_values}),
        ));
    }
    let Some(method_id) = method else {
        return Ok(None);
    };
    let config = crate::repo::step_config_repo::get(conn, method_id)?;
    let steps: Vec<_> = config.steps.into_iter().filter(|s| s.enabled).collect();
    if steps.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        serde_json::json!({"started":false,"method_id":method_id,"ordered":config.ordered,"steps":steps,"groups":[]}),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct GroupChange {
    pub revision: i64,
    pub operation: String,
    pub group_id: i64,
    pub other_group_id: Option<i64>,
    pub quantity: Option<i32>,
    pub name: Option<String>,
    pub preview_token: Option<String>,
}

// Preview and apply use the same source-then-detection lock order as workload writes.
pub(crate) fn change_groups(
    pool: &crate::db::DbPool,
    detection_id: i64,
    request: &GroupChange,
    preview_only: bool,
    operator: &str,
    source_snapshot: Option<&str>,
) -> Result<serde_json::Value> {
    use sha2::{Digest, Sha256};
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let (source_type, source_id): (Option<String>, Option<i64>) = tx
        .query_row(
            "SELECT source_type,source_record_id FROM step_detections WHERE id=?1",
            [detection_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("检测不存在".into()))?;
    if source_type.is_some()
        && crate::repo::record_repo::lock_source_capacity(&tx, source_type.as_deref(), source_id)?
            .is_none()
    {
        return Err(AppError::Validation("取样来源不存在".into()));
    }
    if let Some(expected) = source_snapshot {
        let table = match source_type.as_deref() {
            Some("rd_sample") => "rd_work_records",
            Some("sample_info_sample") => "sample_info_records",
            _ => return Err(AppError::Conflict("检测来源已变化，请重新预览".into())),
        };
        let actual: String = tx.query_row(
            &format!("SELECT to_jsonb(s)::text FROM {table} s WHERE id=?1"),
            [source_id],
            |r| r.get(0),
        )?;
        if actual != expected {
            return Err(AppError::Conflict("来源记录已变化，请重新预览".into()));
        }
    }
    let revision: i64 = tx.query_row(
        "SELECT revision FROM step_detections WHERE id=?1 FOR UPDATE",
        [detection_id],
        |r| r.get(0),
    )?;
    if revision != request.revision {
        return Err(AppError::Conflict(
            "检测进度已变化，请重新预览分组调整".into(),
        ));
    }
    let (kind, source) = match (source_type.as_deref(), source_id) {
        (Some("rd_sample"), Some(id)) => {
            let valid: bool = tx.query_row("SELECT deleted_at IS NULL AND status='已取样' AND sampled_at IS NOT NULL FROM rd_work_records WHERE id=?1", [id], |r| r.get(0))?;
            if !valid {
                return Err(AppError::Validation("研发送样来源不可调整".into()));
            }
            ("rd_sample", id)
        }
        (Some("sample_info_sample"), Some(id)) => {
            let valid: bool = tx.query_row("SELECT deleted_at IS NULL AND status IN ('待检测','已检测') AND sampled_at IS NOT NULL FROM sample_info_records WHERE id=?1", [id], |r| r.get(0))?;
            if !valid {
                return Err(AppError::Validation("样品来源不可调整".into()));
            }
            ("sample_info_sample", id)
        }
        (None, None) => ("manual", detection_id),
        _ => return Err(AppError::Validation("检测来源无效".into())),
    };
    let mut before =
        preview(&tx, kind, source, None)?.ok_or_else(|| AppError::NotFound("检测不存在".into()))?;
    let charge = |conn: &Connection| -> Result<f64> {
        Ok(conn.query_row("SELECT COALESCE(SUM(quantity * unit_price * multiplier),0)::DOUBLE PRECISION FROM step_detection_charges WHERE detection_id=?1 AND rework_record_id IS NULL AND voided_at IS NULL", [detection_id], |r| r.get(0))?)
    };
    let charge_before = charge(&tx)?;
    if request.operation == "merge" {
        let ids = tx.prepare("SELECT work_record_id FROM step_work_entries WHERE detection_id=?1 AND group_id=?2 ORDER BY work_record_id")?
            .query_map(postgres_compat::params![detection_id,request.other_group_id], |r| r.get::<_,i64>(0))?
            .collect::<postgres_compat::Result<Vec<_>>>()?;
        before["moved_work_record_ids"] = serde_json::json!(ids);
    }
    let proposal = serde_json::json!({"detection_id":detection_id,"revision":request.revision,"operation":request.operation,"group_id":request.group_id,"other_group_id":request.other_group_id,"quantity":request.quantity,"name":request.name});
    let digest = Sha256::digest(
        serde_json::to_vec(&serde_json::json!({"before":before,"proposal":proposal}))
            .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    let token = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if !preview_only && request.preview_token.as_deref() != Some(token.as_str()) {
        return Err(AppError::Conflict("请先预览当前分组调整并确认".into()));
    }
    let old_quantity: i32 = tx.query_row(
        "SELECT quantity FROM detection_quantity_groups WHERE id=?1 AND detection_id=?2",
        postgres_compat::params![request.group_id, detection_id],
        |r| r.get(0),
    )?;
    match request.operation.as_str() {
        "split" => {
            let quantity = request
                .quantity
                .filter(|value| *value > 0 && *value < old_quantity)
                .ok_or_else(|| AppError::Validation("拆出数量必须大于0且小于原分组数量".into()))?;
            let name = request.name.as_deref().unwrap_or("").trim();
            if name.is_empty() || name.chars().count() > 100 || name.contains('\0') {
                return Err(AppError::Validation("新分组名称无效".into()));
            }
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM detection_quantity_groups WHERE detection_id=?1",
                [detection_id],
                |r| r.get(0),
            )?;
            if count >= 100 {
                return Err(AppError::Validation("最多允许100个分组".into()));
            }
            tx.execute(
                "UPDATE detection_quantity_groups SET quantity=quantity-?1 WHERE id=?2",
                postgres_compat::params![quantity, request.group_id],
            )?;
            tx.execute("INSERT INTO detection_quantity_groups(detection_id,name,quantity) VALUES(?1,?2,?3)", postgres_compat::params![detection_id,name,quantity])?;
        }
        "merge" => {
            let other = request
                .other_group_id
                .filter(|id| *id != request.group_id)
                .ok_or_else(|| AppError::Validation("请选择不同的合并分组".into()))?;
            let other_quantity: i32 = tx.query_row(
                "SELECT quantity FROM detection_quantity_groups WHERE id=?1 AND detection_id=?2",
                postgres_compat::params![other, detection_id],
                |r| r.get(0),
            )?;
            let total = old_quantity
                .checked_add(other_quantity)
                .ok_or_else(|| AppError::Validation("合并数量超出范围".into()))?;
            tx.execute(
                "UPDATE detection_quantity_groups SET quantity=?1 WHERE id=?2",
                postgres_compat::params![total, request.group_id],
            )?;
            tx.execute(
                "UPDATE step_work_entries SET group_id=?1 WHERE detection_id=?2 AND group_id=?3",
                postgres_compat::params![request.group_id, detection_id, other],
            )?;
            tx.execute(
                "DELETE FROM detection_quantity_groups WHERE id=?1 AND detection_id=?2",
                postgres_compat::params![other, detection_id],
            )?;
        }
        _ => return Err(AppError::Validation("分组调整类型无效".into())),
    }
    reconcile(&tx, detection_id)?;
    let after =
        preview(&tx, kind, source, None)?.ok_or_else(|| AppError::NotFound("检测不存在".into()))?;
    let charge_after = charge(&tx)?;
    if preview_only {
        tx.rollback()?;
    } else {
        crate::repo::audit_repo::log_structured_on_conn(
            &tx,
            "update",
            "detection_quantity_groups",
            Some(detection_id),
            operator,
            "确认调整检测数量分组",
            "work",
            "",
            Some(&before),
            Some(&after),
            "workload",
        )?;
        tx.commit()?;
    }
    Ok(
        serde_json::json!({"before":before,"after":after,"charge_before":charge_before,"charge_after":charge_after,"preview_token":token,"applied":!preview_only}),
    )
}

/// The caller locks the source row before this function and inserts the work row in the same transaction.
pub(crate) fn prepare(
    conn: &Connection,
    source_type: Option<&str>,
    source_id: Option<i64>,
    source_capacity: Option<i64>,
    method_id: Option<i64>,
    project_id: i64,
    group_id: Option<i64>,
    requested_multiplier: Option<f64>,
    operator: &str,
    selection: Option<StepSelection>,
) -> Result<Option<PreparedStep>> {
    use postgres_compat::OptionalExtension;
    let canonical = match source_type {
        Some("rd_sample") => Some("rd_sample"),
        Some("sample_info" | "sample_info_sample") => Some("sample_info_sample"),
        _ => None,
    };
    let manual_id = if source_type.is_none() && source_id.is_none() {
        selection.as_ref().and_then(|s| s.detection_id)
    } else {
        None
    };
    let existing: Option<(i64, i64, f64, Option<i64>, Option<i64>)> = match (canonical, source_id, manual_id) {
        (Some(kind), Some(id), _) => conn.query_row(
            "SELECT id,method_id,multiplier,project_id,group_id FROM step_detections WHERE source_type=?1 AND source_record_id=?2 FOR UPDATE",
            postgres_compat::params![kind,id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
        ).optional()?,
        (None, None, Some(id)) => Some(conn.query_row(
            "SELECT id,method_id,multiplier,project_id,group_id FROM step_detections WHERE id=?1 AND source_type IS NULL FOR UPDATE",
            [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
        ).optional()?.ok_or_else(|| AppError::Validation("手动检测不存在，请刷新后重新选择".into()))?),
        _ => None,
    };
    let enabled = if let Some(method) = method_id {
        // Serialize the step snapshot with configuration saves, which lock this method row.
        conn.query_row(
            "SELECT id FROM methods WHERE id=?1 FOR SHARE",
            [method],
            |row| row.get::<_, i64>(0),
        )?;
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM method_work_steps WHERE method_id=?1 AND enabled)",
            [method],
            |row| row.get::<_, bool>(0),
        )?
    } else {
        false
    };
    if existing.is_none() && !enabled {
        if selection.is_some() {
            return Err(AppError::Validation("所选方法没有可用步骤".into()));
        }
        return Ok(None);
    }
    let selection = selection.ok_or_else(|| {
        AppError::Validation("该方法需要选择检测步骤，请更新客户端并重新录入".into())
    })?;
    if selection.template_id <= 0
        || (selection.extra_charge && !selection.rework)
        || (selection.group_id.is_some() && selection.group_index.is_some())
        || (source_type.is_some()
            && (selection.detection_id.is_some() || selection.detection_quantity.is_some()))
        || (manual_id.is_some() && selection.detection_quantity.is_some())
    {
        return Err(AppError::Validation("步骤或返工计费设置无效".into()));
    }
    let method = method_id.ok_or_else(|| AppError::Validation("请选择实际检测方法".into()))?;
    if (source_type.is_some() && canonical.is_none())
        || (canonical.is_some() && source_id.is_none())
        || (source_type.is_none() && source_id.is_some())
    {
        return Err(AppError::Validation("检测来源无效".into()));
    }
    let source = source_id.unwrap_or(0);
    let detection_id = if let Some((
        id,
        frozen_method,
        frozen_multiplier,
        frozen_project,
        frozen_group,
    )) = existing
    {
        if selection.groups.is_some() || selection.group_index.is_some() {
            return Err(AppError::Validation(
                "已开始检测的数量分组不能通过新建入口修改".into(),
            ));
        }
        if frozen_method != method {
            return Err(AppError::Validation(
                "该送样已开始多步骤检测，请沿用原计量方法".into(),
            ));
        }
        if frozen_project != Some(project_id) || frozen_group != group_id {
            return Err(AppError::Validation(
                "该检测的计量项目和实验室已锁定，请沿用原归属".into(),
            ));
        }
        if requested_multiplier.is_some_and(|value| value != frozen_multiplier) {
            return Err(AppError::Validation(
                "该检测的计费倍率已锁定，请刷新后使用原倍率".into(),
            ));
        }
        id
    } else {
        let quantity = i32::try_from(if canonical.is_some() {
            source_capacity.ok_or_else(|| AppError::Validation("取样来源不存在".into()))?
        } else {
            i64::from(
                selection
                    .detection_quantity
                    .ok_or_else(|| AppError::Validation("请填写手动检测总数量".into()))?,
            )
        })
        .map_err(|_| AppError::Validation("来源数量超出支持范围".into()))?;
        if quantity <= 0 {
            return Err(AppError::Validation("来源数量无效".into()));
        }
        let old_exists: bool = if canonical == Some("rd_sample") {
            conn.query_row("SELECT EXISTS(SELECT 1 FROM work_records WHERE source_type='rd_sample' AND source_record_id=?1)", [source], |row| row.get(0))?
        } else if canonical.is_some() {
            conn.query_row("SELECT EXISTS(SELECT 1 FROM work_records WHERE source_type IN ('sample_info','sample_info_sample') AND source_record_id=?1)", [source], |row| row.get(0))?
        } else {
            false
        };
        if old_exists {
            return Err(AppError::Validation(
                "该来源已有历史工作量，不能自动转为多步骤检测".into(),
            ));
        }
        let inserted = conn.execute(
            "INSERT INTO step_detections(source_type,source_record_id,method_id,quantity,ordered,unit_price,multiplier,created_by_user_id,project_id,group_id)
             SELECT ?1,?2,m.id,?3,COALESCE(s.ordered,FALSE),COALESCE(m.amount,0),COALESCE(?6,m.multiplier,1),u.id,?7,?8
             FROM methods m JOIN users u ON u.username=?4 LEFT JOIN method_step_settings s ON s.method_id=m.id WHERE m.id=?5 AND m.deleted_at IS NULL",
            postgres_compat::params![canonical,source_id,quantity,operator,method,requested_multiplier,project_id,group_id],
        )?;
        if inserted != 1 {
            return Err(AppError::Validation("方法或操作账号不存在".into()));
        }
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO detection_steps(detection_id,template_id,name_snapshot,coefficient_snapshot,sort_order)
             SELECT ?1,id,name,coefficient,sort_order FROM method_work_steps WHERE method_id=?2 AND enabled ORDER BY sort_order,id",
            postgres_compat::params![id,method],
        )?;
        if let Some(groups) = &selection.groups {
            if groups.is_empty() || groups.len() > 100 {
                return Err(AppError::Validation("数量分组必须为1至100组".into()));
            }
            let mut total = 0_i32;
            for group in groups {
                let name = group.name.trim();
                if name.is_empty()
                    || name.chars().count() > 100
                    || name.contains('\0')
                    || group.quantity <= 0
                {
                    return Err(AppError::Validation("分组名称或数量无效".into()));
                }
                total = total
                    .checked_add(group.quantity)
                    .ok_or_else(|| AppError::Validation("分组数量超出范围".into()))?;
                conn.execute("INSERT INTO detection_quantity_groups(detection_id,name,quantity) VALUES(?1,?2,?3)", postgres_compat::params![id,name,group.quantity])?;
            }
            if total != quantity {
                return Err(AppError::Validation("分组数量合计必须等于送样数量".into()));
            }
        } else {
            conn.execute("INSERT INTO detection_quantity_groups(detection_id,name,quantity) VALUES(?1,'全部样品',?2)", postgres_compat::params![id,quantity])?;
        }
        id
    };
    let (step_id, coefficient): (i64,f64) = conn.query_row(
        "SELECT id,coefficient_snapshot FROM detection_steps WHERE detection_id=?1 AND template_id=?2",
        postgres_compat::params![detection_id,selection.template_id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional()?.ok_or_else(|| AppError::Validation("所选步骤不属于该检测，请刷新后重试".into()))?;
    let group_id: i64 = if let Some(group_id) = selection.group_id {
        conn.query_row(
            "SELECT id FROM detection_quantity_groups WHERE id=?1 AND detection_id=?2",
            postgres_compat::params![group_id, detection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("数量分组不存在，请刷新后重试".into()))?
    } else {
        let groups: Vec<i64> = conn
            .prepare("SELECT id FROM detection_quantity_groups WHERE detection_id=?1 ORDER BY id")?
            .query_map([detection_id], |row| row.get(0))?
            .collect::<postgres_compat::Result<_>>()?;
        if let Some(index) = selection.group_index {
            *groups
                .get(index)
                .ok_or_else(|| AppError::Validation("请选择有效的数量分组".into()))?
        } else if groups.len() != 1 {
            return Err(AppError::Validation("请选择数量分组".into()));
        } else {
            groups[0]
        }
    };
    Ok(Some(PreparedStep {
        detection_id,
        group_id,
        step_id,
        coefficient,
        rework: selection.rework,
        extra_charge: selection.extra_charge,
    }))
}

pub(crate) fn attach(
    conn: &Connection,
    work_record_id: i64,
    quantity: i32,
    step: &PreparedStep,
) -> Result<()> {
    conn.execute("INSERT INTO step_work_entries(work_record_id,detection_id,group_id,step_id,rework) VALUES(?1,?2,?3,?4,?5)",
        postgres_compat::params![work_record_id,step.detection_id,step.group_id,step.step_id,step.rework])?;
    reconcile(conn, step.detection_id)?;
    if step.extra_charge {
        conn.execute("INSERT INTO step_detection_charges(detection_id,rework_record_id,quantity,unit_price,multiplier,charged_at) SELECT id,?2,?3,unit_price,multiplier,CURRENT_TIMESTAMP FROM step_detections WHERE id=?1",
            postgres_compat::params![step.detection_id,work_record_id,quantity])?;
    }
    Ok(())
}

pub(crate) fn detection_for_record(conn: &Connection, work_record_id: i64) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT detection_id FROM step_work_entries WHERE work_record_id=?1",
            [work_record_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Called after a linked row is edited, deleted or restored while its detection is locked.
pub(crate) fn reconcile_record(
    conn: &Connection,
    work_record_id: i64,
    detection_id: i64,
) -> Result<()> {
    let (quantity, deleted, rework): (i32, bool, bool) = conn.query_row(
        "SELECT w.quantity,w.deleted_at IS NOT NULL,e.rework FROM work_records w JOIN step_work_entries e ON e.work_record_id=w.id WHERE w.id=?1 AND e.detection_id=?2",
        postgres_compat::params![work_record_id,detection_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    )?;
    if rework {
        if deleted {
            conn.execute("UPDATE step_detection_charges SET voided_at=CURRENT_TIMESTAMP,void_reason='record_deleted' WHERE rework_record_id=?1 AND voided_at IS NULL", [work_record_id])?;
        } else {
            let active: Option<(i64,i32)> = conn.query_row(
                "SELECT id,quantity FROM step_detection_charges WHERE rework_record_id=?1 AND voided_at IS NULL",
                [work_record_id], |row| Ok((row.get(0)?,row.get(1)?)),
            ).optional()?;
            if let Some((charge_id, old_quantity)) = active {
                if old_quantity != quantity {
                    conn.execute("UPDATE step_detection_charges SET voided_at=CURRENT_TIMESTAMP,void_reason='quantity_changed' WHERE id=?1", [charge_id])?;
                    conn.execute("INSERT INTO step_detection_charges(detection_id,rework_record_id,quantity,unit_price,multiplier,charged_at) SELECT detection_id,rework_record_id,?2,unit_price,multiplier,CURRENT_TIMESTAMP FROM step_detection_charges WHERE id=?1", postgres_compat::params![charge_id,quantity])?;
                }
            } else {
                conn.execute("INSERT INTO step_detection_charges(detection_id,rework_record_id,quantity,unit_price,multiplier,charged_at) SELECT detection_id,rework_record_id,?2,unit_price,multiplier,CURRENT_TIMESTAMP FROM step_detection_charges WHERE id=(SELECT id FROM step_detection_charges WHERE rework_record_id=?1 ORDER BY id DESC LIMIT 1) AND void_reason='record_deleted'", postgres_compat::params![work_record_id,quantity])?;
            }
        }
    }
    reconcile(conn, detection_id)?;
    Ok(())
}

/// Call inside the transaction that changes the detection or a linked work record.
pub(crate) fn reconcile(conn: &Connection, detection_id: i64) -> Result<Progress> {
    let (total, ordered, price, multiplier): (i32, bool, f64, f64) = conn.query_row(
        "SELECT quantity,ordered,unit_price,multiplier FROM step_detections WHERE id=?1 FOR UPDATE",
        [detection_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let groups: Vec<(i64, i32)> = conn
        .prepare(
            "SELECT id,quantity FROM detection_quantity_groups WHERE detection_id=?1 ORDER BY id",
        )?
        .query_map([detection_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<postgres_compat::Result<_>>()?;
    let steps: Vec<i64> = conn
        .prepare("SELECT id FROM detection_steps WHERE detection_id=?1 ORDER BY sort_order,id")?
        .query_map([detection_id], |row| row.get(0))?
        .collect::<postgres_compat::Result<_>>()?;
    let group_indexes: HashMap<i64, usize> = groups
        .iter()
        .enumerate()
        .map(|(i, item)| (item.0, i))
        .collect();
    let step_indexes: HashMap<i64, usize> =
        steps.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let records: Vec<(i64, i64, i32, bool)> = conn
        .prepare("SELECT e.group_id,e.step_id,w.quantity,e.rework FROM step_work_entries e JOIN work_records w ON w.id=e.work_record_id WHERE e.detection_id=?1 AND w.deleted_at IS NULL")?
        .query_map([detection_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
        .collect::<postgres_compat::Result<_>>()?;
    let entries: Vec<StepEntry> = records
        .iter()
        .map(|(group, step, quantity, rework)| {
            Ok(StepEntry {
                group: *group_indexes
                    .get(group)
                    .ok_or_else(|| AppError::Internal("步骤分组关联异常".into()))?,
                step: *step_indexes
                    .get(step)
                    .ok_or_else(|| AppError::Internal("检测步骤关联异常".into()))?,
                quantity: *quantity,
                rework: *rework,
            })
        })
        .collect::<Result<_>>()?;
    let progress = step_workload::progress(
        total,
        &groups.iter().map(|item| item.1).collect::<Vec<_>>(),
        steps.len(),
        &entries,
        ordered,
    )
    .map_err(|message| AppError::Validation(message.into()))?;
    if progress.complete {
        conn.execute(
            "INSERT INTO step_detection_charges(detection_id,quantity,unit_price,multiplier,charged_at)
             SELECT ?1,?2,?3,?4,CURRENT_TIMESTAMP
             WHERE NOT EXISTS (SELECT 1 FROM step_detection_charges WHERE detection_id=?1 AND rework_record_id IS NULL AND voided_at IS NULL)",
            postgres_compat::params![detection_id, total, price, multiplier],
        )?;
    } else {
        conn.execute(
            "UPDATE step_detection_charges SET voided_at=CURRENT_TIMESTAMP WHERE detection_id=?1 AND rework_record_id IS NULL AND voided_at IS NULL",
            [detection_id],
        )?;
    }
    conn.execute(
        "UPDATE step_detections SET revision=revision+1 WHERE id=?1",
        [detection_id],
    )?;
    Ok(progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_charge_is_single_and_reversible() {
        if std::env::var("WORKLOAD_TEST_DATABASE_URL").is_err() {
            return;
        }
        let conn = Connection::open_test_database().expect("isolated test database");
        conn.execute_batch(
            "CREATE TEMP TABLE step_detections(id BIGINT PRIMARY KEY,quantity INTEGER,ordered BOOLEAN,unit_price DOUBLE PRECISION,multiplier DOUBLE PRECISION,revision BIGINT NOT NULL DEFAULT 1);
             CREATE TEMP TABLE detection_quantity_groups(id BIGINT PRIMARY KEY,detection_id BIGINT,quantity INTEGER);
             CREATE TEMP TABLE detection_steps(id BIGINT PRIMARY KEY,detection_id BIGINT,sort_order INTEGER);
             CREATE TEMP TABLE work_records(id BIGINT PRIMARY KEY,quantity BIGINT,deleted_at TEXT);
             CREATE TEMP TABLE step_work_entries(work_record_id BIGINT,detection_id BIGINT,group_id BIGINT,step_id BIGINT,rework BOOLEAN);
             CREATE TEMP TABLE step_detection_charges(id BIGSERIAL PRIMARY KEY,detection_id BIGINT,rework_record_id BIGINT,quantity INTEGER,unit_price DOUBLE PRECISION,multiplier DOUBLE PRECISION,charged_at TIMESTAMPTZ,voided_at TIMESTAMPTZ);
             INSERT INTO step_detections(id,quantity,ordered,unit_price,multiplier) VALUES(1,10,TRUE,20,1.5);
             INSERT INTO detection_quantity_groups VALUES(1,1,5),(2,1,5);
             INSERT INTO detection_steps VALUES(1,1,0),(2,1,1),(3,1,2);
             INSERT INTO work_records VALUES(1,5,NULL),(2,5,NULL),(3,5,NULL),(4,5,NULL),(5,5,NULL),(6,5,NULL);
             INSERT INTO step_work_entries VALUES(1,1,1,1,FALSE),(2,1,1,2,FALSE),(3,1,1,3,FALSE),(4,1,2,1,FALSE),(5,1,2,2,FALSE);",
        ).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        assert!(!reconcile(&tx, 1).unwrap().complete);
        tx.execute("INSERT INTO step_work_entries VALUES(6,1,2,3,FALSE)", [])
            .unwrap();
        assert!(reconcile(&tx, 1).unwrap().complete);
        assert!(reconcile(&tx, 1).unwrap().complete);
        let active = |db: &Connection| -> (i64, f64) {
            db.query_row("SELECT COUNT(*),COALESCE(SUM(quantity*unit_price*multiplier),0) FROM step_detection_charges WHERE voided_at IS NULL", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap()
        };
        assert_eq!(active(&tx), (1, 300.0));
        tx.execute(
            "UPDATE work_records SET deleted_at='deleted' WHERE id=6",
            [],
        )
        .unwrap();
        assert!(!reconcile(&tx, 1).unwrap().complete);
        assert_eq!(active(&tx), (0, 0.0));
        tx.execute("UPDATE work_records SET deleted_at=NULL WHERE id=6", [])
            .unwrap();
        assert!(reconcile(&tx, 1).unwrap().complete);
        assert_eq!(active(&tx), (1, 300.0));
        tx.rollback().unwrap();
    }
}
