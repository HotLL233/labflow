use crate::error::Result;

pub fn ensure_seeded(_conn: &postgres_compat::Connection) -> Result<()> {
    // v0.2.17: 种子数据已移除，实验室/项目/方法通过导入或手动创建
    Ok(())
}
