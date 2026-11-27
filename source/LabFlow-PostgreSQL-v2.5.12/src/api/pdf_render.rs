use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn renderer_path() -> PathBuf {
    if let Ok(path) = std::env::var("WORKLOAD_PDFTOPPM_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("pdf-runtime").join("pdftoppm.exe"))
        })
        .unwrap_or_else(|| PathBuf::from("pdf-runtime").join("pdftoppm.exe"))
}

fn pdfinfo_path() -> PathBuf {
    if let Ok(path) = std::env::var("WORKLOAD_PDFINFO_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("pdf-runtime").join("pdfinfo.exe"))
        })
        .unwrap_or_else(|| PathBuf::from("pdf-runtime").join("pdfinfo.exe"))
}

/// Reads the PDF page count without rendering the document.
pub fn pdf_page_count(pdf_path: &Path) -> Result<u32, String> {
    let info = pdfinfo_path();
    if !info.is_file() {
        return Err(format!("未找到 PDF 信息组件：{}", info.display()));
    }
    let output = Command::new(&info)
        .arg(pdf_path)
        .output()
        .map_err(|error| format!("启动 PDF 信息组件失败: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            "PDF 信息组件未能读取页数".into()
        } else {
            format!("读取 PDF 页数失败: {detail}")
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| {
            let (label, value) = line.split_once(':')?;
            if label.trim().eq_ignore_ascii_case("Pages") {
                value.trim().parse::<u32>().ok()
            } else {
                None
            }
        })
        .filter(|count| *count > 0)
        .ok_or_else(|| "PDF 信息中未找到有效页数".into())
}

fn render_pdf_pages(
    pdf_path: &Path,
    out_dir: &Path,
    first_page: Option<u32>,
    last_page: Option<u32>,
) -> Result<u32, String> {
    let renderer = renderer_path();
    if !renderer.is_file() {
        return Err(format!("未找到 PDF 预览组件：{}", renderer.display()));
    }
    std::fs::create_dir_all(out_dir).map_err(|error| format!("创建预览目录失败: {error}"))?;
    let prefix = out_dir.join("render");
    let mut command = Command::new(&renderer);
    command.args(["-png", "-r", "120"]);
    if let Some(first_page) = first_page {
        command.arg("-f").arg(first_page.to_string());
    }
    if let Some(last_page) = last_page {
        command.arg("-l").arg(last_page.to_string());
    }
    let output = command
        .arg(pdf_path)
        .arg(&prefix)
        .output()
        .map_err(|error| format!("启动 PDF 预览组件失败: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            "PDF 预览组件未能生成图片".into()
        } else {
            format!("PDF 预览组件转换失败: {detail}")
        });
    }
    let mut pages = std::fs::read_dir(out_dir)
        .map_err(|error| format!("读取预览目录失败: {error}"))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let index = name
                .strip_prefix("render-")?
                .strip_suffix(".png")?
                .parse::<u32>()
                .ok()?;
            Some((index, path))
        })
        .collect::<Vec<_>>();
    pages.sort_by_key(|(index, _)| *index);
    for (index, path) in &pages {
        std::fs::rename(path, out_dir.join(format!("page_{index}.png")))
            .map_err(|error| format!("整理预览页失败: {error}"))?;
    }
    Ok(pages.len() as u32)
}

/// Uses Poppler to render every PDF page in one process. Unlike Windows WinRT
/// PdfDocument, this works from the scheduled background server and does not
/// depend on a logged-in desktop session.
pub fn pdf_to_pngs(pdf_path: &Path, out_dir: &Path) -> Result<u32, String> {
    render_pdf_pages(pdf_path, out_dir, None, None)
}

/// Renders from a specific page onward. This lets attachment previews publish
/// the first page before expensive remaining pages are generated.
pub fn pdf_to_pngs_from_page(
    pdf_path: &Path,
    out_dir: &Path,
    first_page: u32,
) -> Result<u32, String> {
    if first_page == 0 {
        return Err("预览页码必须从 1 开始".into());
    }
    render_pdf_pages(pdf_path, out_dir, Some(first_page), None)
}

/// Renders just one page for the first-screen attachment preview.
pub fn pdf_page_to_png(pdf_path: &Path, out_dir: &Path, page: u32) -> Result<u32, String> {
    if page == 0 {
        return Err("预览页码必须从 1 开始".into());
    }
    render_pdf_pages(pdf_path, out_dir, Some(page), Some(page))
}

#[cfg(test)]
mod tests {
    use super::{pdf_page_count, pdf_page_to_png, pdf_to_pngs_from_page, renderer_path};

    #[test]
    fn renderer_path_allows_a_deployment_override() {
        std::env::set_var("WORKLOAD_PDFTOPPM_PATH", r"C:\preview\pdftoppm.exe");
        assert_eq!(
            renderer_path().to_string_lossy(),
            r"C:\preview\pdftoppm.exe"
        );
        std::env::remove_var("WORKLOAD_PDFTOPPM_PATH");
    }

    #[test]
    fn renders_first_page_before_remaining_pages_when_a_fixture_is_supplied() {
        let Ok(pdf_path) = std::env::var("WORKLOAD_PDF_RENDER_TEST_FILE") else {
            return;
        };
        let root = std::env::temp_dir().join(format!(
            "workload-pdf-first-page-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(
            pdf_page_to_png(std::path::Path::new(&pdf_path), &root, 1).unwrap(),
            1
        );
        assert!(root.join("page_1.png").is_file());
        let remaining = pdf_to_pngs_from_page(std::path::Path::new(&pdf_path), &root, 2).unwrap();
        assert!(remaining > 0);
        assert!(root.join("page_2.png").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reads_pdf_page_count_when_a_fixture_is_supplied() {
        let Ok(pdf_path) = std::env::var("WORKLOAD_PDF_RENDER_TEST_FILE") else {
            return;
        };
        let count = pdf_page_count(std::path::Path::new(&pdf_path)).unwrap();
        assert!(count > 0);
    }
}
