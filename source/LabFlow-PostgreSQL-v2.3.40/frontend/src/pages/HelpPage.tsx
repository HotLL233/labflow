import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Alert, Box, Button, CircularProgress, Divider, Paper, Typography } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import DownloadIcon from '@mui/icons-material/Download';
import GroupsOutlinedIcon from '@mui/icons-material/GroupsOutlined';
import MenuBookOutlinedIcon from '@mui/icons-material/MenuBookOutlined';
import { useNavigate } from 'react-router-dom';
import { downloadFile, getHelpArticleImageBlob, getHelpArticles, getHelpAttachments, getHelpDocumentFileBlob, getHelpDocumentPageBlob, getHelpDocuments } from '../api/client';
import type { HelpArticle, HelpAttachment, HelpDocument, TocItem } from '../types';

const cleanHtml = (html: string) => html
  .replace(/<\/?(script|style|iframe|object|embed)[^>]*>/gi, '')
  .replace(/\son\w+\s*=\s*(['"]).*?\1/gi, '')
  .replace(/javascript:/gi, '');

const parseToc = (article: HelpArticle | null): TocItem[] => {
  if (!article?.toc_json) return [];
  try {
    const value = JSON.parse(article.toc_json);
    return Array.isArray(value) ? value : [];
  } catch {
    return [];
  }
};

const flattenToc = (items: TocItem[]): TocItem[] => items.flatMap(item => [item, ...flattenToc(item.children || [])]);

const HelpPage: React.FC = () => {
  const navigate = useNavigate();
  const [documents, setDocuments] = useState<HelpDocument[]>([]);
  const [selected, setSelected] = useState<HelpDocument | null>(null);
  const [articles, setArticles] = useState<HelpArticle[]>([]);
  const [attachments, setAttachments] = useState<HelpAttachment[]>([]);
  const [page, setPage] = useState(1);
  const [pageInput, setPageInput] = useState('1');
  const [zoom, setZoom] = useState(1);
  const [fitMode, setFitMode] = useState<'page' | 'width'>('width');
  const [viewMode, setViewMode] = useState<'source' | 'article'>('source');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [previewUrl, setPreviewUrl] = useState('');
  const [pageUrls, setPageUrls] = useState<Record<number, string>>({});
  const articleBodyRef = useRef<HTMLDivElement>(null);
  const pageRefs = useRef<Record<number, HTMLDivElement | null>>({});

  useEffect(() => {
    Promise.all([getHelpDocuments(true), getHelpAttachments(true), getHelpArticles(true)])
      .then(([docs, atts, articleResult]) => {
        if (docs.code !== 0) throw new Error(docs.message || '教程加载失败');
        setDocuments(docs.data || []);
        setAttachments(atts.data || []);
        setArticles(articleResult.code === 0 ? articleResult.data || [] : []);
      })
      .catch(loadError => setError(loadError?.message || '教程加载失败'))
      .finally(() => setLoading(false));
  }, []);

  const fallbackArticle = useMemo(() => {
    if (!selected) return null;
    return articles.find(item => item.title === selected.title || (item.source_file || '').includes(selected.filename)) || null;
  }, [articles, selected]);
  const toc = useMemo(() => parseToc(fallbackArticle), [fallbackArticle]);
  const ext = selected?.file_type.toLowerCase() || '';
  const pages = selected?.page_count || 0;
  const supportsInlineFile = ext === 'pdf' || ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp'].includes(ext);

  useEffect(() => {
    setPage(1);
    setPageInput('1');
    setZoom(1);
    setFitMode('width');
    setViewMode('source');
    setPreviewUrl('');
    setPageUrls({});
    pageRefs.current = {};
  }, [selected]);

  useEffect(() => { setPageInput(String(page)); }, [page]);

  useEffect(() => {
    let cancelled = false;
    const objectUrls: string[] = [];
    if (!selected || viewMode === 'article') return;
    const load = async () => {
      try {
        if (pages > 0) {
          const entries = await Promise.all(Array.from({ length: pages }, async (_, index) => {
            const url = URL.createObjectURL(await getHelpDocumentPageBlob(selected.id, index + 1));
            objectUrls.push(url);
            return [index + 1, url] as const;
          }));
          if (!cancelled) setPageUrls(Object.fromEntries(entries));
        } else if (supportsInlineFile) {
          const url = URL.createObjectURL(await getHelpDocumentFileBlob(selected.id));
          objectUrls.push(url);
          if (!cancelled) setPreviewUrl(url);
        }
      } catch {
        if (!cancelled) setError('教程预览加载失败');
      }
    };
    void load();
    return () => { cancelled = true; objectUrls.forEach(URL.revokeObjectURL); };
  }, [pages, selected, supportsInlineFile, viewMode]);

  useEffect(() => {
    let cancelled = false;
    const urls: string[] = [];
    const loadArticleImages = async () => {
      const container = articleBodyRef.current;
      if (!container || !fallbackArticle || viewMode !== 'article') return;
      const images = Array.from(container.querySelectorAll<HTMLImageElement>('img[data-help-image]'));
      await Promise.all(images.map(async image => {
        const source = image.dataset.helpImage;
        if (!source) return;
        try {
          const blob = await getHelpArticleImageBlob(source);
          if (cancelled) return;
          const url = URL.createObjectURL(blob);
          urls.push(url);
          image.src = url;
        } catch {
          image.alt = '教程图片加载失败';
        }
      }));
    };
    void loadArticleImages();
    return () => { cancelled = true; urls.forEach(URL.revokeObjectURL); };
  }, [fallbackArticle, viewMode]);

  const openTocItem = (item: TocItem) => {
    if (!fallbackArticle) return;
    setViewMode('article');
    window.setTimeout(() => document.getElementById(item.id)?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 0);
  };

  const applyPage = () => {
    const next = Math.max(1, Math.min(pages || 1, Number.parseInt(pageInput, 10) || 1));
    setPage(next);
    window.setTimeout(() => pageRefs.current[next]?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 0);
  };

  const resetView = (mode: 'page' | 'width') => { setFitMode(mode); setZoom(1); };

  if (loading) return <Box sx={{ minHeight: 320, display: 'grid', placeItems: 'center' }}><CircularProgress /></Box>;

  if (selected) {
    const tocItems = flattenToc(toc);
    return <Box sx={{ maxWidth: 1080, mx: 'auto' }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1.5, flexWrap: 'wrap' }}>
        <Button startIcon={<ArrowBackIcon />} onClick={() => setSelected(null)}>返回教程列表</Button>
        <Typography variant="body2" color="text.secondary" sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{selected.title}</Typography>
      </Box>
      {error && <Alert severity="error" sx={{ mb: 1.5 }} onClose={() => setError('')}>{error}</Alert>}
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: '190px minmax(0, 1fr)' }, gap: 1.5, alignItems: 'start' }}>
        <Paper variant="outlined" sx={{ p: 1.25, position: { md: 'sticky' }, top: 80, maxHeight: { md: 'calc(100vh - 110px)' }, overflowY: 'auto' }}>
          <Typography variant="subtitle2" fontWeight={800} sx={{ px: 0.75, mb: 0.75 }}>文档目录</Typography>
          {tocItems.length === 0 ? <Typography variant="caption" color="text.secondary" sx={{ px: 0.75 }}>该文档未提供目录</Typography> : tocItems.map(item => <Button key={item.id} size="small" fullWidth onClick={() => openTocItem(item)} sx={{ justifyContent: 'flex-start', textAlign: 'left', py: 0.45, pl: Math.min(2.5, 0.75 + item.level * 0.45), fontSize: '0.78rem', lineHeight: 1.35 }}>{item.text}</Button>)}
        </Paper>
        <Paper variant="outlined" sx={{ p: { xs: 1.25, md: 2 }, minWidth: 0 }}>
          <Box sx={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 1, mb: 1.25 }}>
            <Box sx={{ minWidth: 0 }}><Typography variant="h6" fontWeight={800} noWrap>{selected.title}</Typography><Typography variant="caption" color="text.secondary">{selected.filename}</Typography></Box>
            {fallbackArticle && <Button size="small" variant="outlined" onClick={() => setViewMode(mode => mode === 'source' ? 'article' : 'source')}>{viewMode === 'source' ? '目录内容' : '原格式预览'}</Button>}
          </Box>
          {viewMode === 'article' && fallbackArticle ? <Box ref={articleBodyRef} sx={{ maxHeight: 'calc(100vh - 220px)', overflow: 'auto', px: { xs: 0.5, md: 1 }, '& img': { maxWidth: '100%', height: 'auto' }, '& table': { width: '100%', borderCollapse: 'collapse', mb: 2 }, '& td, & th': { border: '1px solid #d9dee7', p: 1 }, '& p': { lineHeight: 1.75 }, '& h1,h2,h3': { mt: 2.5, scrollMarginTop: 12 } }} dangerouslySetInnerHTML={{ __html: cleanHtml(fallbackArticle.content_html) }} /> : pages > 0 ? <>
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 0.75, flexWrap: 'wrap', mb: 0.75 }}>
              <Box sx={{ display: 'flex', gap: 0.5 }}><Button size="small" variant={fitMode === 'page' ? 'contained' : 'outlined'} onClick={() => resetView('page')}>适合页面</Button><Button size="small" variant={fitMode === 'width' ? 'contained' : 'outlined'} onClick={() => resetView('width')}>适合宽度</Button></Box>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}><Button size="small" onClick={() => setZoom(value => Math.max(0.5, Number((value - 0.1).toFixed(1))))}>缩小</Button><Typography variant="caption" sx={{ minWidth: 42, textAlign: 'center' }}>{Math.round(zoom * 100)}%</Typography><Button size="small" onClick={() => setZoom(value => Math.min(2, Number((value + 0.1).toFixed(1))))}>放大</Button></Box>
            </Box>
            <Box sx={{ bgcolor: '#eef1f5', p: { xs: 0.5, md: 1 }, maxHeight: 'calc(100vh - 220px)', overflowY: 'auto', overflowX: 'hidden' }}><Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 1.25 }}>{Array.from({ length: pages }, (_, index) => { const pageNo = index + 1; const url = pageUrls[pageNo]; return <Box key={pageNo} ref={(node: HTMLDivElement | null) => { pageRefs.current[pageNo] = node; }} sx={{ width: '100%', display: 'flex', justifyContent: 'center', scrollMarginTop: 12 }}><Box sx={{ position: 'relative', width: fitMode === 'width' ? `${Math.round(zoom * 100)}%` : 'auto', maxWidth: '100%', bgcolor: '#fff', boxShadow: '0 1px 4px rgba(0,0,0,.18)' }}>{url ? <img src={url} alt={`${selected.title} 第${pageNo}页`} style={{ display: 'block', width: '100%', maxHeight: fitMode === 'page' ? '78vh' : 'none', objectFit: 'contain' }} /> : <Box sx={{ height: 180, display: 'grid', placeItems: 'center' }}><CircularProgress size={22} /></Box>}<Typography variant="caption" sx={{ position: 'absolute', right: 6, bottom: 4, px: 0.5, bgcolor: 'rgba(255,255,255,.8)' }}>第 {pageNo} 页</Typography></Box></Box>; })}</Box></Box>
            <Box component="form" onSubmit={event => { event.preventDefault(); applyPage(); }} sx={{ mt: 0.75, display: 'flex', justifyContent: 'center', gap: 0.5, alignItems: 'center' }}><Typography variant="caption">滚动阅读 · 定位第</Typography><input aria-label="页码" value={pageInput} onChange={event => setPageInput(event.target.value.replace(/\D/g, ''))} onBlur={applyPage} style={{ width: 42, textAlign: 'center', height: 28, border: '1px solid #c8ced8', borderRadius: 3 }} /><Typography variant="caption">/ {pages} 页</Typography></Box>
          </> : supportsInlineFile ? <Box sx={{ height: 'min(68vh, 760px)', textAlign: 'center' }}>{previewUrl && <iframe title={selected.title} src={previewUrl} style={{ width: '100%', height: '100%', border: 0 }} />}</Box> : <Alert severity="warning">教程在线预览尚未生成，请联系管理员检查预览组件。</Alert>}
          <Divider sx={{ my: 2 }} />
          <Typography variant="subtitle2" fontWeight={800} gutterBottom>教程附件</Typography>
          {attachments.length === 0 ? <Typography variant="body2" color="text.secondary">暂无附件</Typography> : attachments.map(item => <Box key={item.id} sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1, py: 0.75, borderBottom: '1px solid #eee' }}><Box sx={{ minWidth: 0 }}><Typography variant="body2" noWrap>{item.title}</Typography><Typography variant="caption" color="text.secondary">{item.filename} · {(item.file_size / 1024).toFixed(1)} KB</Typography></Box><Button size="small" startIcon={<DownloadIcon />} onClick={() => void downloadFile(`/api/help-attachments/${item.id}/file`, {}, item.filename)}>下载</Button></Box>)}
        </Paper>
      </Box>
    </Box>;
  }

  return <Box>
    <Box sx={{ mb: 2.5 }}><Typography variant="h4" sx={{ fontWeight: 700, fontSize: { xs: 26, md: 30 } }}>帮助与反馈</Typography><Typography color="text.secondary" sx={{ mt: 0.5 }}>查阅上传的教程文档和相关附件。</Typography></Box>
    {error && <Alert severity="error" sx={{ mb: 2 }} onClose={() => setError('')}>{error}</Alert>}
    <Box sx={{ display: 'flex', justifyContent: 'flex-end', mb: 2 }}><Button variant="outlined" startIcon={<GroupsOutlinedIcon />} onClick={() => navigate('/personnel-change')}>人员变动通知</Button></Box>
    {documents.length === 0 ? <Alert severity="info">暂无可查看的教程文档。</Alert> : <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(240px, 1fr))', gap: 1.5 }}>{documents.map(document => <Paper key={document.id} variant="outlined" onClick={() => setSelected(document)} sx={{ p: 2, minHeight: 125, cursor: 'pointer', transition: 'border-color .15s, background-color .15s', '&:hover': { borderColor: 'primary.main', bgcolor: '#f7fbff' } }}><MenuBookOutlinedIcon color="primary" /><Typography fontWeight={800} sx={{ mt: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{document.title}</Typography><Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{document.filename}</Typography><Typography variant="caption" color="text.secondary">{document.file_type.toUpperCase()} · {document.page_count ? `${document.page_count} 页` : '在线预览'}</Typography></Paper>)}</Box>}
  </Box>;
};

export default HelpPage;
