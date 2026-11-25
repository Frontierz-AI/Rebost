//! Xberg integration — the single document-extraction layer.
//!
//! Everything here is deterministic and fully local: native text first,
//! vendored-tesseract OCR when a file or page has no usable text layer
//! (whatever `*.traineddata` packs are in the tessdata directory).

use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use xberg::core::config::{LanguageDetectionConfig, OutputFormat, PageConfig};
use xberg::{
    ExtractInput, ExtractionConfig, KeywordAlgorithm, KeywordConfig, OcrConfig, OcrStrategy,
};

use crate::search::normalize_lang;
use crate::types::OutlineEntry;

/// The formats Rebost accepts, intersected with Xberg's format registry at
/// startup (the registry is the source of truth for what the build handles).
const REBOST_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "md", "markdown",
    "txt", "rtf", "csv", "tsv", "eml", "msg", "epub", "html", "htm", "xhtml",
];

/// Web pages. They get their own cleanup (see [`html_options`]) and their
/// `<html lang>` and first `<h1>` count for the Card.
pub(crate) const HTML_EXTENSIONS: &[&str] = &["html", "htm", "xhtml"];

pub(crate) fn is_html_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| HTML_EXTENSIONS.iter().any(|h| e.eq_ignore_ascii_case(h)))
}

/// Extensions Rebost offers, intersected with Xberg's registry.
pub fn supported_extensions() -> &'static HashSet<String> {
    static SUPPORTED: OnceLock<HashSet<String>> = OnceLock::new();
    SUPPORTED.get_or_init(|| {
        let registry: HashSet<String> = xberg::core::mime::list_supported_formats()
            .into_iter()
            .map(|f| f.extension.to_lowercase())
            .collect();
        REBOST_EXTENSIONS
            .iter()
            .filter(|e| registry.contains(**e))
            .map(|e| e.to_string())
            .collect()
    })
}

pub(crate) const EXTRACTED_MAX: usize = 1_000_000;

pub(crate) fn limit_extracted(text: String) -> String {
    limit_extracted_to(text, EXTRACTED_MAX)
}

fn limit_extracted_to(text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = &text[..end];
    if let Some(at) = prefix.rfind("\n\n") {
        if at >= max / 2 {
            return prefix[..at].to_string();
        }
    }
    if let Some(at) = prefix.rfind('\n') {
        if at >= max / 2 {
            return prefix[..at].to_string();
        }
    }
    prefix.to_string()
}

fn limit_blocks(blocks: Vec<Block>) -> Vec<Block> {
    let mut used = 0usize;
    let mut out = Vec::new();
    for mut block in blocks {
        if used >= EXTRACTED_MAX {
            break;
        }
        let n = block.text.len();
        if used + n > EXTRACTED_MAX {
            let remain = EXTRACTED_MAX - used;
            if remain < 32 && !out.is_empty() {
                break;
            }
            block.text = limit_extracted_to(block.text, remain);
        }
        used += block.text.len();
        out.push(block);
    }
    out
}

/// True for names that should never be read: hidden files, Word/Excel lock
/// files (`~$…`), `*.tmp`, and the Shelf config file.
pub fn skip_file_name(name: &str) -> bool {
    if name.starts_with('.') || name.starts_with("~$") || name.eq_ignore_ascii_case("shelf.yml") {
        return true;
    }
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tmp"))
}

pub fn is_supported_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if skip_file_name(name) {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| supported_extensions().contains(&e.to_lowercase()))
        .unwrap_or(false)
}

/// What the rest of the pipeline consumes.
#[derive(Debug, Clone)]
pub struct Extraction {
    /// Markdown-ish full text — also the extracted-text cache content.
    pub content: String,
    pub title: String,
    /// Normalized 2-letter language code, when detected.
    pub language: Option<&'static str>,
    /// Raw detected tag for the Card (e.g. "es").
    pub language_tag: Option<String>,
    pub summary: String,
    pub keywords: Vec<String>,
    pub outline: Vec<OutlineEntry>,
    /// Structure-aware building blocks for passages.
    pub blocks: Vec<Block>,
    pub page_count: Option<u32>,
    /// True when any of the text came from local OCR.
    pub ocr_used: bool,
}

/// One structural block, in document order.
#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    pub text: String,
    pub page: Option<u32>,
    pub page_end: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading { level: u8 },
    Paragraph,
    Table,
    SheetStart { name: String },
    SlideStart { number: u32, title: Option<String> },
}

#[derive(Debug, Clone)]
pub struct ExtractorSettings {
    /// Directory holding `*.traineddata` packs used at extract time.
    pub tessdata_dir: Option<PathBuf>,
    /// Bundled packs copied into `tessdata_dir` on the first extract.
    pub tessdata_bundle: Option<PathBuf>,
    /// Extraction timeout per file. Long PDFs get more, see [`timeout_for`].
    pub timeout_secs: u64,
}

/// Time allowed per PDF page, enough for a scanned page through OCR in
/// every bundled language while other files are being read too.
const SECS_PER_PDF_PAGE: u64 = 20;

/// Upper bound for one file, however many pages it has.
const MAX_TIMEOUT_SECS: u64 = 4 * 60 * 60;

/// Extraction timeout for a file: the base, or a per-page budget for long
/// PDFs so a big scan isn't cut off half way through OCR.
pub(crate) fn timeout_for(base_secs: u64, pdf_pages: Option<usize>) -> u64 {
    let by_pages = (pdf_pages.unwrap_or(0) as u64).saturating_mul(SECS_PER_PDF_PAGE);
    base_secs.max(by_pages.min(MAX_TIMEOUT_SECS))
}

/// Page count of a PDF, read without rendering. `None` for other formats or
/// a PDF that won't open (encrypted, damaged); those keep the base timeout.
fn pdf_page_count(path: &Path) -> Option<usize> {
    let is_pdf = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if !is_pdf {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    xberg::pdf_page_count(&bytes, None).ok()
}

impl Default for ExtractorSettings {
    fn default() -> Self {
        Self {
            tessdata_dir: None,
            tessdata_bundle: None,
            timeout_secs: 300,
        }
    }
}

/// Tesseract language codes from `*.traineddata` in `dir` (`osd` skipped).
/// English is listed first when present so mixed-script pages have a fallback.
pub(crate) fn ocr_languages(dir: Option<&Path>) -> Vec<String> {
    let mut langs = Vec::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                let Some(code) = name.strip_suffix(".traineddata") else {
                    continue;
                };
                if code.is_empty() || code == "osd" {
                    continue;
                }
                langs.push(code.to_string());
            }
        }
    }
    langs.sort();
    if let Some(index) = langs.iter().position(|l| l == "eng") {
        langs.remove(index);
        langs.insert(0, "eng".into());
    } else if langs.is_empty() {
        langs.push("eng".into());
    }
    langs
}

static TESSDATA_COPY: Mutex<()> = Mutex::new(());

/// True when `dir` already has at least one `*.traineddata` pack.
pub(crate) fn tessdata_has_packs(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .map(|name| name.ends_with(".traineddata") && entry.path().is_file())
            .unwrap_or(false)
    })
}

fn copy_traineddata(bundle: &Path, dest: &Path) {
    if let Err(error) = std::fs::create_dir_all(dest) {
        log::warn!("create tessdata dir: {error}");
        return;
    }
    let Ok(entries) = std::fs::read_dir(bundle) else {
        return;
    };
    for entry in entries.flatten() {
        let source = entry.path();
        let Some(name) = source.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !(name.ends_with(".traineddata") || name == "LICENSE") {
            continue;
        }
        let dest_file = dest.join(name);
        if dest_file.exists() {
            continue;
        }
        if let Err(error) = std::fs::copy(&source, &dest_file) {
            log::warn!("copying {name}: {error}");
        }
    }
}

fn ensure_tessdata(settings: &ExtractorSettings) {
    let Some(dest) = settings.tessdata_dir.as_ref() else {
        return;
    };
    if tessdata_has_packs(dest) {
        return;
    }
    let Some(bundle) = settings.tessdata_bundle.as_ref() else {
        return;
    };
    let _guard = TESSDATA_COPY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if tessdata_has_packs(dest) {
        return;
    }
    log::info!("copying OCR language packs for first extract");
    copy_traineddata(bundle, dest);
}

fn build_config(settings: &ExtractorSettings, timeout_secs: u64) -> ExtractionConfig {
    ExtractionConfig {
        // Rebost keeps its own extracted-text cache keyed by content hash.
        use_cache: false,
        output_format: OutputFormat::Markdown,
        pages: Some(PageConfig {
            extract_pages: true,
            ..Default::default()
        }),
        include_document_structure: true,
        language_detection: Some(LanguageDetectionConfig::default()),
        // Keywords and summary run post-extraction with the detected
        // language, so stopwords match the document.
        keywords: None,
        ocr: Some(OcrConfig {
            language: ocr_languages(settings.tessdata_dir.as_deref()),
            tessdata_path: settings.tessdata_dir.clone(),
            ..Default::default()
        }),
        // OCR only the pages that lack a usable text layer.
        ocr_strategy: OcrStrategy::Auto,
        extraction_timeout_secs: Some(timeout_secs),
        ..Default::default()
    }
}

/// Elements dropped whole from a web page: embedded frames and objects,
/// and cookie or consent banners. The preset only drops elements whose
/// class or id is exactly `cookie`, `consent`, … while real banners use
/// compound names (`cookie-banner`, `onetrust-consent-sdk`). Kept specific so
/// a page about, say, an informed-consent form keeps its text.
const HTML_EXCLUDE: &[&str] = &[
    "iframe",
    "frame",
    "object",
    "embed",
    "template",
    "[id*=cookie]",
    "[id*=Cookie]",
    "[class*=cookie-banner]",
    "[class*=cookie-consent]",
    "[class*=cookie-notice]",
    "[class*=cookie-bar]",
    "[class*=cookie-law]",
    "[class*=cookie-popup]",
    "[class*=cookiebanner]",
    "[class*=cookieconsent]",
    "[class*=CookieConsent]",
    "[id*=onetrust]",
    "[class*=cc-window]",
    "[id*=gdpr]",
    "[class*=gdpr-]",
    "[aria-label*=cookie]",
    "[aria-label*=Cookie]",
];

/// HTML conversion for Shelf pages: main text only. The Aggressive preset
/// drops `<nav>`, page-level `<header>`, `<footer>`, `<aside>`, `<noscript>`,
/// forms and elements marked as menus, cookie or consent banners and ads;
/// scripts and styles never reach the text. Images and frames are left out
/// (their `src` would only add file names and `data:` payloads), and nothing
/// a page links to is ever loaded.
fn html_options() -> xberg::ConversionOptions {
    use html_to_markdown_rs::{InlineDataMedia, PreprocessingOptions, PreprocessingPreset};
    let mut options = xberg::ConversionOptions {
        // `<title>` and `<html lang>` land in the result metadata.
        extract_metadata: true,
        preprocessing: PreprocessingOptions {
            enabled: true,
            preset: PreprocessingPreset::Aggressive,
            remove_navigation: true,
            remove_forms: true,
        },
        skip_images: true,
        inline_data_media: InlineDataMedia::DropElement,
        exclude_selectors: HTML_EXCLUDE.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    // Same marks Xberg uses for Markdown when it picks the options itself.
    options.sup_symbol = "^".into();
    options.sub_symbol = "~".into();
    options
}

/// Per-file adjustments on top of [`build_config`].
fn config_for(path: &Path, settings: &ExtractorSettings, timeout_secs: u64) -> ExtractionConfig {
    let mut config = build_config(settings, timeout_secs);
    if is_html_file(path) {
        config.html_options = Some(html_options());
        // A page's inline pictures are decoration far more often than text.
        config.ocr_embedded_images = Some(false);
    }
    config
}

/// Xberg's Markdown for a web page opens with a YAML block of `<head>`
/// metadata (title, description, links). It is not page text: keep it out
/// of the cache, the index and the keywords.
fn strip_front_matter(text: &str) -> &str {
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return text;
    };
    if first.trim_end() != "---" {
        return text;
    }
    let mut offset = first.len();
    for line in lines {
        offset += line.len();
        if line.trim_end() == "---" {
            return text[offset..].trim_start();
        }
    }
    text
}

/// Turn Markdown links `[text](target)` into their text, so a page's URLs
/// don't become keywords, passages or PII hits. Fenced code is left alone.
fn strip_link_targets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence || !line.contains("](") {
            out.push_str(line);
        } else {
            out.push_str(&strip_line_links(line));
        }
    }
    out
}

fn strip_line_links(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            out.push(c);
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '[' {
            if let Some((label_end, target_end)) = link_at(&chars, i) {
                let label: String = chars[i + 1..label_end].iter().collect();
                out.push_str(&strip_line_links(&label));
                i = target_end + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// For a `[` at `open`, the index of its `]` and of the `)` closing the
/// target that follows it, when this is an inline link.
fn link_at(chars: &[char], open: usize) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut i = open;
    let label_end = loop {
        match chars.get(i)? {
            '\\' => i += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break i;
                }
            }
            _ => {}
        }
        i += 1;
    };
    if chars.get(label_end + 1) != Some(&'(') {
        return None;
    }
    let mut i = label_end + 2;
    if chars.get(i) == Some(&'<') {
        while *chars.get(i)? != '>' {
            i += 1;
        }
        i += 1;
    }
    let mut parens = 1usize;
    loop {
        match chars.get(i)? {
            '\\' => i += 1,
            '(' => parens += 1,
            ')' => {
                parens -= 1;
                if parens == 0 {
                    return Some((label_end, i));
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// `<html lang="es-ES">` → `es`, when Rebost knows the language.
fn html_lang(tag: &str) -> Option<&'static str> {
    let primary = tag.trim().split(['-', '_']).next()?;
    normalize_lang(primary)
}

/// Clean a filename into a presentable title: `bank-agreement_v2.pdf` →
/// `bank agreement v2`.
fn title_from_filename(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Document");
    stem.replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rebuild structural blocks from extracted markdown when the index is
/// rewritten without running Xberg again (schema bump).
pub(crate) fn blocks_from_markdown(content: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    for chunk in content.split("\n\n") {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix('#') {
            let level = 1 + heading.chars().take_while(|c| *c == '#').count() as u8;
            let text = heading.trim_start_matches('#').trim().to_string();
            if !text.is_empty() {
                blocks.push(Block {
                    kind: BlockKind::Heading { level },
                    text,
                    page: None,
                    page_end: None,
                });
                continue;
            }
        }
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            text: trimmed.to_string(),
            page: None,
            page_end: None,
        });
    }
    blocks
}

fn tidy(text: &str, max: usize) -> String {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() > max {
        let cut: String = cleaned.chars().take(max).collect();
        format!("{}…", cut.trim_end())
    } else {
        cleaned
    }
}

const STRUCTURED_SUMMARY_MAX: usize = 720;
const RANK_SUMMARY_MAX: usize = 360;

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn clip_summary_piece(text: &str, max: usize) -> String {
    if char_len(text) <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    if let Some(at) = cut.rfind(['.', '!', '?']) {
        if at >= max / 3 {
            return cut[..=at].trim_end().to_string();
        }
    }
    if let Some(at) = cut.rfind(char::is_whitespace) {
        if at >= max / 3 {
            return format!("{}…", cut[..at].trim_end());
        }
    }
    format!("{}…", cut.trim_end())
}

fn push_summary_piece(out: &mut String, piece: &str, max: usize) -> bool {
    let piece = piece.trim_end();
    if piece.is_empty() {
        return true;
    }
    let extra = if out.is_empty() { 0 } else { 2 };
    let next_len = char_len(out) + extra + char_len(piece);
    if !out.is_empty() && next_len > max {
        return false;
    }
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    if out.is_empty() && char_len(piece) > max {
        out.push_str(&clip_summary_piece(piece, max));
        return false;
    }
    out.push_str(piece);
    char_len(out) < max
}

/// Markdown preview from headings and the paragraphs under them.
fn summary_from_blocks(blocks: &[Block], max: usize) -> Option<String> {
    let structured = blocks.iter().any(|block| {
        matches!(
            block.kind,
            BlockKind::Heading { .. } | BlockKind::SheetStart { .. } | BlockKind::SlideStart { .. }
        )
    });
    if !structured {
        return None;
    }

    let mut out = String::new();
    for block in blocks {
        let piece = match &block.kind {
            BlockKind::Heading { level } => {
                let text = block.text.trim();
                if text.is_empty() {
                    continue;
                }
                let hashes = "#".repeat((*level).clamp(1, 6) as usize);
                format!("{hashes} {text}")
            }
            BlockKind::SheetStart { name } => {
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                format!("## {name}")
            }
            BlockKind::SlideStart { number, title } => {
                let label = title
                    .as_deref()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("Slide {number}"));
                format!("## {label}")
            }
            BlockKind::Paragraph => {
                let text = block.text.trim();
                if text.is_empty() {
                    continue;
                }
                text.to_string()
            }
            BlockKind::Table => continue,
        };
        if !push_summary_piece(&mut out, &piece, max) {
            break;
        }
    }

    let trimmed = out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn prose_for_summary(content: &str, blocks: &[Block]) -> String {
    let from_blocks = blocks
        .iter()
        .filter_map(|block| match block.kind {
            BlockKind::Paragraph | BlockKind::Table => Some(block.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    if !from_blocks.trim().is_empty() {
        return from_blocks;
    }
    content
        .lines()
        .map(|line| line.trim_start_matches('#').trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Xberg's OCR pipeline needs the same 16 MiB stack budget as its native wrappers.
/// Keep those stacks off Tauri's runtime, whose workers also handle chat and IPC.
/// Xberg renders OCR pages on the thread that polls the extraction, so each
/// ingest worker gets its own thread; with fewer, one file's rendering stalls
/// another's while its timeout keeps running.
fn extraction_runtime() -> Result<&'static tokio::runtime::Runtime> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, String>> =
        std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(crate::core::MAX_INGEST_WORKERS)
                .thread_name("rebost-extract")
                .thread_stack_size(16 * 1024 * 1024)
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow!("create document extraction runtime: {error}"))
}

/// Run Xberg on one file and shape the result for the pipeline.
pub async fn extract_file(path: &Path, settings: &ExtractorSettings) -> Result<Extraction> {
    ensure_tessdata(settings);
    let runtime = extraction_runtime()?;
    let pdf_pages = {
        let path = path.to_path_buf();
        runtime
            .spawn_blocking(move || pdf_page_count(&path))
            .await
            .unwrap_or(None)
    };
    let timeout_secs = timeout_for(settings.timeout_secs, pdf_pages);
    let config = config_for(path, settings, timeout_secs);
    let input = ExtractInput::from_uri(path.to_string_lossy().as_ref());
    let started = std::time::Instant::now();
    let output = runtime
        .spawn(async move {
            // Build the large extraction future only after reaching its worker stack.
            Box::pin(xberg::extract(input, &config)).await
        })
        .await
        .context("document extraction worker stopped")?
        .with_context(|| format!("extract {}", path.display()))?;
    log::info!(
        "extracted {} in {:.1}s (limit {timeout_secs}s, {} PDF pages)",
        path.file_name().unwrap_or_default().to_string_lossy(),
        started.elapsed().as_secs_f64(),
        pdf_pages.map_or_else(|| "no".to_string(), |n| n.to_string()),
    );

    let doc = output
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no extraction result for {}", path.display()))?;

    let is_html = is_html_file(path);
    let content = if is_html {
        strip_link_targets(strip_front_matter(doc.content.trim()))
            .trim()
            .to_string()
    } else {
        doc.content.trim().to_string()
    };
    let content = limit_extracted(content);
    let ocr_used =
        doc.metadata.ocr_used || doc.extraction_method.map(|m| m.used_ocr()).unwrap_or(false);

    // A scan that even OCR couldn't read is an error; a genuinely empty
    // file is not — it simply becomes Ready with nothing to index.
    if content.is_empty() && ocr_used {
        return Err(anyhow!("couldn't read any text, even with OCR"));
    }

    // Language: a page's own `<html lang>` first; otherwise xberg's
    // detection, which emits ISO 639-3 tags (e.g. "spa", "fra", "eng").
    let declared = if is_html {
        doc.metadata.language.as_deref().and_then(html_lang)
    } else {
        None
    };
    let (language, language_card) = match declared {
        Some(code) => (Some(code), Some(code.to_string())),
        None => {
            let language_tag = doc
                .detected_languages
                .as_ref()
                .and_then(|langs| langs.first())
                .cloned();
            let language = language_tag.as_deref().and_then(normalize_lang);
            (language, language.map(|l| l.to_string()).or(language_tag))
        }
    };

    // Structural blocks from the document tree; pages as fallback.
    let mut blocks: Vec<Block> = Vec::new();
    let mut outline: Vec<OutlineEntry> = Vec::new();
    let mut structure_title: Option<String> = None;

    if let Some(structure) = &doc.document {
        use xberg::types::document_structure::NodeContent;
        for node in &structure.nodes {
            match &node.content {
                NodeContent::Title { text } => {
                    if structure_title.is_none() && !text.trim().is_empty() {
                        structure_title = Some(text.clone());
                    }
                    if outline.len() < 40 && !text.trim().is_empty() {
                        outline.push(OutlineEntry {
                            title: tidy(text, 90),
                            page: node.page,
                        });
                    }
                    blocks.push(Block {
                        kind: BlockKind::Heading { level: 1 },
                        text: text.clone(),
                        page: node.page,
                        page_end: node.page_end,
                    });
                }
                NodeContent::Heading { level, text } => {
                    if !text.trim().is_empty() {
                        if *level <= 3 && outline.len() < 40 {
                            outline.push(OutlineEntry {
                                title: tidy(text, 90),
                                page: node.page,
                            });
                        }
                        blocks.push(Block {
                            kind: BlockKind::Heading { level: *level },
                            text: text.clone(),
                            page: node.page,
                            page_end: node.page_end,
                        });
                    }
                }
                NodeContent::Paragraph { text }
                | NodeContent::ListItem { text }
                | NodeContent::Footnote { text } => {
                    if !text.trim().is_empty() {
                        blocks.push(Block {
                            kind: BlockKind::Paragraph,
                            text: text.clone(),
                            page: node.page,
                            page_end: node.page_end,
                        });
                    }
                }
                NodeContent::Code { text, .. } | NodeContent::Formula { text } => {
                    if !text.trim().is_empty() {
                        blocks.push(Block {
                            kind: BlockKind::Paragraph,
                            text: text.clone(),
                            page: node.page,
                            page_end: node.page_end,
                        });
                    }
                }
                NodeContent::Table { grid } => {
                    let rendered = render_table(grid);
                    if !rendered.is_empty() {
                        blocks.push(Block {
                            kind: BlockKind::Table,
                            text: rendered,
                            page: node.page,
                            page_end: node.page_end,
                        });
                    }
                }
                NodeContent::Slide { number, title } => {
                    blocks.push(Block {
                        kind: BlockKind::SlideStart {
                            number: *number,
                            title: title.clone(),
                        },
                        text: String::new(),
                        page: node.page,
                        page_end: node.page_end,
                    });
                }
                _ => {}
            }
        }
    }

    // Page-level fallback / enrichment for sheets and slides.
    let pages = doc.pages.as_deref().unwrap_or(&[]);
    let page_count = (!pages.is_empty()).then_some(pages.len() as u32);

    if blocks.iter().all(|b| b.text.trim().is_empty()) {
        blocks.clear();
        if !pages.is_empty() {
            for page in pages {
                if let Some(sheet) = &page.sheet_name {
                    blocks.push(Block {
                        kind: BlockKind::SheetStart {
                            name: sheet.clone(),
                        },
                        text: String::new(),
                        page: Some(page.page_number),
                        page_end: Some(page.page_number),
                    });
                    if outline.len() < 40 {
                        outline.push(OutlineEntry {
                            title: sheet.clone(),
                            page: Some(page.page_number),
                        });
                    }
                } else if let Some(section) = &page.section_name {
                    blocks.push(Block {
                        kind: BlockKind::SlideStart {
                            number: page.page_number,
                            title: Some(section.clone()),
                        },
                        text: String::new(),
                        page: Some(page.page_number),
                        page_end: Some(page.page_number),
                    });
                }
                for paragraph in page.content.split("\n\n") {
                    if !paragraph.trim().is_empty() {
                        blocks.push(Block {
                            kind: BlockKind::Paragraph,
                            text: paragraph.trim().to_string(),
                            page: Some(page.page_number),
                            page_end: Some(page.page_number),
                        });
                    }
                }
                if let Some(notes) = &page.speaker_notes {
                    if !notes.trim().is_empty() {
                        blocks.push(Block {
                            kind: BlockKind::Paragraph,
                            text: format!("Speaker notes: {}", notes.trim()),
                            page: Some(page.page_number),
                            page_end: Some(page.page_number),
                        });
                    }
                }
            }
        } else {
            let md = blocks_from_markdown(&content);
            for block in &md {
                if let BlockKind::Heading { level } = &block.kind {
                    if *level <= 3 && outline.len() < 40 && !block.text.is_empty() {
                        outline.push(OutlineEntry {
                            title: tidy(&block.text, 90),
                            page: None,
                        });
                    }
                }
            }
            blocks.extend(md);
        }
    }

    // Title: document metadata (a page's `<title>`) → top-level heading (a
    // page's first `<h1>`) → cleaned filename.
    let first_h1 = is_html
        .then(|| {
            blocks.iter().find_map(|b| match b.kind {
                BlockKind::Heading { level: 1 } if !b.text.trim().is_empty() => {
                    Some(b.text.clone())
                }
                _ => None,
            })
        })
        .flatten();
    let title = doc
        .metadata
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or(structure_title)
        .or(first_h1)
        .map(|t| tidy(&t, 120))
        .unwrap_or_else(|| title_from_filename(path));

    // Structured files keep headings; the rest uses TextRank on prose.
    let summary = if content.is_empty() {
        String::new()
    } else {
        summary_from_blocks(&blocks, STRUCTURED_SUMMARY_MAX)
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| {
                let prose = prose_for_summary(&content, &blocks);
                if prose.trim().is_empty() {
                    return String::new();
                }
                xberg::text::summarization::textrank::summarize(
                    &prose,
                    language.or(Some("en")),
                    Some(80),
                )
                .map(|s| tidy(&s, RANK_SUMMARY_MAX))
                .unwrap_or_default()
            })
    };

    // Keywords: Xberg YAKE with the document's own language.
    let keywords = if content.is_empty() {
        Vec::new()
    } else {
        let keyword_config = KeywordConfig {
            algorithm: KeywordAlgorithm::Yake,
            max_keywords: 8,
            language: Some(language.unwrap_or("en").to_string()),
            ..Default::default()
        };
        xberg::keywords::extract_keywords(&content, &keyword_config)
            .map(|list| {
                list.into_iter()
                    .map(|k| tidy(&k.text, 60))
                    .filter(|k| !k.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let final_page_count =
        page_count.or_else(|| blocks.iter().filter_map(|b| b.page_end.or(b.page)).max());

    Ok(Extraction {
        content,
        title,
        language,
        language_tag: language_card,
        summary,
        keywords,
        outline,
        blocks: limit_blocks(blocks),
        page_count: final_page_count,
        ocr_used,
    })
}

fn render_table(grid: &xberg::types::document_structure::TableGrid) -> String {
    // Cells arrive in row-major order with explicit row indices.
    let max_rows = grid.rows.min(120);
    let mut rows: Vec<Vec<&str>> = vec![Vec::new(); max_rows as usize];
    for cell in &grid.cells {
        if cell.row < max_rows {
            rows[cell.row as usize].push(cell.content.as_str());
        }
    }
    let mut lines = Vec::new();
    for row in rows {
        let line = row
            .iter()
            .map(|c| c.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_grows_with_pdf_pages() {
        assert_eq!(timeout_for(300, None), 300);
        assert_eq!(timeout_for(300, Some(3)), 300);
        assert_eq!(timeout_for(300, Some(100)), 100 * SECS_PER_PDF_PAGE);
        assert_eq!(timeout_for(300, Some(100_000)), MAX_TIMEOUT_SECS);
        assert_eq!(timeout_for(300, Some(usize::MAX)), MAX_TIMEOUT_SECS);
    }

    #[test]
    fn page_count_reads_pdfs_only() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("note.md");
        std::fs::write(&note, "# Note").unwrap();
        assert_eq!(pdf_page_count(&note), None);
        let broken = dir.path().join("broken.pdf");
        std::fs::write(&broken, "not a pdf").unwrap();
        assert_eq!(pdf_page_count(&broken), None);
        assert_eq!(pdf_page_count(&dir.path().join("missing.pdf")), None);
    }

    /// Real OCR timing on a scanned PDF: `REBOST_SCAN_PDF=/path/scan.pdf
    /// cargo test --lib scanned_pdf_reads_within_its_timeout -- --ignored --nocapture`
    #[test]
    #[ignore = "needs a scanned PDF via REBOST_SCAN_PDF; slow"]
    fn scanned_pdf_reads_within_its_timeout() {
        let path = PathBuf::from(std::env::var("REBOST_SCAN_PDF").expect("REBOST_SCAN_PDF"));
        let pages = pdf_page_count(&path).expect("page count");
        let tessdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/tessdata");
        let settings = ExtractorSettings {
            tessdata_dir: Some(tessdata),
            tessdata_bundle: None,
            timeout_secs: 300,
        };
        let started = std::time::Instant::now();
        let extraction = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(extract_file(&path, &settings))
            .expect("extract");
        let secs = started.elapsed().as_secs_f64();
        eprintln!(
            "{pages} pages, {secs:.1}s ({:.1}s/page), ocr={}, {} chars, limit {}s",
            secs / pages as f64,
            extraction.ocr_used,
            extraction.content.len(),
            timeout_for(300, Some(pages)),
        );
        assert!(extraction.ocr_used);
        assert!(!extraction.content.is_empty());
    }

    #[test]
    fn ocr_languages_fall_back_to_english() {
        assert_eq!(ocr_languages(None), vec!["eng".to_string()]);
    }

    #[test]
    fn bundled_tessdata_packs_exist() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/tessdata");
        let langs = ocr_languages(Some(&dir));
        assert!(
            !langs.is_empty(),
            "expected at least one *.traineddata pack in {}",
            dir.display()
        );
        for language in &langs {
            let path = dir.join(format!("{language}.traineddata"));
            assert!(path.is_file(), "missing {}", path.display());
            let size = std::fs::metadata(&path).unwrap().len();
            assert!(
                size > 1_000_000,
                "{} is suspiciously small ({size} bytes)",
                path.display()
            );
        }
        assert!(
            dir.join("LICENSE").is_file(),
            "tessdata must ship the Apache-2.0 LICENSE next to the packs"
        );
    }

    #[test]
    fn tessdata_copies_into_empty_dest() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/tessdata");
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("tessdata");
        assert!(!tessdata_has_packs(&dest));
        ensure_tessdata(&ExtractorSettings {
            tessdata_dir: Some(dest.clone()),
            tessdata_bundle: Some(src),
            timeout_secs: 30,
        });
        assert!(tessdata_has_packs(&dest));
        assert!(
            dest.join("LICENSE").is_file(),
            "Apache-2.0 LICENSE should copy with the packs"
        );
    }

    #[test]
    fn only_document_types_are_offered() {
        assert!(is_supported_file(Path::new("brief.pdf")));
        assert!(is_supported_file(Path::new("note.md")));
        assert!(is_supported_file(Path::new("sheet.xlsx")));
        assert!(is_supported_file(Path::new("page.html")));
        assert!(is_supported_file(Path::new("old/PAGE.HTM")));
        assert!(is_supported_file(Path::new("chapter.xhtml")));
        assert!(!is_supported_file(Path::new("package.json")));
        assert!(!is_supported_file(Path::new("data.xml")));
        assert!(!is_supported_file(Path::new("Page_files/app.js")));
        assert!(!is_supported_file(Path::new("Page_files/style.css")));
        assert!(!is_supported_file(Path::new("main.rs")));
        assert!(!is_supported_file(Path::new("script.py")));
        assert!(!is_supported_file(Path::new("archive.mhtml")));
        assert!(!is_supported_file(Path::new("~$lock.docx")));
        assert!(!is_supported_file(Path::new("~$Book.xlsx")));
        assert!(!is_supported_file(Path::new("scratch.tmp")));
        assert!(!is_supported_file(Path::new("notes/draft.TMP")));
        assert!(!is_supported_file(Path::new(".hidden.md")));
        assert!(!is_supported_file(Path::new("clip.mp4")));
        assert!(is_supported_file(Path::new("report.docx")));
    }

    #[test]
    fn skip_file_name_covers_locks_and_tmp() {
        assert!(skip_file_name("~$lock.docx"));
        assert!(skip_file_name("scratch.tmp"));
        assert!(skip_file_name("SCRATCH.TMP"));
        assert!(skip_file_name(".hidden.md"));
        assert!(skip_file_name("shelf.yml"));
        assert!(!skip_file_name("note.md"));
        assert!(!skip_file_name("report.docx"));
    }

    #[test]
    fn limit_extracted_keeps_short_text() {
        assert_eq!(limit_extracted("hi".into()), "hi");
    }

    #[test]
    fn limit_extracted_cuts_at_a_paragraph() {
        let chunk = "hello world paragraph\n\n";
        let input = chunk.repeat(EXTRACTED_MAX / chunk.len() + 80);
        let out = limit_extracted(input);
        assert!(out.len() <= EXTRACTED_MAX);
        assert!(out.contains("hello world paragraph"));
    }

    fn block(kind: BlockKind, text: &str) -> Block {
        Block {
            kind,
            text: text.to_string(),
            page: None,
            page_end: None,
        }
    }

    #[test]
    fn structured_summary_keeps_markdown_headings() {
        let summary = summary_from_blocks(
            &[
                block(BlockKind::Heading { level: 1 }, "Chapter one"),
                block(BlockKind::Heading { level: 2 }, "Opening"),
                block(BlockKind::Paragraph, "The clause lasts ninety days."),
                block(BlockKind::Heading { level: 2 }, "Next"),
                block(BlockKind::Paragraph, "A later section follows."),
            ],
            720,
        )
        .expect("structured summary");
        assert_eq!(
            summary,
            "# Chapter one\n\n## Opening\n\nThe clause lasts ninety days.\n\n## Next\n\nA later section follows."
        );
    }

    #[test]
    fn structured_summary_stops_at_a_block() {
        let first = "## Opening\n\nThe clause lasts ninety days. Notice is written.";
        let summary = summary_from_blocks(
            &[
                block(BlockKind::Heading { level: 2 }, "Opening"),
                block(
                    BlockKind::Paragraph,
                    "The clause lasts ninety days. Notice is written.",
                ),
                block(BlockKind::Heading { level: 2 }, "Later"),
                block(
                    BlockKind::Paragraph,
                    "This paragraph is far too long to fit in a short summary budget.",
                ),
            ],
            first.chars().count() + 4,
        )
        .expect("structured summary");
        assert_eq!(summary, first);
        assert!(!summary.contains("Later"));
    }

    #[test]
    fn prose_summary_skips_heading_lines() {
        let prose = prose_for_summary(
            "# Chapter one\n\n## Opening\n\nThe clause lasts ninety days.",
            &[
                block(BlockKind::Heading { level: 1 }, "Chapter one"),
                block(BlockKind::Heading { level: 2 }, "Opening"),
                block(BlockKind::Paragraph, "The clause lasts ninety days."),
            ],
        );
        assert_eq!(prose, "The clause lasts ninety days.");
        assert!(!prose.contains('#'));
    }

    #[test]
    fn front_matter_is_dropped_from_page_text() {
        let md = "---\ntitle: Horario\nmeta-description: d\n---\n\n# Horario\n\nAbre a las nueve.";
        assert_eq!(strip_front_matter(md), "# Horario\n\nAbre a las nueve.");
        assert_eq!(strip_front_matter("# No front matter"), "# No front matter");
        // An unclosed block is left as it is rather than eating the page.
        assert_eq!(strip_front_matter("---\nnot closed"), "---\nnot closed");
        // A rule later on is page text.
        assert_eq!(
            strip_front_matter("Intro\n\n---\n\nMore"),
            "Intro\n\n---\n\nMore"
        );
    }

    #[test]
    fn link_targets_are_dropped_but_text_kept() {
        assert_eq!(
            strip_link_targets("Consulte el [reglamento](https://example.org/r) hoy."),
            "Consulte el reglamento hoy."
        );
        assert_eq!(
            strip_link_targets("[a [nested] label](<with space.html>) and [b](x(1).html)"),
            "a [nested] label and b"
        );
        assert_eq!(strip_link_targets("[not a link] (x)"), "[not a link] (x)");
        assert_eq!(
            strip_link_targets("broken [link](unclosed"),
            "broken [link](unclosed"
        );
        assert_eq!(strip_link_targets("\\[kept](x)"), "\\[kept](x)");
        assert_eq!(
            strip_link_targets("```\n[code](kept)\n```\n[t](u)\n"),
            "```\n[code](kept)\n```\nt\n"
        );
    }

    #[test]
    fn html_lang_takes_the_primary_subtag() {
        assert_eq!(html_lang("es"), Some("es"));
        assert_eq!(html_lang("ca-ES"), Some("ca"));
        assert_eq!(html_lang(" en_GB "), Some("en"));
        assert_eq!(html_lang("x-klingon"), None);
        assert_eq!(html_lang(""), None);
    }

    #[test]
    fn html_files_get_page_cleanup() {
        let settings = ExtractorSettings::default();
        let page = config_for(Path::new("Page.HTML"), &settings, 30);
        assert!(page.html_options.is_some());
        assert_eq!(page.ocr_embedded_images, Some(false));
        let doc = config_for(Path::new("brief.docx"), &settings, 30);
        assert!(doc.html_options.is_none());
        assert_eq!(doc.ocr_embedded_images, None);
    }
}
