//! Local retrieval: one Tantivy index for Shelf passages and older chat
//! messages. Lexical, deterministic, no embeddings.

pub mod gate;
mod query;
pub(crate) mod schema;
pub(crate) mod stems;

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use tantivy::schema::Field;
use tantivy::{doc, DateTime, Index, IndexReader, IndexWriter, ReloadPolicy, Term};

use crate::types::{DocumentMeta, Passage};
use schema::{build_schema, register_tokenizers, Fields, SCHEMA_VERSION, STEM_LANGS};

pub use schema::normalize_lang;

pub struct SearchIndex {
    pub(crate) index: Index,
    pub(crate) reader: IndexReader,
    pub(crate) writer: Mutex<IndexWriter>,
    pub(crate) fields: Fields,
}

impl SearchIndex {
    /// Open (or create) the application-level index at `dir`. The index is
    /// derived data: on schema mismatch it is wiped and rebuilt by callers.
    pub fn open(dir: &Path) -> Result<Self> {
        let version_file = dir.join("rebost-schema-version");
        let existing = std::fs::read_to_string(&version_file).ok();
        if existing.as_deref() != Some(SCHEMA_VERSION) && dir.join("meta.json").exists() {
            // Old layout — the index is rebuildable, start clean.
            std::fs::remove_dir_all(dir).ok();
        }
        std::fs::create_dir_all(dir)?;

        let (schema, _) = build_schema();
        let index = if dir.join("meta.json").exists() {
            Index::open_in_dir(dir).context("open tantivy index")?
        } else {
            let index =
                Index::create_in_dir(dir, schema.clone()).context("create tantivy index")?;
            std::fs::write(&version_file, SCHEMA_VERSION)?;
            index
        };
        register_tokenizers(&index);

        // Re-derive typed fields from the opened index's schema.
        let schema = index.schema();
        let field = |name: &str| -> Result<Field> {
            schema
                .get_field(name)
                .with_context(|| format!("missing field {name}"))
        };
        let mut stems = BTreeMap::new();
        for lang in STEM_LANGS {
            stems.insert(*lang, field(&format!("stem_{lang}"))?);
        }
        let fields = Fields {
            record_type: field("record_type")?,
            shelf_id: field("shelf_id")?,
            document_id: field("document_id")?,
            source_id: field("source_id")?,
            source_type: field("source_type")?,
            path: field("path")?,
            title: field("title")?,
            summary: field("summary")?,
            keywords: field("keywords")?,
            section: field("section")?,
            body: field("body")?,
            stems,
            page_start: field("page_start")?,
            page_end: field("page_end")?,
            language: field("language")?,
            quality: field("quality")?,
            thread_id: field("thread_id")?,
            message_id: field("message_id")?,
            role: field("role")?,
            created_at: field("created_at")?,
        };

        let writer = index.writer(64 * 1024 * 1024)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            writer: Mutex::new(writer),
            fields,
        })
    }

    /// Replace all passages of a document, then commit — each file becomes
    /// searchable on its own as soon as it finishes.
    pub fn index_document(
        &self,
        meta: &DocumentMeta,
        card_summary: &str,
        keywords: &[String],
        passages: &[Passage],
        language: Option<&str>,
        quality: &str,
        title: &str,
    ) -> Result<()> {
        let lang = language.and_then(normalize_lang);
        let keywords_joined = keywords.join(" ");
        {
            let writer = crate::core::mutex_lock(&self.writer);
            writer.delete_term(Term::from_field_text(self.fields.document_id, &meta.id));
            for passage in passages {
                let mut document = doc!(
                    self.fields.record_type => "passage",
                    self.fields.shelf_id => meta.shelf_id.as_str(),
                    self.fields.document_id => meta.id.as_str(),
                    self.fields.source_id => meta.source_id.as_str(),
                    self.fields.source_type => match meta.source_type {
                        crate::types::SourceType::Imported => "imported",
                        crate::types::SourceType::Linked => "linked",
                    },
                    self.fields.path => meta.path.as_str(),
                    self.fields.title => title,
                    self.fields.summary => card_summary,
                    self.fields.keywords => keywords_joined.as_str(),
                    self.fields.body => passage.body.as_str(),
                    self.fields.quality => quality,
                );
                if let Some(section) = &passage.section {
                    document.add_text(self.fields.section, section);
                }
                if let Some(p) = passage.page_start {
                    document.add_u64(self.fields.page_start, p as u64);
                }
                if let Some(p) = passage.page_end {
                    document.add_u64(self.fields.page_end, p as u64);
                }
                if let Some(lang) = lang {
                    document.add_text(self.fields.language, lang);
                    if let Some(stem_field) = self.fields.stems.get(lang) {
                        let stemmed_input = format!(
                            "{title}\n{}\n{keywords_joined}\n{}",
                            passage.section.as_deref().unwrap_or(""),
                            passage.body
                        );
                        document.add_text(*stem_field, &stemmed_input);
                    }
                }
                writer.add_document(document)?;
            }
        }
        self.commit()
    }

    /// Index one conversation message for older-conversation memory.
    pub fn index_message(
        &self,
        thread_id: &str,
        message_id: &str,
        role: &str,
        text: &str,
        language: Option<&str>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let lang = language.and_then(normalize_lang);
        {
            let writer = crate::core::mutex_lock(&self.writer);
            writer.delete_term(Term::from_field_text(self.fields.message_id, message_id));
            let mut document = doc!(
                self.fields.record_type => "message",
                self.fields.thread_id => thread_id,
                self.fields.message_id => message_id,
                self.fields.role => role,
                self.fields.body => text,
                self.fields.created_at => DateTime::from_timestamp_secs(created_at.timestamp()),
            );
            if let Some(lang) = lang {
                document.add_text(self.fields.language, lang);
                if let Some(stem_field) = self.fields.stems.get(lang) {
                    document.add_text(*stem_field, text);
                }
            }
            writer.add_document(document)?;
        }
        self.commit()
    }

    pub fn remove_document(&self, document_id: &str) -> Result<()> {
        {
            let writer = crate::core::mutex_lock(&self.writer);
            writer.delete_term(Term::from_field_text(self.fields.document_id, document_id));
        }
        self.commit()
    }

    pub fn remove_documents(&self, document_ids: &[String]) -> Result<()> {
        {
            let writer = crate::core::mutex_lock(&self.writer);
            for id in document_ids {
                writer.delete_term(Term::from_field_text(self.fields.document_id, id));
            }
        }
        self.commit()
    }

    pub fn remove_shelf(&self, shelf_id: &str) -> Result<()> {
        {
            let writer = crate::core::mutex_lock(&self.writer);
            writer.delete_term(Term::from_field_text(self.fields.shelf_id, shelf_id));
        }
        self.commit()
    }

    pub fn remove_thread(&self, thread_id: &str) -> Result<()> {
        {
            let writer = crate::core::mutex_lock(&self.writer);
            writer.delete_term(Term::from_field_text(self.fields.thread_id, thread_id));
        }
        self.commit()
    }

    fn commit(&self) -> Result<()> {
        crate::core::mutex_lock(&self.writer).commit()?;
        self.reader.reload()?;
        Ok(())
    }
}
