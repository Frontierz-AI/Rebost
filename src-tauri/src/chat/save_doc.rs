//! Save a Chat answer as a document on the conversation's Shelf.
//!
//! The `save_document` tool only carries a file name and a format. The
//! document itself is the model's next, normal (streamed) answer; when the
//! turn finishes cleanly, that answer is written here. The "Save to Shelf"
//! button under an answer uses the same path.
//!
//! ```text
//! answer → strip [S1] markers (+ a short Sources list)
//!        → .docx / .md / .txt bytes
//!        → <Shelf folder>/<Name>.<ext>   (never a subfolder, never a linked folder,
//!                                         never overwrite: Name-1, Name-2…)
//!        → registry entry marked "written by Rebost" → normal ingest
//! ```

use std::collections::HashSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::core::Ctx;
use crate::ingest::{Job, ProcessJob};
use crate::shelf::Shelf;
use crate::types::{DocStatus, DocumentMeta, SourcePassage, SourceType};

/// Longest file stem, in UTF-8 bytes. Leaves room for `-NN.docx` under the
/// 255-unit file-name limit on every platform.
const STEM_MAX_BYTES: usize = 120;
/// Error code the UI turns into "This Shelf is full".
pub(crate) const ERROR_SHELF_FULL: &str = "shelf-full";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    #[default]
    Docx,
    Md,
    Txt,
}

impl DocFormat {
    pub(crate) fn ext(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Md => "md",
            Self::Txt => "txt",
        }
    }

    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "docx" | "word" | "doc" | "microsoft word" => Some(Self::Docx),
            "md" | "markdown" => Some(Self::Md),
            "txt" | "text" | "plain" | "plain text" | "plaintext" => Some(Self::Txt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveStatus {
    Saved,
    Failed,
    Undone,
}

/// What happened to the document a turn (or the button) asked to save.
/// Stored on the assistant message so the notice survives a reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedDocument {
    pub status: SaveStatus,
    pub file_name: String,
    pub format: DocFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shelf_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Hash of the bytes written. Undo only removes a file that still matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// `shelf-full`, or a short reason the save did not happen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SavedDocument {
    pub(crate) fn failed(file_name: String, format: DocFormat, error: impl Into<String>) -> Self {
        Self {
            status: SaveStatus::Failed,
            file_name,
            format,
            shelf_id: None,
            document_id: None,
            path: None,
            hash: None,
            error: Some(error.into()),
        }
    }
}

/// A save the model asked for this turn. Written only if the turn ends well.
#[derive(Debug, Clone)]
pub(crate) struct SaveRequest {
    pub shelf_id: String,
    pub stem: String,
    pub format: DocFormat,
    /// Set when the save cannot happen (Shelf full); the turn still answers.
    pub refused: Option<String>,
}

impl SaveRequest {
    pub(crate) fn file_label(&self) -> String {
        file_label(&self.stem, self.format)
    }
}

pub(crate) fn file_label(stem: &str, format: DocFormat) -> String {
    format!("{stem}.{}", format.ext())
}

/// Words that suggest the user wants a file. `save_document` is offered
/// only then, so a plain question never tempts a small model into it. The
/// "Save to Shelf" button covers everything else.
const SAVE_HINTS: &[&str] = &[
    // en
    "document",
    "doc",
    "file",
    "report",
    "letter",
    "memo",
    "draft",
    "write",
    "save",
    "summary",
    "notes",
    "essay",
    "proposal",
    "minutes",
    "markdown",
    "txt",
    "word",
    // es / ca / pt / it
    "archivo",
    "fichero",
    "informe",
    "carta",
    "borrador",
    "redact",
    "escrib",
    "escriu",
    "guarda",
    "desa",
    "resum",
    "nota",
    "acta",
    "propuesta",
    "proposta",
    "arxiu",
    "fitxer",
    "esborrany",
    "arquivo",
    "ficheiro",
    "relatório",
    "rascunho",
    "escrev",
    "salva",
    "rapporto",
    "relazione",
    "lettera",
    "bozza",
    "scrivi",
    "riassunto",
    "verbale",
    // fr / de / nl
    "fichier",
    "rapport",
    "lettre",
    "brouillon",
    "rédige",
    "écri",
    "enregistr",
    "sauvegard",
    "résumé",
    "compte rendu",
    "dokument",
    "datei",
    "bericht",
    "brief",
    "entwurf",
    "schreib",
    "speicher",
    "zusammenfass",
    "protokoll",
    "bestand",
    "verslag",
    "concept",
    "schrijf",
    "opslaan",
    "samenvatting",
    // sv / da / nb / fi / cs
    "fil",
    "utkast",
    "udkast",
    "skriv",
    "spara",
    "gem ",
    "lagre",
    "sammanfatt",
    "sammendrag",
    "asiakirja",
    "dokumentti",
    "tiedosto",
    "raportti",
    "kirje",
    "luonnos",
    "kirjoita",
    "tallenna",
    "yhteenveto",
    "soubor",
    "zpráv",
    "dopis",
    "napiš",
    "ulož",
    "shrnutí",
    // el / ja
    "έγγραφ",
    "αρχείο",
    "αναφορ",
    "επιστολ",
    "γράψ",
    "αποθήκευ",
    "περίληψ",
    "文書",
    "ドキュメント",
    "ファイル",
    "報告",
    "レポート",
    "手紙",
    "下書き",
    "書いて",
    "保存",
    "要約",
    "議事録",
];

pub(crate) fn save_intent(text: &str) -> bool {
    let lower = text.to_lowercase();
    SAVE_HINTS.iter().any(|hint| lower.contains(hint))
}

/// Library Shelf that may receive a saved document (not an upload Shelf).
pub(crate) fn writable_shelf(ctx: &Ctx, shelf_id: &str) -> Option<Shelf> {
    crate::core::read_lock(&ctx.library)
        .shelf(shelf_id)
        .filter(|shelf| shelf.thread_id.is_none())
        .cloned()
}

pub(crate) fn shelf_has_room(ctx: &Ctx, shelf_id: &str) -> bool {
    crate::ingest::remaining_file_slots(ctx, shelf_id) > 0
}

const KNOWN_EXTENSIONS: &[&str] = &[
    "docx", "doc", "md", "markdown", "txt", "text", "pdf", "rtf", "odt", "html", "htm",
];

/// Format named by a trailing extension on the requested name, if any.
pub(crate) fn format_from_name(raw: &str) -> Option<DocFormat> {
    let ext = raw.trim().rsplit_once('.')?.1;
    DocFormat::parse(ext)
}

/// A safe, readable file stem: no path parts, no control characters, no
/// Windows reserved names, no trailing dots or spaces, bounded length.
/// Any extension the model typed is dropped; the format decides it.
pub(crate) fn sanitize_stem(raw: &str) -> String {
    let mut name: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' => '-',
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    loop {
        let trimmed = name.trim().trim_end_matches('.').trim();
        let Some((stem, ext)) = trimmed.rsplit_once('.') else {
            name = trimmed.to_string();
            break;
        };
        if KNOWN_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
            name = stem.to_string();
        } else {
            name = trimmed.to_string();
            break;
        }
    }
    while name.contains("..") {
        name = name.replace("..", ".");
    }
    let mut name = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches(['.', '-', ' '])
        .to_string();
    if name.len() > STEM_MAX_BYTES {
        let mut cut = STEM_MAX_BYTES;
        while !name.is_char_boundary(cut) {
            cut -= 1;
        }
        name.truncate(cut);
    }
    let name = name.trim_end_matches(['.', ' ']).trim().to_string();
    if name.is_empty() {
        return "Document".into();
    }
    let base_len = name.find('.').unwrap_or(name.len());
    if is_windows_reserved(&name[..base_len].trim().to_ascii_uppercase()) {
        let mut name = name;
        name.insert(base_len, '_');
        return name;
    }
    name
}

fn is_windows_reserved(base: &str) -> bool {
    matches!(base, "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base[3..]
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '¹' | '²' | '³')))
}

/// First free `Name.ext`, `Name-1.ext`, `Name-2.ext`… in `dir`, compared
/// case-insensitively (macOS and Windows folders usually are).
pub(crate) fn unique_file_name(dir: &Path, stem: &str, ext: &str) -> String {
    let taken: HashSet<String> = std::fs::read_dir(crate::shelf::with_long_path(dir))
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let mut n = 0usize;
    loop {
        let name = if n == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        if !taken.contains(&name.to_lowercase()) {
            return name;
        }
        n += 1;
    }
}

fn marker_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"[ \t]*\[S(\d+)(?:\s*[,;]\s*S?\d+)*\]").expect("citation marker")
    })
}

/// Drop `[S1]` markers. When they pointed at Shelf files, end with a short
/// Sources list naming those files instead.
pub(crate) fn document_text(answer: &str, sources: &[SourcePassage]) -> String {
    let body = unwrap_fence(answer.trim());
    let mut order: Vec<String> = Vec::new();
    for caps in marker_re().captures_iter(body) {
        for id in caps[0]
            .trim()
            .trim_matches(['[', ']'])
            .split([',', ';'])
            .map(|part| format!("S{}", part.trim().trim_start_matches(['S', 's'])))
        {
            if !order.contains(&id) {
                order.push(id);
            }
        }
    }
    let mut text = marker_re().replace_all(body, "").trim().to_string();
    let mut listed: Vec<String> = Vec::new();
    for id in &order {
        let Some(source) = sources.iter().find(|s| &s.sid == id) else {
            continue;
        };
        let mut line = source.title.clone();
        if let Some(page) = source.page_start {
            line.push_str(&format!(" (p. {page})"));
        }
        if source.written_by_rebost {
            line.push_str(&format!(" ({})", rust_i18n::t!("chat.writtenByRebost")));
        }
        if !listed.contains(&line) {
            listed.push(line);
        }
    }
    if !listed.is_empty() {
        text.push_str(&format!(
            "\n\n## {}\n\n",
            rust_i18n::t!("chat.saveSourcesHeading")
        ));
        for line in listed {
            text.push_str(&format!("- {line}\n"));
        }
    }
    text.trim_end().to_string() + "\n"
}

/// Some models wrap the whole document in a ```markdown fence.
fn unwrap_fence(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    let Some(newline) = rest.find('\n') else {
        return text;
    };
    let lang = rest[..newline].trim().to_ascii_lowercase();
    if !matches!(lang.as_str(), "" | "markdown" | "md" | "text") {
        return text;
    }
    let inner = &rest[newline + 1..];
    match inner.trim_end().strip_suffix("```") {
        Some(body) if !body.contains("\n```") => body.trim(),
        _ => text,
    }
}

/// First `# Heading` of the text, or the stem.
fn title_for(text: &str, stem: &str) -> String {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("# ").map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| stem.to_string())
}

/// File stem for the "Save to Shelf" button: the first heading, else the
/// start of the first line, else `fallback`.
pub(crate) fn default_stem(answer: &str, fallback: &str) -> String {
    let line = unwrap_fence(answer.trim())
        .lines()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let line = marker_re().replace_all(line, "");
    let words: Vec<&str> = line
        .trim_matches(|c: char| c == '*' || c == '_')
        .split_whitespace()
        .collect();
    let mut stem = String::new();
    for word in words {
        if stem.chars().count() + word.chars().count() > 60 {
            break;
        }
        if !stem.is_empty() {
            stem.push(' ');
        }
        stem.push_str(word);
    }
    let stem = sanitize_stem(stem.trim_end_matches([',', ';', ':', '.']));
    if stem == "Document" && !fallback.trim().is_empty() {
        sanitize_stem(fallback)
    } else {
        stem
    }
}

pub(crate) fn render(text: &str, format: DocFormat, stem: &str) -> Result<Vec<u8>> {
    match format {
        DocFormat::Docx => crate::docx::markdown_to_docx(text, &title_for(text, stem)),
        DocFormat::Md => Ok(text.as_bytes().to_vec()),
        DocFormat::Txt => Ok(markdown_to_plain(text).into_bytes()),
    }
}

/// Readable plain text from Markdown: no `#`, `**` or table pipes.
pub(crate) fn markdown_to_plain(markdown: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let mut out = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut link: Vec<String> = Vec::new();
    let mut cell = 0usize;
    let newline = |out: &mut String| {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
    };
    let blank = |out: &mut String| {
        if out.is_empty() {
            return;
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
    };
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Paragraph) => {
                if lists.is_empty() && !out.ends_with("> ") {
                    blank(&mut out);
                }
            }
            Event::Start(Tag::Heading { .. }) | Event::Start(Tag::CodeBlock(_)) => blank(&mut out),
            Event::Start(Tag::BlockQuote(_)) => {
                blank(&mut out);
                out.push_str("> ");
            }
            Event::Start(Tag::List(start)) => {
                if lists.is_empty() {
                    blank(&mut out);
                } else {
                    newline(&mut out);
                }
                lists.push(start);
            }
            Event::Start(Tag::Item) => {
                newline(&mut out);
                let depth = lists.len().saturating_sub(1);
                out.push_str(&"  ".repeat(depth));
                match lists.last_mut() {
                    Some(Some(n)) => {
                        out.push_str(&format!("{n}. "));
                        *n += 1;
                    }
                    _ => out.push_str("- "),
                }
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                newline(&mut out);
            }
            Event::Start(Tag::Table(_)) => blank(&mut out),
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                newline(&mut out);
                cell = 0;
            }
            Event::Start(Tag::TableCell) => {
                if cell > 0 {
                    out.push('\t');
                }
                cell += 1;
            }
            Event::Start(Tag::Link { dest_url, .. }) => link.push(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = link.pop() {
                    if url.starts_with("http://") || url.starts_with("https://") {
                        out.push_str(&format!(" ({url})"));
                    }
                }
            }
            Event::Text(text) | Event::Code(text) => out.push_str(&text),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Rule => {
                blank(&mut out);
                out.push_str("----------");
            }
            Event::TaskListMarker(done) => out.push_str(if done { "[x] " } else { "[ ] " }),
            Event::Html(html) | Event::InlineHtml(html)
                if html.trim().to_ascii_lowercase().starts_with("<br") =>
            {
                out.push('\n');
            }
            _ => {}
        }
    }
    out.trim().to_string() + "\n"
}

/// Write `answer` as `<stem>.<ext>` at the root of the Shelf's own folder and
/// register it for reading. Returns the notice for the message and, on
/// success, the ingest job to queue.
pub(crate) fn save_answer(
    ctx: &Ctx,
    shelf_id: &str,
    stem: &str,
    format: DocFormat,
    answer: &str,
    sources: &[SourcePassage],
) -> (SavedDocument, Option<ProcessJob>) {
    let stem = sanitize_stem(stem);
    let label = file_label(&stem, format);
    let Some(shelf) = writable_shelf(ctx, shelf_id) else {
        return (
            SavedDocument::failed(label, format, rust_i18n::t!("errors.saveNeedsShelf")),
            None,
        );
    };
    if !shelf_has_room(ctx, shelf_id) {
        return (SavedDocument::failed(label, format, ERROR_SHELF_FULL), None);
    }
    let text = document_text(answer, sources);
    if text.trim().is_empty() {
        return (
            SavedDocument::failed(label, format, rust_i18n::t!("errors.saveEmpty")),
            None,
        );
    }
    match write_into_shelf(ctx, &shelf, &stem, format, &text) {
        Ok(done) => done,
        Err(error) => {
            log::warn!("saving a document to {shelf_id}: {error:#}");
            (
                SavedDocument::failed(label, format, rust_i18n::t!("errors.saveFailed")),
                None,
            )
        }
    }
}

fn write_into_shelf(
    ctx: &Ctx,
    shelf: &Shelf,
    stem: &str,
    format: DocFormat,
    text: &str,
) -> Result<(SavedDocument, Option<ProcessJob>)> {
    let bytes = render(text, format, stem)?;
    let root = shelf.managed_path.clone();
    let (path, file_name) = create_unique(&root, stem, format.ext())?;
    let rel_path = file_name.clone();
    let doc_id = crate::ids::document_id(&shelf.id, Shelf::IMPORTED_SOURCE, &rel_path);
    let meta = DocumentMeta {
        id: doc_id.clone(),
        written_by_rebost: true,
        shelf_id: shelf.id.clone(),
        source_id: Shelf::IMPORTED_SOURCE.into(),
        source_type: SourceType::Imported,
        path: path.to_string_lossy().to_string(),
        rel_path: rel_path.clone(),
        file_name: file_name.clone(),
        format: format.ext().into(),
        size_bytes: 0,
        mtime_ms: 0,
        hash: String::new(),
        status: DocStatus::Reading,
        error: None,
        passage_count: 0,
        pages: None,
        pii_total: 0,
        pii_categories: Default::default(),
        ocr: false,
        updated_at: crate::timestamps::now_string(),
        source_label: "Imported".into(),
    };
    // Registered before the bytes land, so the folder watcher's own job
    // finds the "written by Rebost" mark already there.
    {
        let mut library = crate::core::write_lock(&ctx.library);
        library.upsert_document(meta.clone());
        if let Err(error) = library.save_documents(&ctx.paths, &shelf.id) {
            log::error!("saving document registry for {}: {error:#}", shelf.id);
        }
    }
    let written = std::fs::OpenOptions::new()
        .write(true)
        .open(crate::shelf::with_long_path(&path))
        .and_then(|mut file| {
            file.write_all(&bytes)?;
            file.sync_all()
        });
    if let Err(error) = written {
        let _ = std::fs::remove_file(crate::shelf::with_long_path(&path));
        let mut library = crate::core::write_lock(&ctx.library);
        library.remove_document(&shelf.id, &doc_id);
        let _ = library.save_documents(&ctx.paths, &shelf.id);
        return Err(error.into());
    }
    crate::ingest::emit_file_event(ctx, &meta);
    let hash = crate::ids::content_hash_file(&path).ok();
    let saved = SavedDocument {
        status: SaveStatus::Saved,
        file_name: file_name.clone(),
        format,
        shelf_id: Some(shelf.id.clone()),
        document_id: Some(doc_id),
        path: Some(path.to_string_lossy().to_string()),
        hash,
        error: None,
    };
    let job = ProcessJob {
        shelf_id: shelf.id.clone(),
        source_id: Shelf::IMPORTED_SOURCE.into(),
        source_type: SourceType::Imported,
        source_label: "Imported".into(),
        abs_path: path,
        rel_path,
        force: false,
        epoch: 0,
    };
    Ok((saved, Some(job)))
}

/// Reserve a new file directly in `root`. `create_new` makes the
/// no-overwrite rule hold even if another program races us.
fn create_unique(root: &Path, stem: &str, ext: &str) -> Result<(PathBuf, String)> {
    std::fs::create_dir_all(crate::shelf::with_long_path(root))?;
    for _ in 0..64 {
        let name = unique_file_name(root, stem, ext);
        let path = root.join(&name);
        if path.parent() != Some(root) {
            return Err(anyhow!("file name left the Shelf folder"));
        }
        if crate::shelf::dest_too_long(&path) {
            return Err(anyhow!("file name too long"));
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(crate::shelf::with_long_path(&path))
        {
            Ok(_) => return Ok((path, name)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(anyhow!("no free file name"))
}

pub(crate) fn queue(ingestor: Option<&crate::ingest::Ingestor>, job: Option<ProcessJob>) {
    let (Some(ingestor), Some(job)) = (ingestor, job) else {
        return;
    };
    let ingestor = ingestor.clone();
    tokio::spawn(async move { ingestor.enqueue(Job::Process(job)).await });
}

/// Remove a saved file and its document. Refuses a file that is no longer
/// the one Rebost wrote (edited since, or outside the Shelf's own folder).
pub(crate) fn undo(ctx: &std::sync::Arc<Ctx>, saved: &SavedDocument) -> Result<()> {
    if saved.status != SaveStatus::Saved {
        return Ok(());
    }
    let (Some(shelf_id), Some(doc_id), Some(path)) = (
        saved.shelf_id.as_deref(),
        saved.document_id.as_deref(),
        saved.path.as_deref(),
    ) else {
        return Err(anyhow!("{}", rust_i18n::t!("errors.saveUndoFailed")));
    };
    let path = PathBuf::from(path);
    let root = crate::core::read_lock(&ctx.library)
        .shelf(shelf_id)
        .map(|s| s.managed_path.clone());
    let inside = root.is_some_and(|root| {
        path.parent().is_some_and(|parent| {
            parent == root
                || matches!(
                    (parent.canonicalize(), root.canonicalize()),
                    (Ok(a), Ok(b)) if a == b
                )
        })
    });
    if !inside {
        return Err(anyhow!("{}", rust_i18n::t!("errors.saveUndoFailed")));
    }
    if path.exists() {
        let current = crate::ids::content_hash_file(&path).ok();
        if saved.hash.is_some() && current != saved.hash {
            return Err(anyhow!("{}", rust_i18n::t!("errors.saveUndoChanged")));
        }
        std::fs::remove_file(crate::shelf::with_long_path(&path))?;
    }
    crate::ingest::remove_document(ctx, shelf_id, doc_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NoopEvents;
    use crate::ingest::extract::ExtractorSettings;
    use crate::paths::Paths;
    use std::sync::Arc;

    fn setup() -> (tempfile::TempDir, Arc<Ctx>, Shelf) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        let ctx = Ctx::new(paths, Arc::new(NoopEvents), ExtractorSettings::default()).unwrap();
        let shelf = crate::core::write_lock(&ctx.library)
            .create_shelf(&ctx.paths, "Docs", &dir.path().join("Rebost"))
            .unwrap();
        (dir, ctx, shelf)
    }

    fn source(sid: &str, title: &str) -> SourcePassage {
        SourcePassage {
            anchor: None,
            written_by_rebost: false,
            sid: sid.into(),
            document_id: format!("d_{title}"),
            shelf_id: "s".into(),
            title: title.into(),
            section: None,
            page_start: None,
            page_end: None,
            body: String::new(),
            path: String::new(),
            score: 1.0,
        }
    }

    #[test]
    fn older_registries_and_messages_still_load() {
        let meta: DocumentMeta = serde_json::from_str(
            r#"{"id":"d_1","shelfId":"s_1","sourceId":"imported","sourceType":"imported",
            "path":"/x/a.md","relPath":"a.md","fileName":"a.md","format":"md","sizeBytes":1,
            "hash":"","status":"ready","passageCount":1,"piiTotal":0,
            "updatedAt":"2026-01-01T00:00:00Z","sourceLabel":"Imported"}"#,
        )
        .unwrap();
        assert!(!meta.written_by_rebost);
        assert!(!serde_json::to_string(&meta)
            .unwrap()
            .contains("writtenByRebost"));
        let passage: SourcePassage = serde_json::from_str(
            r#"{"sid":"S1","documentId":"d_1","shelfId":"s_1","title":"a.md","path":"/x","score":1}"#,
        )
        .unwrap();
        assert!(!passage.written_by_rebost);
        let message: crate::chat::conversations::StoredMessage = serde_json::from_str(
            r#"{"id":"m_1","role":"assistant","text":"Hi","ts":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert!(message.saved.is_none());
    }

    #[test]
    fn names_collide_case_insensitively_and_count_up() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            unique_file_name(dir.path(), "Report", "docx"),
            "Report.docx"
        );
        std::fs::write(dir.path().join("report.DOCX"), "x").unwrap();
        assert_eq!(
            unique_file_name(dir.path(), "Report", "docx"),
            "Report-1.docx"
        );
        std::fs::write(dir.path().join("Report-1.docx"), "x").unwrap();
        assert_eq!(
            unique_file_name(dir.path(), "Report", "docx"),
            "Report-2.docx"
        );
        assert_eq!(unique_file_name(dir.path(), "Report", "md"), "Report.md");
    }

    #[test]
    fn sanitize_drops_paths_extensions_and_reserved_names() {
        assert_eq!(sanitize_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(
            sanitize_stem("C:\\Users\\me\\Plan.docx"),
            "C -Users-me-Plan"
        );
        assert_eq!(sanitize_stem("Budget 2027.md"), "Budget 2027");
        assert_eq!(sanitize_stem("notes.txt.docx"), "notes");
        assert_eq!(sanitize_stem("Release v1.2"), "Release v1.2");
        assert_eq!(sanitize_stem("  trailing dots...  "), "trailing dots");
        assert_eq!(sanitize_stem("bell\u{0007}ring\n"), "bell ring");
        assert_eq!(sanitize_stem("Q3: results?"), "Q3 results");
        assert_eq!(sanitize_stem("CON"), "CON_");
        assert_eq!(sanitize_stem("nul.txt"), "nul_");
        assert_eq!(sanitize_stem("com1.report"), "com1_.report");
        assert_eq!(sanitize_stem("Console"), "Console");
        assert_eq!(sanitize_stem(".hidden"), "hidden");
        assert_eq!(sanitize_stem(".."), "Document");
        assert_eq!(sanitize_stem(""), "Document");
        let long = sanitize_stem(&"é".repeat(300));
        assert!(long.len() <= STEM_MAX_BYTES && !long.is_empty());
        assert!(!sanitize_stem("a/b\\c").contains(['/', '\\']));
    }

    #[test]
    fn save_is_offered_only_when_the_message_asks_for_a_file() {
        assert!(save_intent("Write a cover letter for the Barcelona job"));
        assert!(save_intent("Redacta un informe sobre el contracte"));
        assert!(save_intent("Fes-me un resum de les notes"));
        assert!(save_intent("報告書を作成して"));
        assert!(!save_intent("When is the rent due?"));
        assert!(!save_intent("¿Cuánto cuesta el alquiler?"));
    }

    #[test]
    fn format_parses_aliases_and_trailing_extensions() {
        assert_eq!(DocFormat::parse("Markdown"), Some(DocFormat::Md));
        assert_eq!(DocFormat::parse(".TXT"), Some(DocFormat::Txt));
        assert_eq!(DocFormat::parse("word"), Some(DocFormat::Docx));
        assert_eq!(DocFormat::parse("pdf"), None);
        assert_eq!(format_from_name("Plan.md"), Some(DocFormat::Md));
        assert_eq!(format_from_name("Plan"), None);
    }

    #[test]
    fn citations_become_a_sources_list() {
        let sources = [source("S1", "Lease.pdf"), source("S2", "Notes.md")];
        let text = document_text(
            "# Summary\n\nRent is 900 [S1]. Pets are allowed [S2][S1].\n\nUnknown [S9].",
            &sources,
        );
        assert!(!text.contains("[S"), "{text}");
        assert!(text.contains("Rent is 900. Pets are allowed."), "{text}");
        assert!(text.contains("- Lease.pdf\n- Notes.md"), "{text}");
        let plain = document_text("No citations here.", &sources);
        assert_eq!(plain, "No citations here.\n");
    }

    #[test]
    fn button_names_come_from_the_first_heading_or_line() {
        assert_eq!(
            default_stem("# Cover letter\n\nDear Ana,", "Chat"),
            "Cover letter"
        );
        assert_eq!(
            default_stem("**Rent summary** [S1]\n\nRent is 900.", "Chat"),
            "Rent summary"
        );
        assert_eq!(default_stem("   ", "Office move"), "Office move");
        let long = default_stem(&"word ".repeat(40), "x");
        assert!(long.chars().count() <= 60, "{long}");
    }

    #[test]
    fn a_fenced_document_is_unwrapped() {
        let text = document_text("```markdown\n# Letter\n\nDear Ana,\n```", &[]);
        assert_eq!(text, "# Letter\n\nDear Ana,\n");
        let code = document_text("```rust\nfn main() {}\n```", &[]);
        assert!(code.starts_with("```rust"));
    }

    #[test]
    fn plain_text_drops_markdown_syntax() {
        let plain = markdown_to_plain(
            "# Title\n\nSome **bold** text with [a link](https://x.org).\n\n- one\n- two\n\n1. first\n2. second\n\n| A | B |\n|---|---|\n| 1 | 2 |\n",
        );
        assert!(
            plain.starts_with("Title\n\nSome bold text with a link (https://x.org)."),
            "{plain}"
        );
        assert!(plain.contains("- one\n- two"), "{plain}");
        assert!(plain.contains("1. first\n2. second"), "{plain}");
        assert!(plain.contains("A\tB\n1\t2"), "{plain}");
        assert!(!plain.contains('#') && !plain.contains("**"));
    }

    #[test]
    fn saves_at_the_shelf_root_without_overwriting_and_marks_provenance() {
        let (_dir, ctx, shelf) = setup();
        let existing = shelf.managed_path.join("Plan.md");
        std::fs::write(&existing, "keep me").unwrap();
        let (saved, job) = save_answer(
            &ctx,
            &shelf.id,
            "../Plan.docx",
            DocFormat::Md,
            "# Plan\n\nStep one [S1].",
            &[source("S1", "Brief.pdf")],
        );
        assert_eq!(saved.status, SaveStatus::Saved, "{saved:?}");
        assert_eq!(saved.file_name, "Plan-1.md");
        let path = PathBuf::from(saved.path.as_deref().unwrap());
        assert_eq!(path.parent().unwrap(), shelf.managed_path);
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "keep me");
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("Step one.") && written.contains("- Brief.pdf"));
        let meta = crate::core::read_lock(&ctx.library)
            .document(&shelf.id, saved.document_id.as_deref().unwrap())
            .unwrap();
        assert!(meta.written_by_rebost);
        assert_eq!(meta.rel_path, "Plan-1.md");
        let job = job.unwrap();
        assert_eq!(job.abs_path, path);
        assert_eq!(job.source_id, Shelf::IMPORTED_SOURCE);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_saved_docx_is_read_like_any_other_file_and_keeps_its_mark() {
        let (_dir, ctx, shelf) = setup();
        let (saved, job) = save_answer(
            &ctx,
            &shelf.id,
            "Kitchen rota",
            DocFormat::Docx,
            "# Kitchen rota\n\n- Monday: Ana\n- Tuesday: Joan\n",
            &[],
        );
        assert_eq!(saved.file_name, "Kitchen rota.docx");
        crate::ingest::process_file(&ctx, &job.unwrap())
            .await
            .unwrap();
        let meta = crate::core::read_lock(&ctx.library)
            .document(&shelf.id, saved.document_id.as_deref().unwrap())
            .unwrap();
        assert_eq!(meta.status, DocStatus::Ready, "{meta:?}");
        assert!(meta.written_by_rebost, "ingest keeps the mark");
        let text = std::fs::read_to_string(ctx.paths.extracted_path(&shelf.id, &meta.id)).unwrap();
        assert!(text.contains("Tuesday: Joan"), "{text}");

        undo(&ctx, &saved).unwrap();
        assert!(!PathBuf::from(saved.path.unwrap()).exists());
        assert!(crate::core::read_lock(&ctx.library)
            .document(&shelf.id, &meta.id)
            .is_none());
    }

    #[test]
    fn undo_refuses_an_edited_file_and_paths_outside_the_shelf() {
        let (dir, ctx, shelf) = setup();
        let (saved, _) = save_answer(&ctx, &shelf.id, "Memo", DocFormat::Txt, "Hello.", &[]);
        let path = PathBuf::from(saved.path.clone().unwrap());
        std::fs::write(&path, "Edited by hand").unwrap();
        assert!(undo(&ctx, &saved).is_err());
        assert!(path.exists());

        let outside = dir.path().join("elsewhere.txt");
        std::fs::write(&outside, "not ours").unwrap();
        let mut forged = saved.clone();
        forged.path = Some(outside.to_string_lossy().to_string());
        forged.hash = None;
        assert!(undo(&ctx, &forged).is_err());
        assert!(outside.exists());
    }

    #[test]
    fn a_full_shelf_or_an_upload_shelf_is_refused() {
        let (_dir, ctx, shelf) = setup();
        {
            let mut library = crate::core::write_lock(&ctx.library);
            for i in 0..crate::shelf::DEFAULT_FILE_LIMIT {
                library.upsert_document(DocumentMeta {
                    id: format!("d_fill{i}"),
                    written_by_rebost: false,
                    shelf_id: shelf.id.clone(),
                    source_id: Shelf::IMPORTED_SOURCE.into(),
                    source_type: SourceType::Imported,
                    path: String::new(),
                    rel_path: format!("fill{i}.md"),
                    file_name: format!("fill{i}.md"),
                    format: "md".into(),
                    size_bytes: 1,
                    mtime_ms: 0,
                    hash: String::new(),
                    status: DocStatus::Ready,
                    error: None,
                    passage_count: 1,
                    pages: None,
                    pii_total: 0,
                    pii_categories: Default::default(),
                    ocr: false,
                    updated_at: "2026-01-01T00:00:00Z".into(),
                    source_label: "Imported".into(),
                });
            }
        }
        let (full, job) = save_answer(&ctx, &shelf.id, "one more", DocFormat::Txt, "x", &[]);
        assert_eq!(full.status, SaveStatus::Failed);
        assert_eq!(full.error.as_deref(), Some(ERROR_SHELF_FULL));
        assert!(job.is_none());

        let upload = crate::core::write_lock(&ctx.library)
            .ensure_conversation_shelf(&ctx.paths, &crate::ids::thread_id())
            .unwrap();
        let (refused, _) = save_answer(&ctx, &upload.id, "x", DocFormat::Txt, "x", &[]);
        assert_eq!(refused.status, SaveStatus::Failed);
        assert!(std::fs::read_dir(&upload.managed_path)
            .unwrap()
            .flatten()
            .all(|e| e.file_name() == "shelf.yml"));
    }
}
