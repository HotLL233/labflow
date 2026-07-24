#[cfg(target_os = "windows")]
use {
    std::path::Path,
    windows::core::HSTRING,
    windows::Data::Pdf::PdfDocument,
    windows::Graphics::Imaging::{BitmapDecoder, BitmapEncoder, BitmapPixelFormat, SoftwareBitmap},
    windows::Storage::{
        StorageFile,
        Streams::{DataReader, InMemoryRandomAccessStream},
    },
};

/// 将 PDF 每一页渲染为 PNG，输出到 `out_dir/page_1.png`, `page_2.png` ...
/// 返回页数
#[cfg(target_os = "windows")]
pub fn pdf_to_pngs(pdf_path: &Path, out_dir: &Path) -> Result<u32, String> {
    let pdf_abs = pdf_path
        .canonicalize()
        .map_err(|e| format!("找不到 PDF: {}", e))?;
    let hpath = HSTRING::from(pdf_abs.to_string_lossy().as_ref());

    let file = StorageFile::GetFileFromPathAsync(&hpath)
        .map_err(|e| format!("GetFileFromPathAsync: {}", e))?
        .get()
        .map_err(|e| format!("get file: {}", e))?;

    let stream = file
        .OpenReadAsync()
        .map_err(|e| format!("OpenReadAsync: {}", e))?
        .get()
        .map_err(|e| format!("open read: {}", e))?;

    let pdf = PdfDocument::LoadFromStreamAsync(&stream)
        .map_err(|e| format!("LoadFromStreamAsync: {}", e))?
        .get()
        .map_err(|e| format!("load pdf: {}", e))?;

    let page_count = pdf.PageCount().map_err(|e| format!("PageCount: {}", e))? as u32;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir: {}", e))?;

    for i in 0..page_count {
        let page = pdf
            .GetPage(i)
            .map_err(|e| format!("GetPage({}): {}", i, e))?;

        let mem = InMemoryRandomAccessStream::new().map_err(|e| format!("new stream: {}", e))?;

        page.RenderToStreamAsync(&mem)
            .map_err(|e| format!("RenderToStreamAsync: {}", e))?
            .get()
            .map_err(|e| format!("render page {}: {}", i, e))?;

        // 解码 BMP → PNG
        mem.Seek(0).map_err(|e| format!("seek: {}", e))?;
        let decoder = BitmapDecoder::CreateAsync(&mem)
            .map_err(|e| format!("CreateAsync decoder: {}", e))?
            .get()
            .map_err(|e| format!("get decoder: {}", e))?;

        let bmp = decoder
            .GetSoftwareBitmapAsync()
            .map_err(|e| format!("GetSoftwareBitmapAsync: {}", e))?
            .get()
            .map_err(|e| format!("get bitmap: {}", e))?;

        let rgba = SoftwareBitmap::Convert(&bmp, BitmapPixelFormat::Rgba8)
            .map_err(|e| format!("Convert: {}", e))?;

        // 编码 PNG 到内存流
        let png_mem =
            InMemoryRandomAccessStream::new().map_err(|e| format!("png stream new: {}", e))?;
        let encoder = BitmapEncoder::CreateAsync(
            BitmapEncoder::PngEncoderId().map_err(|e| format!("PngEncoderId: {}", e))?,
            &png_mem,
        )
        .map_err(|e| format!("CreateAsync encoder: {}", e))?
        .get()
        .map_err(|e| format!("get encoder: {}", e))?;
        encoder
            .SetSoftwareBitmap(&rgba)
            .map_err(|e| format!("SetSoftwareBitmap: {}", e))?;
        encoder
            .FlushAsync()
            .map_err(|e| format!("FlushAsync: {}", e))?
            .get()
            .map_err(|e| format!("flush: {}", e))?;

        // 读取 PNG 字节写入文件
        png_mem.Seek(0).map_err(|e| format!("png seek: {}", e))?;
        let png_size = png_mem.Size().map_err(|e| format!("Size: {}", e))? as usize;
        let reader = DataReader::CreateDataReader(&png_mem)
            .map_err(|e| format!("CreateDataReader: {}", e))?;
        reader
            .LoadAsync(png_size as u32)
            .map_err(|e| format!("LoadAsync: {}", e))?
            .get()
            .map_err(|e| format!("load: {}", e))?;
        let mut buf = vec![0u8; png_size];
        reader
            .ReadBytes(&mut buf)
            .map_err(|e| format!("ReadBytes: {}", e))?;

        let out_path = out_dir.join(format!("page_{}.png", i + 1));
        std::fs::write(&out_path, &buf).map_err(|e| format!("write png: {}", e))?;
    }

    Ok(page_count)
}

#[cfg(not(target_os = "windows"))]
use std::path::Path;

#[cfg(not(target_os = "windows"))]
pub fn pdf_to_pngs(_pdf_path: &Path, _out_dir: &Path) -> Result<u32, String> {
    Err("PDF 渲染仅支持 Windows 平台".to_string())
}
