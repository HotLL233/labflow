use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

pub struct DocxResult {
    pub html: String,
    pub toc: Vec<TocItem>,
}

#[derive(Clone, serde::Serialize)]
pub struct TocItem {
    pub id: String,
    pub text: String,
    pub level: u8,
    pub children: Vec<TocItem>,
}

pub struct DocxImageOptions<'a> {
    pub output_dir: &'a Path,
    pub image_url_prefix: &'a str,
}

fn xml_attribute(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .filter_map(|attribute| attribute.ok())
        .find(|attribute| attribute.key.as_ref() == key)
        .and_then(|attribute| String::from_utf8(attribute.value.into_owned()).ok())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn file_name_from_target(target: &str) -> Option<String> {
    Path::new(target)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .map(ToOwned::to_owned)
}

fn read_image_relationships(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<HashMap<String, String>, String> {
    let mut relationships = HashMap::new();
    let Ok(mut file) = archive.by_name("word/_rels/document.xml.rels") else {
        return Ok(relationships);
    };
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|error| format!("读取 Word 图片关系失败: {error}"))?;

    let mut reader = Reader::from_reader(BufReader::new(&data[..]));
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if element.name().as_ref() == b"Relationship" =>
            {
                let relationship_type = xml_attribute(&element, b"Type").unwrap_or_default();
                if !relationship_type.ends_with("/image") {
                    buffer.clear();
                    continue;
                }
                if let (Some(id), Some(target)) = (
                    xml_attribute(&element, b"Id"),
                    xml_attribute(&element, b"Target"),
                ) {
                    let path = if target.starts_with("/") {
                        target.trim_start_matches('/').to_string()
                    } else {
                        format!("word/{target}")
                    };
                    relationships.insert(id, path);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("解析 Word 图片关系失败: {error}")),
            _ => {}
        }
        buffer.clear();
    }
    Ok(relationships)
}

fn extract_images(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    relationships: &HashMap<String, String>,
    output_dir: &Path,
) -> Result<HashMap<String, String>, String> {
    if relationships.is_empty() {
        return Ok(HashMap::new());
    }
    fs::create_dir_all(output_dir).map_err(|error| format!("创建教程图片目录失败: {error}"))?;

    let mut images = HashMap::new();
    for (relationship_id, archive_path) in relationships {
        let Some(file_name) = file_name_from_target(archive_path) else {
            continue;
        };
        let Ok(mut image_file) = archive.by_name(archive_path) else {
            continue;
        };
        let mut image_data = Vec::new();
        image_file
            .read_to_end(&mut image_data)
            .map_err(|error| format!("读取教程图片失败: {error}"))?;
        if image_data.is_empty() {
            continue;
        }
        fs::write(output_dir.join(&file_name), image_data)
            .map_err(|error| format!("保存教程图片失败: {error}"))?;
        images.insert(relationship_id.clone(), file_name);
    }
    Ok(images)
}

fn image_html(
    relationship_id: Option<String>,
    images: &HashMap<String, String>,
    prefix: &str,
) -> Option<String> {
    let relationship_id = relationship_id?;
    let filename = images.get(&relationship_id)?;
    Some(format!(
        "<figure class=\"help-article-image\"><img data-help-image=\"{}/{}\" alt=\"教程图片\" /><figcaption>教程图片</figcaption></figure>",
        prefix.trim_end_matches('/'),
        filename,
    ))
}

pub fn parse_docx(data: &[u8], images: DocxImageOptions<'_>) -> Result<DocxResult, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data))
        .map_err(|error| format!("无法打开 DOCX 文件: {error}"))?;
    let image_relationships = read_image_relationships(&mut archive)?;
    let extracted_images = extract_images(&mut archive, &image_relationships, images.output_dir)?;

    let mut heading_styles = Vec::new();
    if let Ok(mut styles_file) = archive.by_name("word/styles.xml") {
        let mut styles = Vec::new();
        styles_file
            .read_to_end(&mut styles)
            .map_err(|error| format!("读取 styles 文件失败: {error}"))?;
        let mut reader = Reader::from_reader(BufReader::new(&styles[..]));
        let mut buffer = Vec::new();
        let mut in_style = false;
        let mut style_id = String::new();
        let mut style_name = String::new();
        loop {
            match reader.read_event_into(&mut buffer) {
                Ok(Event::Start(element)) => {
                    let tag = String::from_utf8_lossy(element.name().as_ref()).to_lowercase();
                    if tag == "w:style" {
                        in_style = true;
                        style_id = xml_attribute(&element, b"w:styleId").unwrap_or_default();
                        style_name.clear();
                    } else if in_style && tag == "w:name" {
                        style_name = xml_attribute(&element, b"w:val").unwrap_or_default();
                    }
                }
                Ok(Event::End(element)) if element.name().as_ref() == b"w:style" => {
                    if style_name.to_lowercase().starts_with("heading")
                        || style_name.contains("标题")
                    {
                        heading_styles.push(style_id.clone());
                    }
                    in_style = false;
                }
                Ok(Event::Eof) => break,
                Err(error) => return Err(format!("解析 styles 文件失败: {error}")),
                _ => {}
            }
            buffer.clear();
        }
    }

    let mut document_file = archive
        .by_name("word/document.xml")
        .map_err(|error| format!("找不到 document.xml: {error}"))?;
    let mut document = Vec::new();
    document_file
        .read_to_end(&mut document)
        .map_err(|error| format!("读取 document 文件失败: {error}"))?;

    let mut reader = Reader::from_reader(BufReader::new(&document[..]));
    let mut buffer = Vec::new();
    let mut html = String::from("<div>");
    let mut toc = Vec::new();
    let mut paragraph_html = String::new();
    let mut current_style: Option<String> = None;
    let mut heading_level = None;
    let mut run_text = String::new();
    let mut in_run = false;
    let mut run_bold = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let tag = String::from_utf8_lossy(element.name().as_ref()).to_lowercase();
                match tag.as_str() {
                    "w:p" => {
                        paragraph_html.clear();
                        current_style = None;
                        heading_level = None;
                    }
                    "w:pstyle" => {
                        current_style = xml_attribute(&element, b"w:val");
                        if let Some(style_id) = current_style.as_deref() {
                            if heading_styles.iter().any(|style| style == style_id) {
                                heading_level = Some(
                                    style_id
                                        .chars()
                                        .filter(char::is_ascii_digit)
                                        .collect::<String>()
                                        .parse::<u8>()
                                        .unwrap_or(1),
                                );
                            }
                        }
                    }
                    "w:r" => {
                        in_run = true;
                        run_text.clear();
                        run_bold = false;
                    }
                    "w:b" => run_bold = true,
                    "a:blip" => {
                        if let Some(image) = image_html(
                            xml_attribute(&element, b"r:embed"),
                            &extracted_images,
                            images.image_url_prefix,
                        ) {
                            paragraph_html.push_str(&image);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(element)) => {
                let tag = String::from_utf8_lossy(element.name().as_ref()).to_lowercase();
                if tag == "a:blip" {
                    if let Some(image) = image_html(
                        xml_attribute(&element, b"r:embed"),
                        &extracted_images,
                        images.image_url_prefix,
                    ) {
                        paragraph_html.push_str(&image);
                    }
                } else if tag == "w:tab" && in_run {
                    run_text.push_str("&emsp;");
                } else if tag == "w:br" && in_run {
                    run_text.push_str("<br />");
                }
            }
            Ok(Event::End(element)) => {
                let tag = String::from_utf8_lossy(element.name().as_ref()).to_lowercase();
                match tag.as_str() {
                    "w:p" => {
                        if !paragraph_html.is_empty() {
                            if let Some(level) = heading_level {
                                let id = format!("h_{}", toc.len() + 1);
                                let text = strip_html(&paragraph_html);
                                html.push_str(&format!(
                                    "<h{level} id=\"{id}\">{paragraph_html}</h{level}>"
                                ));
                                toc.push(TocItem {
                                    id,
                                    text,
                                    level,
                                    children: vec![],
                                });
                            } else {
                                html.push_str(&format!("<p>{paragraph_html}</p>"));
                            }
                        }
                    }
                    "w:r" => {
                        in_run = false;
                        if run_bold {
                            paragraph_html.push_str(&format!("<b>{run_text}</b>"));
                        } else {
                            paragraph_html.push_str(&run_text);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) if in_run => {
                run_text.push_str(&escape_html(&text.unescape().unwrap_or_default()));
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("解析 Word 文档失败: {error}")),
            _ => {}
        }
        buffer.clear();
    }

    html.push_str("</div>");
    Ok(DocxResult { html, toc })
}

fn strip_html(value: &str) -> String {
    let mut plain = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(character),
            _ => {}
        }
    }
    plain.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn extracts_embedded_images_and_writes_preview_markup() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(br#"<w:document xmlns:w="w" xmlns:a="a" xmlns:r="r"><w:body><w:p><w:r><w:t>Step</w:t></w:r><w:r><a:blip r:embed="rId7"/></w:r></w:p></w:body></w:document>"#).unwrap();
        writer
            .start_file("word/_rels/document.xml.rels", options)
            .unwrap();
        writer.write_all(br#"<Relationships><Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png" /></Relationships>"#).unwrap();
        writer.start_file("word/media/image1.png", options).unwrap();
        writer.write_all(b"image-bytes").unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let output_dir =
            std::env::temp_dir().join(format!("workload-docx-image-test-{}", uuid::Uuid::new_v4()));
        let result = parse_docx(
            &bytes,
            DocxImageOptions {
                output_dir: &output_dir,
                image_url_prefix: "/api/help-documents/7/images",
            },
        )
        .unwrap();

        assert!(output_dir.join("image1.png").exists());
        assert!(result
            .html
            .contains("data-help-image=\"/api/help-documents/7/images/image1.png\""));
        assert!(result.html.contains("Step"));
        let _ = fs::remove_dir_all(output_dir);
    }
}
