use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::article::{HelpArticle, HelpArticleUpdate};
use crate::repo::{audit_repo, trash_repo};

const COLS: &str = "id, title, content_html, toc_json, source_file, is_visible, sort_order, created_at, updated_at";

fn row_to_article(
    row: &postgres_compat::Row,
) -> std::result::Result<HelpArticle, postgres_compat::Error> {
    Ok(HelpArticle {
        id: row.get(0)?,
        title: row.get(1)?,
        content_html: row.get(2)?,
        toc_json: row.get(3)?,
        source_file: row.get(4)?,
        is_visible: row.get::<_, bool>(5).unwrap_or(true),
        sort_order: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub fn list(pool: &DbPool, visible_only: bool) -> Result<Vec<HelpArticle>> {
    let conn = pool.get()?;
    let sql = if visible_only {
        &format!("SELECT {COLS} FROM help_articles WHERE is_visible=1 AND deleted_at IS NULL ORDER BY sort_order,id")
    } else {
        &format!("SELECT {COLS} FROM help_articles WHERE deleted_at IS NULL ORDER BY sort_order,id")
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row_to_article(row))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<HelpArticle> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}

fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<HelpArticle> {
    conn.query_row(
        &format!("SELECT {COLS} FROM help_articles WHERE id = ?1"),
        [id],
        |row| row_to_article(row),
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("文章不存在".into()),
        _ => e.into(),
    })
}

pub fn create(
    pool: &DbPool,
    title: &str,
    content_html: &str,
    toc_json: Option<&str>,
    source_file: Option<&str>,
    operator: &str,
) -> Result<HelpArticle> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO help_articles (title, content_html, toc_json, source_file) VALUES (?1, ?2, ?3, ?4)",
        (title, content_html, toc_json, source_file),
    )?;
    let id = tx.last_insert_rowid();
    let article = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&article).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "help_articles",
        Some(id),
        operator,
        &format!("创建帮助文章「{}」", article.title),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(article)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    body: &HelpArticleUpdate,
    operator: &str,
) -> Result<HelpArticle> {
    let mut conn = pool.get()?;
    let current = get_by_id_on_conn(&conn, id)?;
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    if let Some(ref title) = body.title {
        tx.execute(
            "UPDATE help_articles SET title=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            (title, id),
        )?;
    }
    if let Some(ref html) = body.content_html {
        tx.execute("UPDATE help_articles SET content_html=?1, updated_at=datetime('now','localtime') WHERE id=?2", (html, id))?;
    }
    if let Some(ref toc) = body.toc_json {
        tx.execute("UPDATE help_articles SET toc_json=?1, updated_at=datetime('now','localtime') WHERE id=?2", (toc, id))?;
    }
    if let Some(v) = body.is_visible {
        tx.execute("UPDATE help_articles SET is_visible=?1, updated_at=datetime('now','localtime') WHERE id=?2", (v, id))?;
    }
    if let Some(so) = body.sort_order {
        tx.execute("UPDATE help_articles SET sort_order=?1, updated_at=datetime('now','localtime') WHERE id=?2", (so, id))?;
    }
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "help_articles",
        Some(id),
        operator,
        &format!("修改帮助文章「{}」", updated.title),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<HelpArticle> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let doc = get_by_id_on_conn(&tx, id)?;
    let before = serde_json::to_value(&doc).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE help_articles SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1", [id])?;
    trash_repo::move_to_trash_on_conn(
        &tx,
        "帮助文章",
        "help_articles",
        id,
        "files",
        "shared",
        &doc.title,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        "",
        true,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "help_articles",
        Some(id),
        operator,
        &format!("删除帮助文章「{}」", doc.title),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(doc)
}

pub fn reorder(pool: &DbPool, ids: &[(i64, i64)], operator: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let ids_set: std::collections::HashSet<i64> = ids.iter().map(|(id, _)| *id).collect();
    let before_items: Vec<HelpArticle> = list(pool, false)?
        .into_iter()
        .filter(|article| ids_set.contains(&article.id))
        .collect();
    let before = serde_json::to_value(&before_items).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    for (id, sort_order) in ids {
        tx.execute(
            "UPDATE help_articles SET sort_order=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            (sort_order, id),
        )?;
    }
    let mut stmt = tx.prepare(&format!(
        "SELECT {COLS} FROM help_articles WHERE deleted_at IS NULL ORDER BY sort_order,id"
    ))?;
    let after_items = stmt
        .query_map([], row_to_article)?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|article| ids_set.contains(&article.id))
        .collect::<Vec<_>>();
    drop(stmt);
    let after = serde_json::to_value(&after_items).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "help_articles",
        None,
        operator,
        "调整帮助文章排序",
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}
