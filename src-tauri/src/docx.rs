//! Minimal Markdown → Word (.docx) writer for documents Rebost saves to a Shelf.
//!
//! Pure Rust and dependency-light: `pulldown-cmark` parses the Markdown and
//! `zip` packs a small, valid OOXML package:
//!
//! ```text
//! [Content_Types].xml
//! _rels/.rels
//! docProps/core.xml          title (first H1), creator
//! word/document.xml          body
//! word/_rels/document.xml.rels   styles, numbering, external links
//! word/styles.xml            Title, Heading 1–3, List Paragraph, Quote, Code, Table Grid
//! word/numbering.xml         bullets and restartable numbered lists (nested)
//! ```
//!
//! Supported: headings, paragraphs, bold / italic / strikethrough / inline
//! code, bullet and numbered lists (nested), tables, links, block quotes,
//! code blocks, horizontal rules. Images keep their alt text only.

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use anyhow::Result;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MAX_LEVEL: usize = 8;
/// Usable width of an A4 page with 1" margins, in twentieths of a point.
const TEXT_WIDTH_TWIPS: usize = 9026;

/// Convert Markdown to a `.docx` file. `fallback_title` is used for the
/// document properties when the text has no level-1 heading.
pub(crate) fn markdown_to_docx(markdown: &str, fallback_title: &str) -> Result<Vec<u8>> {
    let created = jiff::Timestamp::now()
        .strftime("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    build_docx(markdown, fallback_title, &created)
}

fn build_docx(markdown: &str, fallback_title: &str, created: &str) -> Result<Vec<u8>> {
    let mut builder = Builder::default();
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    for event in Parser::new_ext(markdown, options) {
        builder.event(event);
    }
    builder.finish();
    let title = builder
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(fallback_title.trim())
        .to_string();

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let parts: [(&str, String); 7] = [
        ("[Content_Types].xml", content_types()),
        ("_rels/.rels", package_rels()),
        ("docProps/core.xml", core_props(&title, created)),
        ("word/document.xml", document_xml(&builder.body)),
        (
            "word/_rels/document.xml.rels",
            document_rels(&builder.links),
        ),
        ("word/styles.xml", styles_xml()),
        ("word/numbering.xml", numbering_xml(&builder.ordered_starts)),
    ];
    for (name, xml) in parts {
        zip.start_file(name, options)?;
        zip.write_all(xml.as_bytes())?;
    }
    Ok(zip.finish()?.into_inner())
}

#[derive(Default)]
struct Para {
    style: Option<&'static str>,
    num: Option<(usize, usize)>,
    indent: Option<usize>,
    jc: Option<&'static str>,
    runs: String,
}

struct ListCtx {
    num_id: usize,
    /// The next paragraph in this item carries the bullet or number.
    needs_marker: bool,
}

#[derive(Default)]
struct TableCtx {
    aligns: Vec<Alignment>,
    rows: String,
    row: String,
    cells_in_row: usize,
    in_head: bool,
    cell: Option<String>,
}

#[derive(Default)]
struct Builder {
    body: String,
    /// External link targets, in relationship order.
    links: Vec<String>,
    /// Start value for each numbered list; list `i` uses numId `2 + i`.
    ordered_starts: Vec<u64>,
    para: Option<Para>,
    lists: Vec<ListCtx>,
    quote_depth: usize,
    code: Option<String>,
    table: Option<TableCtx>,
    bold: usize,
    italic: usize,
    strike: usize,
    sup: usize,
    sub: usize,
    in_link: Vec<bool>,
    heading: Option<usize>,
    title: Option<String>,
    collecting_title: bool,
    wrote_block: bool,
    last_was_table: bool,
}

impl Builder {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some(code) = self.code.as_mut() {
                    code.push_str(&text);
                } else {
                    self.text(&text, false);
                }
            }
            Event::Code(text) => self.text(&text, true),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.text(&text, false),
            Event::Html(html) | Event::InlineHtml(html) => self.html(&html),
            Event::FootnoteReference(name) => self.text(&format!("[{name}]"), false),
            Event::SoftBreak => self.text(" ", false),
            Event::HardBreak => {
                self.ensure_para();
                if let Some(para) = self.para.as_mut() {
                    para.runs.push_str("<w:r><w:br/></w:r>");
                }
            }
            Event::Rule => {
                self.close_para();
                self.push_block(
                    "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" \
w:color=\"auto\"/></w:pBdr></w:pPr></w:p>",
                );
            }
            Event::TaskListMarker(done) => self.text(if done { "☒ " } else { "☐ " }, false),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock | Tag::FootnoteDefinition(_) => {
                self.close_para();
                self.open_para(None);
            }
            Tag::Heading { level, .. } => {
                self.close_para();
                let level = heading_number(level);
                let style = if level == 1 && self.title.is_none() {
                    self.collecting_title = true;
                    self.title = Some(String::new());
                    if self.wrote_block {
                        "Heading1"
                    } else {
                        "Title"
                    }
                } else {
                    match level {
                        1 => "Heading1",
                        2 => "Heading2",
                        _ => "Heading3",
                    }
                };
                self.heading = Some(level);
                self.open_para(Some(style));
            }
            Tag::BlockQuote(_) => {
                self.close_para();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(_) => {
                self.close_para();
                self.code = Some(String::new());
            }
            Tag::List(start) => {
                self.close_para();
                let num_id = match start {
                    Some(first) => {
                        self.ordered_starts.push(first);
                        1 + self.ordered_starts.len()
                    }
                    None => 1,
                };
                self.lists.push(ListCtx {
                    num_id,
                    needs_marker: false,
                });
            }
            Tag::Item => {
                self.close_para();
                if let Some(list) = self.lists.last_mut() {
                    list.needs_marker = true;
                }
            }
            Tag::Table(aligns) => {
                self.close_para();
                self.table = Some(TableCtx {
                    aligns,
                    ..TableCtx::default()
                });
            }
            Tag::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.in_head = true;
                    table.row.clear();
                    table.cells_in_row = 0;
                }
            }
            Tag::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    table.row.clear();
                    table.cells_in_row = 0;
                }
            }
            Tag::TableCell => {
                let jc = self.table.as_mut().and_then(|table| {
                    table.cell = Some(String::new());
                    match table.aligns.get(table.cells_in_row) {
                        Some(Alignment::Center) => Some("center"),
                        Some(Alignment::Right) => Some("right"),
                        _ => None,
                    }
                });
                self.para = Some(Para {
                    jc,
                    ..Para::default()
                });
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Superscript => self.sup += 1,
            Tag::Subscript => self.sub += 1,
            Tag::Link { dest_url, .. } => {
                self.ensure_para();
                let external = is_external_link(&dest_url);
                if external {
                    self.links.push(dest_url.to_string());
                    let id = link_rel_id(self.links.len() - 1);
                    if let Some(para) = self.para.as_mut() {
                        let _ = write!(para.runs, "<w:hyperlink r:id=\"{id}\" w:history=\"1\">");
                    }
                }
                self.in_link.push(external);
            }
            Tag::Image { .. }
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock | TagEnd::FootnoteDefinition => self.close_para(),
            TagEnd::Heading(_) => {
                self.close_para();
                self.heading = None;
                self.collecting_title = false;
            }
            TagEnd::BlockQuote(_) => {
                self.close_para();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                let code = self.code.take().unwrap_or_default();
                let mut runs = String::new();
                for (i, line) in code.trim_end_matches('\n').split('\n').enumerate() {
                    if i > 0 {
                        runs.push_str("<w:r><w:br/></w:r>");
                    }
                    push_text_run(&mut runs, "", line);
                }
                self.para = Some(Para {
                    style: Some("Code"),
                    runs,
                    ..Para::default()
                });
                self.close_para();
            }
            TagEnd::List(_) => {
                self.close_para();
                self.lists.pop();
            }
            TagEnd::Item => self.close_para(),
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    let cols = table.aligns.len().max(1);
                    let width = TEXT_WIDTH_TWIPS / cols;
                    let mut xml = String::from(
                        "<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/>\
<w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblLook w:val=\"04A0\" w:firstRow=\"1\" \
w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/>\
</w:tblPr><w:tblGrid>",
                    );
                    for _ in 0..cols {
                        let _ = write!(xml, "<w:gridCol w:w=\"{width}\"/>");
                    }
                    xml.push_str("</w:tblGrid>");
                    xml.push_str(&table.rows);
                    xml.push_str("</w:tbl>");
                    self.push_block(&xml);
                    self.last_was_table = true;
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    let cols = table.aligns.len().max(1);
                    while table.cells_in_row < cols {
                        table.row.push_str(
                            "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr><w:p/></w:tc>",
                        );
                        table.cells_in_row += 1;
                    }
                    let header = if table.in_head {
                        "<w:trPr><w:tblHeader/></w:trPr>"
                    } else {
                        ""
                    };
                    let _ = write!(table.rows, "<w:tr>{header}{}</w:tr>", table.row);
                    table.row.clear();
                    table.in_head = false;
                }
            }
            TagEnd::TableCell => {
                self.close_para();
                if let Some(table) = self.table.as_mut() {
                    let cell = table.cell.take().unwrap_or_default();
                    let cell = if cell.is_empty() {
                        "<w:p/>".to_string()
                    } else {
                        cell
                    };
                    let _ = write!(
                        table.row,
                        "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr>{cell}</w:tc>"
                    );
                    table.cells_in_row += 1;
                }
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Superscript => self.sup = self.sup.saturating_sub(1),
            TagEnd::Subscript => self.sub = self.sub.saturating_sub(1),
            TagEnd::Link => {
                if self.in_link.pop() == Some(true) {
                    if let Some(para) = self.para.as_mut() {
                        para.runs.push_str("</w:hyperlink>");
                    }
                }
            }
            TagEnd::Image
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn html(&mut self, html: &str) {
        let lower = html.trim().to_ascii_lowercase();
        if lower.starts_with("<br") {
            self.event(Event::HardBreak);
            return;
        }
        let text = strip_tags(html);
        if !text.trim().is_empty() {
            self.text(&text, false);
        }
    }

    fn text(&mut self, text: &str, code: bool) {
        if text.is_empty() {
            return;
        }
        if self.collecting_title {
            if let Some(title) = self.title.as_mut() {
                title.push_str(text);
            }
        }
        self.ensure_para();
        let props = self.run_props(code);
        if let Some(para) = self.para.as_mut() {
            push_text_run(&mut para.runs, &props, text);
        }
    }

    fn run_props(&self, code: bool) -> String {
        let mut props = String::new();
        if self.in_link.last() == Some(&true) {
            props.push_str("<w:rStyle w:val=\"Hyperlink\"/>");
        } else if code {
            props.push_str("<w:rStyle w:val=\"VerbatimChar\"/>");
        }
        let head = self.table.as_ref().is_some_and(|t| t.in_head);
        if self.bold > 0 || head {
            props.push_str("<w:b/>");
        }
        if self.italic > 0 {
            props.push_str("<w:i/>");
        }
        if self.strike > 0 {
            props.push_str("<w:strike/>");
        }
        if self.sup > 0 {
            props.push_str("<w:vertAlign w:val=\"superscript\"/>");
        } else if self.sub > 0 {
            props.push_str("<w:vertAlign w:val=\"subscript\"/>");
        }
        props
    }

    fn ensure_para(&mut self) {
        if self.para.is_none() {
            self.open_para(None);
        }
    }

    fn open_para(&mut self, style: Option<&'static str>) {
        if self.table.as_ref().is_some_and(|t| t.cell.is_some()) {
            self.para = Some(Para::default());
            return;
        }
        let mut para = Para {
            style,
            ..Para::default()
        };
        let depth = self.lists.len();
        if let Some(list) = self.lists.last_mut() {
            if style.is_none() {
                para.style = Some("ListParagraph");
            }
            let level = (depth - 1).min(MAX_LEVEL);
            if list.needs_marker {
                list.needs_marker = false;
                para.num = Some((list.num_id, level));
            } else {
                para.indent = Some(720 * (level + 1));
            }
        } else if style.is_none() && self.quote_depth > 0 {
            para.style = Some("Quote");
        }
        self.para = Some(para);
    }

    fn close_para(&mut self) {
        let Some(para) = self.para.take() else {
            return;
        };
        let mut xml = String::from("<w:p>");
        let mut ppr = String::new();
        if let Some(style) = para.style {
            let _ = write!(ppr, "<w:pStyle w:val=\"{style}\"/>");
        }
        if let Some((num_id, level)) = para.num {
            let _ = write!(
                ppr,
                "<w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"{num_id}\"/></w:numPr>"
            );
        }
        if let Some(left) = para.indent {
            let _ = write!(ppr, "<w:ind w:left=\"{left}\"/>");
        }
        if let Some(jc) = para.jc {
            let _ = write!(ppr, "<w:jc w:val=\"{jc}\"/>");
        }
        if !ppr.is_empty() {
            let _ = write!(xml, "<w:pPr>{ppr}</w:pPr>");
        }
        xml.push_str(&para.runs);
        // An unterminated link (malformed input) must not leave the XML open.
        let opened = para.runs.matches("<w:hyperlink ").count();
        let closed = para.runs.matches("</w:hyperlink>").count();
        for _ in closed..opened {
            xml.push_str("</w:hyperlink>");
        }
        xml.push_str("</w:p>");
        if let Some(cell) = self.table.as_mut().and_then(|t| t.cell.as_mut()) {
            cell.push_str(&xml);
        } else {
            self.push_block(&xml);
        }
    }

    fn push_block(&mut self, xml: &str) {
        self.body.push_str(xml);
        self.wrote_block = true;
        self.last_was_table = false;
    }

    fn finish(&mut self) {
        self.close_para();
        if let Some(code) = self.code.take() {
            let mut runs = String::new();
            push_text_run(&mut runs, "", &code);
            self.para = Some(Para {
                style: Some("Code"),
                runs,
                ..Para::default()
            });
            self.close_para();
        }
        // Word wants a paragraph between a table and the section properties.
        if self.last_was_table || self.body.is_empty() {
            self.push_block("<w:p/>");
        }
    }
}

fn heading_number(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn is_external_link(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

fn link_rel_id(index: usize) -> String {
    format!("rIdLink{}", index + 1)
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// One run per text piece; tabs and newlines become Word tabs and breaks.
fn push_text_run(out: &mut String, props: &str, text: &str) {
    out.push_str("<w:r>");
    if !props.is_empty() {
        let _ = write!(out, "<w:rPr>{props}</w:rPr>");
    }
    let mut piece = String::new();
    let flush = |out: &mut String, piece: &mut String| {
        if !piece.is_empty() {
            let _ = write!(
                out,
                "<w:t xml:space=\"preserve\">{}</w:t>",
                xml_escape(piece)
            );
            piece.clear();
        }
    };
    for ch in text.chars() {
        match ch {
            '\t' => {
                flush(out, &mut piece);
                out.push_str("<w:tab/>");
            }
            '\n' => {
                flush(out, &mut piece);
                out.push_str("<w:br/>");
            }
            '\r' => {}
            ch => piece.push(ch),
        }
    }
    flush(out, &mut piece);
    out.push_str("</w:r>");
}

/// Escape for XML text and attribute values, dropping characters XML 1.0
/// cannot carry at all.
pub(crate) fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(ch),
            c if (c as u32) < 0x20 => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

const XML_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

fn content_types() -> String {
    format!(
        "{XML_HEAD}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
</Types>"
    )
}

fn package_rels() -> String {
    format!(
        "{XML_HEAD}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
</Relationships>"
    )
}

fn core_props(title: &str, created: &str) -> String {
    format!(
        "{XML_HEAD}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title><dc:creator>Rebost</dc:creator><cp:lastModifiedBy>Rebost</cp:lastModifiedBy>\
<dcterms:created xsi:type=\"dcterms:W3CDTF\">{created}</dcterms:created>\
<dcterms:modified xsi:type=\"dcterms:W3CDTF\">{created}</dcterms:modified>\
</cp:coreProperties>",
        xml_escape(title)
    )
}

fn document_xml(body: &str) -> String {
    format!(
        "{XML_HEAD}<w:document xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\"><w:body>{body}\
<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" \
w:bottom=\"1440\" w:left=\"1440\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr>\
</w:body></w:document>"
    )
}

fn document_rels(links: &[String]) -> String {
    let mut xml = format!(
        "{XML_HEAD}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering\" Target=\"numbering.xml\"/>"
    );
    for (i, url) in links.iter().enumerate() {
        let _ = write!(
            xml,
            "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>",
            link_rel_id(i),
            xml_escape(url.trim())
        );
    }
    xml.push_str("</Relationships>");
    xml
}

fn styles_xml() -> String {
    let heading = |id: &str, name: &str, level: usize, size: usize, before: usize| {
        format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>\
<w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:uiPriority w:val=\"9\"/><w:qFormat/>\
<w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before=\"{before}\" w:after=\"80\"/>\
<w:outlineLvl w:val=\"{level}\"/></w:pPr><w:rPr><w:b/><w:color w:val=\"1F3864\"/>\
<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/></w:rPr></w:style>"
        )
    };
    format!(
        "{XML_HEAD}<w:styles xmlns:w=\"{W_NS}\">\
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" \
w:eastAsia=\"Calibri\" w:cs=\"Calibri\"/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/></w:rPr>\
</w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"160\" w:line=\"276\" \
w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/>\
<w:next w:val=\"Normal\"/><w:uiPriority w:val=\"10\"/><w:qFormat/><w:pPr><w:spacing w:after=\"240\"/></w:pPr>\
<w:rPr><w:sz w:val=\"52\"/><w:szCs w:val=\"52\"/></w:rPr></w:style>\
{}{}{}\
<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/>\
<w:basedOn w:val=\"Normal\"/><w:uiPriority w:val=\"34\"/><w:qFormat/><w:pPr><w:spacing w:after=\"60\"/>\
<w:ind w:left=\"720\"/><w:contextualSpacing/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Quote\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/>\
<w:next w:val=\"Normal\"/><w:uiPriority w:val=\"29\"/><w:qFormat/><w:pPr><w:pBdr><w:left w:val=\"single\" \
w:sz=\"12\" w:space=\"8\" w:color=\"A6A6A6\"/></w:pBdr><w:ind w:left=\"720\" w:right=\"720\"/></w:pPr>\
<w:rPr><w:i/><w:color w:val=\"404040\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:customStyle=\"1\" w:styleId=\"Code\"><w:name w:val=\"Code\"/>\
<w:basedOn w:val=\"Normal\"/><w:pPr><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F2F2F2\"/>\
<w:spacing w:after=\"160\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:rPr><w:rFonts w:ascii=\"Consolas\" \
w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/></w:rPr></w:style>\
<w:style w:type=\"character\" w:default=\"1\" w:styleId=\"DefaultParagraphFont\"><w:name w:val=\"Default Paragraph Font\"/>\
<w:uiPriority w:val=\"1\"/><w:semiHidden/></w:style>\
<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/>\
<w:basedOn w:val=\"DefaultParagraphFont\"/><w:uiPriority w:val=\"99\"/><w:rPr><w:color w:val=\"0563C1\"/>\
<w:u w:val=\"single\"/></w:rPr></w:style>\
<w:style w:type=\"character\" w:customStyle=\"1\" w:styleId=\"VerbatimChar\"><w:name w:val=\"Verbatim Char\"/>\
<w:basedOn w:val=\"DefaultParagraphFont\"/><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" \
w:cs=\"Consolas\"/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/></w:rPr></w:style>\
<w:style w:type=\"table\" w:default=\"1\" w:styleId=\"TableNormal\"><w:name w:val=\"Normal Table\"/>\
<w:uiPriority w:val=\"99\"/><w:semiHidden/><w:tblPr><w:tblInd w:w=\"0\" w:type=\"dxa\"/><w:tblCellMar>\
<w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"108\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/>\
<w:right w:w=\"108\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr></w:style>\
<w:style w:type=\"table\" w:styleId=\"TableGrid\"><w:name w:val=\"Table Grid\"/><w:basedOn w:val=\"TableNormal\"/>\
<w:uiPriority w:val=\"39\"/><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
<w:tblPr><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
<w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/><w:bottom w:val=\"single\" w:sz=\"4\" \
w:space=\"0\" w:color=\"auto\"/><w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
<w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/><w:insideV w:val=\"single\" w:sz=\"4\" \
w:space=\"0\" w:color=\"auto\"/></w:tblBorders></w:tblPr></w:style>\
</w:styles>",
        heading("Heading1", "heading 1", 0, 32, 360),
        heading("Heading2", "heading 2", 1, 26, 240),
        heading("Heading3", "heading 3", 2, 24, 200),
    )
}

fn numbering_xml(ordered_starts: &[u64]) -> String {
    const BULLETS: [&str; 3] = ["•", "◦", "▪"];
    const FORMATS: [&str; 3] = ["decimal", "lowerLetter", "lowerRoman"];
    let mut xml = format!("{XML_HEAD}<w:numbering xmlns:w=\"{W_NS}\">");
    xml.push_str(
        "<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"hybridMultilevel\"/>",
    );
    for level in 0..=MAX_LEVEL {
        let _ = write!(
            xml,
            "<w:lvl w:ilvl=\"{level}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/>\
<w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/>\
</w:pPr></w:lvl>",
            BULLETS[level % BULLETS.len()],
            720 * (level + 1)
        );
    }
    xml.push_str("</w:abstractNum>");
    xml.push_str(
        "<w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"hybridMultilevel\"/>",
    );
    for level in 0..=MAX_LEVEL {
        let _ = write!(
            xml,
            "<w:lvl w:ilvl=\"{level}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{}\"/>\
<w:lvlText w:val=\"%{}.\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/>\
</w:pPr></w:lvl>",
            FORMATS[level % FORMATS.len()],
            level + 1,
            720 * (level + 1)
        );
    }
    xml.push_str("</w:abstractNum>");
    xml.push_str("<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num>");
    for (i, start) in ordered_starts.iter().enumerate() {
        // Each numbered list restarts instead of continuing the previous one.
        let _ = write!(
            xml,
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"1\"/>",
            i + 2
        );
        for level in 0..=MAX_LEVEL {
            let _ = write!(
                xml,
                "<w:lvlOverride w:ilvl=\"{level}\"><w:startOverride w:val=\"{start}\"/></w:lvlOverride>"
            );
        }
        xml.push_str("</w:num>");
    }
    xml.push_str("</w:numbering>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    const SAMPLE: &str = "# Quarterly report\n\n\
Intro with **bold**, *italic*, ~~gone~~ and `code` & a < b \"quotes\".\n\n\
## Findings\n\n\
- First point\n- Second point\n  - Nested point\n\n\
1. Step one\n2. Step two\n\nBetween.\n\n\
3. Restarted at three\n\n\
| Name | Amount |\n| :--- | ---: |\n| Rent | 1,200 |\n| Food | 300 |\n\n\
> A quoted line.\n\n\
See [the site](https://example.com/?a=1&b=2) or [local](notes.md).\n\n\
---\n\n\
```\nlet x = 1;\nlet y = 2;\n```\n\n\
### Last words\n\nDone.\n";

    fn entries(bytes: &[u8]) -> std::collections::BTreeMap<String, String> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut out = std::collections::BTreeMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let mut text = String::new();
            file.read_to_string(&mut text).unwrap();
            out.insert(file.name().unwrap().to_string(), text);
        }
        out
    }

    fn assert_well_formed(name: &str, xml: &str) {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut depth = 0i32;
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(_)) => depth += 1,
                Ok(quick_xml::events::Event::End(_)) => depth -= 1,
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(error) => panic!("{name} is not well-formed: {error}\n{xml}"),
            }
            assert!(depth >= 0, "{name} closes more than it opens");
        }
        assert_eq!(depth, 0, "{name} leaves elements open");
    }

    #[test]
    fn package_has_every_part_and_well_formed_xml() {
        let bytes = build_docx(SAMPLE, "Fallback", "2026-11-25T10:00:00Z").unwrap();
        let parts = entries(&bytes);
        for name in [
            "[Content_Types].xml",
            "_rels/.rels",
            "docProps/core.xml",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
        ] {
            let xml = parts.get(name).unwrap_or_else(|| panic!("missing {name}"));
            assert_well_formed(name, xml);
        }
        let core = &parts["docProps/core.xml"];
        assert!(
            core.contains("<dc:title>Quarterly report</dc:title>"),
            "{core}"
        );
        let doc = &parts["word/document.xml"];
        assert!(doc.contains("<w:pStyle w:val=\"Title\"/>"));
        assert!(doc.contains("<w:pStyle w:val=\"Heading2\"/>"));
        assert!(doc.contains("<w:pStyle w:val=\"Heading3\"/>"));
        for escaped in ["&amp;", "&lt;", "&quot;quotes&quot;"] {
            assert!(doc.contains(escaped), "{escaped} in {doc}");
        }
        assert!(doc.contains("<w:b/>") && doc.contains("<w:i/>") && doc.contains("<w:strike/>"));
        assert!(doc.contains("<w:ilvl w:val=\"1\"/>"), "nested bullet");
        assert!(doc.contains("<w:tbl>") && doc.contains("<w:tblHeader/>"));
        assert!(doc.contains("<w:jc w:val=\"right\"/>"));
        assert!(doc.contains("<w:pStyle w:val=\"Quote\"/>"));
        assert!(doc.contains("<w:pStyle w:val=\"Code\"/>"));
        assert!(doc.contains("<w:pBdr><w:bottom"));
        assert!(doc.contains("<w:hyperlink r:id=\"rIdLink1\""));
        assert!(doc.contains("local"), "relative links keep their text");
        let rels = &parts["word/_rels/document.xml.rels"];
        assert!(rels.contains("https://example.com/?a=1&amp;b=2"), "{rels}");
        assert_eq!(rels.matches("TargetMode=\"External\"").count(), 1);
        let numbering = &parts["word/numbering.xml"];
        assert!(numbering.contains("<w:num w:numId=\"2\">"));
        assert!(
            numbering.contains("<w:startOverride w:val=\"3\"/>"),
            "{numbering}"
        );
        assert!(parts["word/styles.xml"].contains("w:styleId=\"Heading1\""));
    }

    #[test]
    fn title_falls_back_and_control_characters_are_dropped() {
        let bytes = build_docx(
            "Plain text\u{0007} only.",
            "My <notes>",
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        let parts = entries(&bytes);
        assert!(parts["docProps/core.xml"].contains("<dc:title>My &lt;notes&gt;</dc:title>"));
        let doc = &parts["word/document.xml"];
        assert!(doc.contains("Plain text only."));
        assert!(!doc.contains('\u{0007}'));
        assert_well_formed("document", doc);
    }

    #[test]
    fn heading_after_text_is_not_the_title_style() {
        let bytes = build_docx("Intro.\n\n# Later", "x", "2026-01-01T00:00:00Z").unwrap();
        let doc = &entries(&bytes)["word/document.xml"];
        assert!(doc.contains("<w:pStyle w:val=\"Heading1\"/>"));
        assert!(!doc.contains("<w:pStyle w:val=\"Title\"/>"));
    }

    #[test]
    fn empty_and_odd_input_still_makes_a_valid_document() {
        for input in [
            "",
            "| a |\n|---|\n| b |",
            "[unterminated](",
            "```\nno end",
            "<div>x</div>",
        ] {
            let bytes = build_docx(input, "Empty", "2026-01-01T00:00:00Z").unwrap();
            let parts = entries(&bytes);
            assert_well_formed(input, &parts["word/document.xml"]);
            assert!(parts["word/document.xml"].contains("<w:sectPr>"));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn xberg_reads_back_the_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, markdown_to_docx(SAMPLE, "Fallback").unwrap()).unwrap();
        let extraction = crate::ingest::extract::extract_file(
            &path,
            &crate::ingest::extract::ExtractorSettings::default(),
        )
        .await
        .unwrap();
        let text = extraction.content;
        for needle in [
            "Quarterly report",
            "Findings",
            "First point",
            "Nested point",
            "Step two",
            "Rent",
            "1,200",
            "A quoted line.",
            "the site",
            "let y = 2;",
            "Last words",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
    }
}
