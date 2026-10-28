use super::docx_parser::{DocxResult, TocItem};

/// 从 PDF 字节提取纯文本，尝试识别标题
pub fn parse_pdf(data: &[u8]) -> Result<DocxResult, String> {
    let raw =
        pdf_extract::extract_text_from_mem(data).map_err(|e| format!("PDF解析失败: {}", e))?;

    let mut html = String::from("<div>");
    let mut toc: Vec<TocItem> = Vec::new();
    let mut toc_id = 0;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 启发式：检测标题
        // 规则1: 中文数字编号 "第一章", "第1章"
        let is_cn_chapter = trimmed.starts_with('第')
            && (trimmed.contains("章") || trimmed.contains("节") || trimmed.contains("条"));

        // 规则2: 数字编号 "1.", "1.1", "1.1.1"
        let is_num_heading = trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
            && (trimmed.contains(". ") || trimmed.contains("、") || trimmed.contains("．"))
            && trimmed.len() < 60;

        // 规则3: 全大写短行 (英文标题)
        let is_upper_title = trimmed.len() > 5
            && trimmed.len() < 80
            && trimmed.chars().filter(|c| c.is_alphabetic()).count() > 5
            && trimmed
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(|c| c.is_uppercase() || c == ' ');

        if is_cn_chapter || is_num_heading || is_upper_title {
            toc_id += 1;
            let level = if is_cn_chapter {
                1
            } else if is_num_heading {
                trimmed
                    .chars()
                    .take_while(|c| *c != ' ' && *c != '.' && *c != '、')
                    .collect::<String>()
                    .matches('.')
                    .count() as u8
                    + 1
            } else {
                2
            };
            let id = format!("h_{}", toc_id);
            html.push_str(&format!(
                "<h{} id=\"{}\">{}</h{}>",
                level, id, trimmed, level
            ));
            toc.push(TocItem {
                id,
                text: trimmed.to_string(),
                level,
                children: vec![],
            });
        } else {
            html.push_str(&format!("<p>{}</p>", trimmed));
        }
    }

    html.push_str("</div>");
    Ok(DocxResult { html, toc })
}
