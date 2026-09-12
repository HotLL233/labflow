use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRecord {
    pub project_name: String,
    pub group_name: String,
    pub recorded_at: String, // YYYY-MM-DD
    pub batch_no: String,
    pub quantity: i64,
    pub user_name: Option<String>,
    pub extra_info: Option<String>,
}

/// 导入结果明细，直接返回到前端展示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// 是否成功
    pub success: bool,
    /// Excel 中读取到的总行数（含跳过的空行）
    pub total_rows_read: usize,
    /// 新插入的记录数
    pub inserted: usize,
    /// 数量累加更新的记录数
    pub updated: usize,
    /// 跳过的无效行数（日期/批号/数量不合法）
    pub skipped: usize,
    /// 处理的 Sheet 名称
    pub sheet_name: String,
    /// 检测到的列名
    pub columns_found: Vec<String>,
    /// 数据库导入过程中的错误（如果有）
    pub errors: Vec<String>,
    /// 提示信息
    pub warnings: Vec<String>,
}

impl ImportResult {
    pub fn summary(&self) -> String {
        if !self.success {
            return format!("导入失败: {}", self.errors.join("; "));
        }
        let mut parts = vec![];
        if self.inserted > 0 {
            parts.push(format!("新增 {} 条", self.inserted));
        }
        if self.updated > 0 {
            parts.push(format!("累加 {} 条", self.updated));
        }
        if self.skipped > 0 {
            parts.push(format!("跳过 {} 行", self.skipped));
        }
        if parts.is_empty() {
            "无有效数据".to_string()
        } else {
            parts.join("，")
        }
    }
}
