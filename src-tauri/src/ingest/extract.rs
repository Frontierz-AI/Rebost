//! Xberg integration — the single document-extraction layer.
//!
//! Everything here is deterministic and fully local: native text first,
//! vendored-tesseract OCR when a file or page has no usable text layer
//! (whatever `*.traineddata` packs are in the tessdata directory).

use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use xberg::core::config::{LanguageDetectionConfig, OutputFormat, PageConfig};
use xberg::{
    ExtractInput, ExtractionConfig, KeywordAlgorithm, KeywordConfig, OcrConfig, OcrStrategy,
};

use crate::search::normalize_lang;
use crate::types::OutlineEntry;

/// The formats Rebost accepts, intersected with Xberg's format registry at
/// startup (the registry is the source of truth for what the build handles).
const REBOST_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "html", "htm", "md",
    "markdown", "txt", "rtf", "csv", "tsv", "eml", "msg", "epub", "json", "yaml", "yml", "xml",
];

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

pub fn is_supported_file(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.') || n == "shelf.yml")
        .unwrap_or(true)
    {
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
    /// Directory holding `*.traineddata` packs (bundled with the app).
    pub tessdata_dir: Option<PathBuf>,
    /// Extraction timeout per file.
    pub timeout_secs: u64,
}

impl Default for ExtractorSettings {
    fn default() -> Self {
        Self {
            tessdata_dir: None,
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

fn build_config(settings: &ExtractorSettings) -> ExtractionConfig {
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
        extraction_timeout_secs: Some(settings.timeout_secs),
        ..Default::default()
    }
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

fn tidy(text: &str, max: usize) -> String {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() > max {
        let cut: String = cleaned.chars().take(max).collect();
        format!("{}…", cut.trim_end())
    } else {
        cleaned
    }
}

/// Run Xberg on one file and shape the result for the pipeline.
pub async fn extract_file(path: &Path, settings: &ExtractorSettings) -> Result<Extraction> {
    let config = build_config(settings);
    let input = ExtractInput::from_uri(path.to_string_lossy().as_ref());
    let output = xberg::extract(input, &config)
        .await
        .with_context(|| format!("extract {}", path.display()))?;

    let doc = output
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no extraction result for {}", path.display()))?;

    let content = doc.content.trim().to_string();
    let ocr_used =
        doc.metadata.ocr_used || doc.extraction_method.map(|m| m.used_ocr()).unwrap_or(false);

    // A scan that even OCR couldn't read is an error; a genuinely empty
    // file is not — it simply becomes Ready with nothing to index.
    if content.is_empty() && ocr_used {
        return Err(anyhow!("couldn't read any text, even with OCR"));
    }

    // Language: xberg emits ISO 639-3 tags (e.g. "spa", "fra", "eng").
    let language_tag = doc
        .detected_languages
        .as_ref()
        .and_then(|langs| langs.first())
        .cloned();
    let language = language_tag.as_deref().and_then(normalize_lang);
    let language_card = language.map(|l| l.to_string()).or(language_tag.clone());

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
    let page_count = if !pages.is_empty() {
        Some(pages.len() as u32)
    } else {
        doc.metadata.pages.as_ref().map(|_| 0).filter(|c| *c > 0)
    };

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
            // Last resort: markdown-ish split of the whole content.
            for chunk in content.split("\n\n") {
                let trimmed = chunk.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(heading) = trimmed.strip_prefix('#') {
                    let level = 1 + heading.chars().take_while(|c| *c == '#').count() as u8;
                    let text = heading.trim_start_matches('#').trim().to_string();
                    if !text.is_empty() {
                        if level <= 3 && outline.len() < 40 {
                            outline.push(OutlineEntry {
                                title: tidy(&text, 90),
                                page: None,
                            });
                        }
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
        }
    }

    // Title: document metadata → top-level heading → cleaned filename.
    let title = doc
        .metadata
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or(structure_title)
        .map(|t| tidy(&t, 120))
        .unwrap_or_else(|| title_from_filename(path));

    // Summary: Xberg TextRank with the document's own language.
    let summary = if content.is_empty() {
        String::new()
    } else {
        xberg::text::summarization::textrank::summarize(&content, language.or(Some("en")), Some(80))
            .map(|s| tidy(&s, 360))
            .unwrap_or_default()
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
        blocks,
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
    }
}
