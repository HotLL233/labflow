use calamine::DataType;
use std::collections::{HashMap, HashSet};

use crate::error::AppError;

use super::master_import_handler::ImportIssue;

pub(super) fn method_key(name: &str, instrument_code: &str) -> String {
    format!("{}\u{1f}{}", name.trim(), instrument_code.trim())
}

pub(super) fn method_key_parts(key: &str) -> Option<(&str, &str)> {
    key.split_once('\u{1f}')
}

pub(super) fn method_label(key: &str) -> String {
    method_key_parts(key)
        .map(|(name, instrument)| format!("{name} [{instrument}]"))
        .unwrap_or_else(|| key.to_string())
}

pub(super) fn workbook_error(error: calamine::Error) -> AppError {
    AppError::Validation(format!("读取工作表失败: {error}"))
}

pub(super) fn cell_to_string(cell: &DataType) -> String {
    match cell {
        DataType::String(value) | DataType::DateTimeIso(value) | DataType::DurationIso(value) => {
            value.trim().to_string()
        }
        DataType::Float(value) | DataType::DateTime(value) | DataType::Duration(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        DataType::Int(value) => value.to_string(),
        DataType::Bool(value) => {
            if *value {
                "是".into()
            } else {
                "否".into()
            }
        }
        DataType::Empty | DataType::Error(_) => String::new(),
    }
}

pub(super) fn split_multi(raw: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    raw.split([';', '；', '、', ',', '，', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert((*value).to_string()))
        .map(ToString::to_string)
        .collect()
}

pub(super) fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|value| value.is_ascii_hexdigit())
}

pub(super) fn push_error(
    issues: &mut Vec<ImportIssue>,
    sheet: &str,
    row: usize,
    entity_type: &str,
    name: &str,
    message: &str,
) {
    issues.push(ImportIssue {
        sheet: sheet.into(),
        row,
        entity_type: entity_type.into(),
        name: name.into(),
        action: "阻止导入".into(),
        level: "error".into(),
        message: message.into(),
    });
}

pub(super) fn push_warning(
    issues: &mut Vec<ImportIssue>,
    sheet: &str,
    row: usize,
    entity_type: &str,
    name: &str,
    message: &str,
) {
    issues.push(ImportIssue {
        sheet: sheet.into(),
        row,
        entity_type: entity_type.into(),
        name: name.into(),
        action: "保留原值".into(),
        level: "warning".into(),
        message: message.into(),
    });
}

pub(super) fn check_duplicates<T, F>(
    items: &[T],
    key: F,
    sheet: &str,
    entity_type: &str,
    issues: &mut Vec<ImportIssue>,
) where
    F: Fn(&T) -> (&String, usize),
{
    let mut seen = HashMap::<String, usize>::new();
    for item in items {
        let (name, row) = key(item);
        if let Some(first_row) = seen.insert(name.clone(), row) {
            push_error(
                issues,
                sheet,
                row,
                entity_type,
                name,
                &format!("模板内名称重复，首次出现在第 {first_row} 行"),
            );
        }
    }
}
