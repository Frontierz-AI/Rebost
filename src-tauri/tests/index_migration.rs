//! A real Tantivy 0.26 index must not strand cached files or older chat memory.

mod common;

use rebost::chat::conversations::{Conversations, StoredMessage};
use rebost::core::{Ctx, NoopEvents};
use rebost::ingest::extract::ExtractorSettings;
use rebost::ingest::{process_file, ProcessJob};
use rebost::paths::Paths;
use rebost::types::{DocStatus, DocumentMeta, SourceType};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

const PENDING: &str = "rebost-rebuild-pending";

fn open_ctx(base: &Path) -> Arc<Ctx> {
    Ctx::new(
        Paths::new(base.to_path_buf()),
        Arc::new(NoopEvents),
        ExtractorSettings::default(),
    )
    .unwrap()
}

fn install_v3_index(base: &Path) {
    let index = Paths::new(base.to_path_buf()).search_dir();
    std::fs::remove_dir_all(&index).unwrap();
    std::fs::create_dir_all(&index).unwrap();
    let archive = include_bytes!("fixtures/search-v3-tantivy-0.26.2.tar.gz");
    let gzip = flate2::read::GzDecoder::new(&archive[..]);
    tar::Archive::new(gzip).unpack(index).unwrap();
}

async fn linked_document(ctx: &Arc<Ctx>, external: &Path) -> (String, DocumentMeta) {
    std::fs::create_dir_all(external).unwrap();
    let external = external.canonicalize().unwrap();
    let file = common::fixture_contract_md(&external);
    let shelf = {
        let mut library = ctx.library.write().unwrap();
        let shelf = library
            .create_shelf(&ctx.paths, "Migration", &ctx.paths.library_dir())
            .unwrap();
        library.add_linked_folder(&shelf.id, &external).unwrap()
    };
    let source_id = rebost::ids::source_id(&external.to_string_lossy());
    let rel_path = file.file_name().unwrap().to_string_lossy().to_string();
    let id = rebost::ids::document_id(&shelf.id, &source_id, &rel_path);
    process_file(
        ctx,
        &ProcessJob {
            shelf_id: shelf.id.clone(),
            source_id,
            source_type: SourceType::Linked,
            source_label: "External drive".into(),
            abs_path: file,
            rel_path,
            force: false,
            epoch: 0,
        },
    )
    .await
    .unwrap();
    let document = ctx
        .library
        .read()
        .unwrap()
        .document(&shelf.id, &id)
        .unwrap();
    assert_eq!(document.status, DocStatus::Ready);
    (shelf.id, document)
}

fn message(text: &str, role: &str, status: &str) -> StoredMessage {
    StoredMessage {
        id: rebost::ids::message_id(),
        role: role.into(),
        text: text.into(),
        ts: "2026-08-01T12:00:00Z".into(),
        status: status.into(),
        images: Vec::new(),
        thinking: None,
        activity: Vec::new(),
        shelf_id: None,
        sources: Vec::new(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn v3_restores_disconnected_documents_and_saved_chats_without_changing_sources() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("app");
    let external = dir.path().join("external");
    let ctx = open_ctx(&base);
    let (shelf, doc) = linked_document(&ctx, &external).await;
    let thread = Conversations::create(&ctx.paths, Some(shelf.clone())).unwrap();
    for msg in [
        message("Migrationoranges need ninety days notice.", "user", "done"),
        message("Migrationoranges were approved.", "assistant", "stopped"),
        message(
            "Errormigrationsecret must stay out of memory.",
            "assistant",
            "error",
        ),
    ] {
        Conversations::append(&ctx.paths, &thread.id, &msg).unwrap();
    }
    let thread_path = ctx.paths.thread_path(&thread.id);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&thread_path)
        .unwrap()
        .write_all(b"{\"id\":\"crash-truncated")
        .unwrap();
    let cache_path = ctx.paths.extracted_path(&shelf, &doc.id);
    let card_path = ctx.paths.card_path(&shelf, &doc.id);
    let before = [
        std::fs::read(&thread_path).unwrap(),
        std::fs::read(&cache_path).unwrap(),
        std::fs::read(&card_path).unwrap(),
    ];
    drop(ctx);
    std::fs::rename(&external, dir.path().join("disconnected-drive")).unwrap();
    install_v3_index(&base);

    let ctx = open_ctx(&base);
    assert!(!external.exists());
    assert!(ctx.search.has_document(&doc.id));
    assert!(!ctx
        .search
        .search_passages("ninety days", &shelf, 8)
        .unwrap()
        .is_empty());
    let hits = ctx
        .search
        .search_messages(
            "Migrationoranges",
            None,
            Some(std::slice::from_ref(&thread.id)),
            &[],
            8,
        )
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits
        .iter()
        .all(|hit| hit.created_at == "2026-08-01T12:00:00+00:00"));
    assert!(ctx
        .search
        .search_messages("Errormigrationsecret", None, None, &[], 8)
        .unwrap()
        .is_empty());
    assert!(ctx
        .search
        .search_messages("legacy-index-only-sentinel", None, None, &[], 8)
        .unwrap()
        .is_empty());
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(
        std::fs::read_to_string(ctx.paths.search_dir().join("rebost-schema-version")).unwrap(),
        "rebost-search/v4"
    );
    assert_eq!(
        before,
        [
            std::fs::read(&thread_path).unwrap(),
            std::fs::read(&cache_path).unwrap(),
            std::fs::read(&card_path).unwrap()
        ]
    );
    let count = ctx.search.num_docs();
    drop(ctx);

    let reopened = open_ctx(&base);
    assert_eq!(
        reopened.search.num_docs(),
        count,
        "reopening must not duplicate restored entries"
    );
    assert_eq!(
        reopened
            .search
            .search_messages("Migrationoranges", None, Some(&[thread.id]), &[], 8)
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_utf8_records_do_not_abort_cached_document_and_chat_restoration() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("app");
    let external = dir.path().join("external");
    let ctx = open_ctx(&base);
    let (shelf, doc) = linked_document(&ctx, &external).await;
    let thread = Conversations::create(&ctx.paths, Some(shelf.clone())).unwrap();
    let first = message(
        "Migrationoranges before the damaged record.",
        "user",
        "done",
    );
    let last = message(
        "Migrationoranges after the damaged record.",
        "assistant",
        "done",
    );
    Conversations::append(&ctx.paths, &thread.id, &first).unwrap();
    let thread_path = ctx.paths.thread_path(&thread.id);
    let mut damaged = serde_json::to_vec(&message("Invalid bytes ?", "assistant", "done")).unwrap();
    *damaged.iter_mut().find(|byte| **byte == b'?').unwrap() = 0xff;
    damaged.push(b'\n');
    std::fs::OpenOptions::new()
        .append(true)
        .open(&thread_path)
        .unwrap()
        .write_all(&damaged)
        .unwrap();
    Conversations::append(&ctx.paths, &thread.id, &last).unwrap();
    // A crash also left the final record halfway through a four-byte UTF-8 character.
    std::fs::OpenOptions::new()
        .append(true)
        .open(&thread_path)
        .unwrap()
        .write_all(b"{\"text\":\"crash-truncated \xf0\x9f")
        .unwrap();
    let before = std::fs::read(&thread_path).unwrap();
    drop(ctx);
    std::fs::rename(&external, dir.path().join("disconnected-drive")).unwrap();
    install_v3_index(&base);

    let ctx = open_ctx(&base);
    assert!(!external.exists());
    assert!(ctx.search.has_document(&doc.id));
    assert!(!ctx
        .search
        .search_passages("ninety days", &shelf, 8)
        .unwrap()
        .is_empty());
    let hits = ctx
        .search
        .search_messages("Migrationoranges", None, Some(&[thread.id]), &[], 8)
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().any(|hit| hit.message_id == first.id));
    assert!(hits.iter().any(|hit| hit.message_id == last.id));
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(std::fs::read(thread_path).unwrap(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_chat_preserves_other_restored_entries_and_retries_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("app");
    let external = dir.path().join("external");
    let ctx = open_ctx(&base);
    let (shelf, doc) = linked_document(&ctx, &external).await;
    let damaged_thread = Conversations::create(&ctx.paths, Some(shelf.clone())).unwrap();
    let good_thread = Conversations::create(&ctx.paths, Some(shelf.clone())).unwrap();
    let delayed = message(
        "Migrationoranges recovered after the read error.",
        "user",
        "done",
    );
    let available = message(
        "Migrationoranges remain available during recovery.",
        "user",
        "done",
    );
    Conversations::append(&ctx.paths, &damaged_thread.id, &delayed).unwrap();
    Conversations::append(&ctx.paths, &good_thread.id, &available).unwrap();
    let damaged_path = ctx.paths.thread_path(&damaged_thread.id);
    let good_path = ctx.paths.thread_path(&good_thread.id);
    let damaged_bytes = std::fs::read(&damaged_path).unwrap();
    let good_bytes = std::fs::read(&good_path).unwrap();
    drop(ctx);
    std::fs::rename(&external, dir.path().join("disconnected-drive")).unwrap();
    install_v3_index(&base);
    std::fs::remove_file(&damaged_path).unwrap();
    // Unix can open a directory as a file, then reports an error on the first read.
    std::fs::create_dir(&damaged_path).unwrap();

    let ctx = open_ctx(&base);
    assert!(!external.exists());
    assert!(ctx.search.has_document(&doc.id));
    assert!(!ctx
        .search
        .search_passages("ninety days", &shelf, 8)
        .unwrap()
        .is_empty());
    let hits = ctx
        .search
        .search_messages("Migrationoranges", None, None, &[], 8)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message_id, available.id);
    assert!(ctx.paths.search_dir().join(PENDING).exists());
    let count = ctx.search.num_docs();
    drop(ctx);
    std::fs::remove_dir(&damaged_path).unwrap();
    std::fs::write(&damaged_path, &damaged_bytes).unwrap();

    let ctx = open_ctx(&base);
    assert!(ctx.search.has_document(&doc.id));
    let hits = ctx
        .search
        .search_messages("Migrationoranges", None, None, &[], 8)
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().any(|hit| hit.message_id == delayed.id));
    assert!(hits.iter().any(|hit| hit.message_id == available.id));
    assert_eq!(ctx.search.num_docs(), count + 1);
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(std::fs::read(damaged_path).unwrap(), damaged_bytes);
    assert_eq!(std::fs::read(good_path).unwrap(), good_bytes);
}

#[tokio::test(flavor = "multi_thread")]
async fn incomplete_restore_retries_after_a_missing_cache_is_recovered() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("app");
    let ctx = open_ctx(&base);
    let (shelf, doc) = linked_document(&ctx, &dir.path().join("external")).await;
    let cached = ctx.paths.extracted_path(&shelf, &doc.id);
    let text = std::fs::read(&cached).unwrap();
    let thread = Conversations::create(&ctx.paths, Some(shelf)).unwrap();
    Conversations::append(
        &ctx.paths,
        &thread.id,
        &message("Migrationoranges survive a retry.", "user", "done"),
    )
    .unwrap();
    drop(ctx);
    install_v3_index(&base);
    std::fs::remove_file(&cached).unwrap();

    let ctx = open_ctx(&base);
    assert!(ctx.paths.search_dir().join(PENDING).exists());
    assert!(!ctx.search.has_document(&doc.id));
    assert_eq!(
        ctx.search
            .search_messages("Migrationoranges", None, None, &[], 8)
            .unwrap()
            .len(),
        1
    );
    drop(ctx);
    std::fs::write(cached, text).unwrap();

    let ctx = open_ctx(&base);
    assert!(ctx.search.has_document(&doc.id));
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(
        ctx.search
            .search_messages("Migrationoranges", None, None, &[], 8)
            .unwrap()
            .len(),
        1,
        "retry must replace, not duplicate, previously restored messages"
    );
}

#[test]
fn absent_metadata_is_valid_for_a_new_install_and_an_empty_shelf() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = open_ctx(dir.path());
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert!(!ctx.paths.threads_index().exists());
    let shelf = ctx
        .library
        .write()
        .unwrap()
        .create_shelf(&ctx.paths, "Empty", &ctx.paths.library_dir())
        .unwrap();
    assert!(!ctx.paths.documents_registry(&shelf.id).exists());
    drop(ctx);
    install_v3_index(dir.path());

    let ctx = open_ctx(dir.path());
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(ctx.search.num_docs(), 0);
}

#[test]
fn unreadable_thread_inventory_retries_after_metadata_is_recovered() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = open_ctx(dir.path());
    let thread = Conversations::create(&ctx.paths, None).unwrap();
    Conversations::append(
        &ctx.paths,
        &thread.id,
        &message(
            "Migrationoranges survive metadata recovery.",
            "user",
            "done",
        ),
    )
    .unwrap();
    let index = ctx.paths.threads_index();
    let saved = std::fs::read(&index).unwrap();
    let jsonl = ctx.paths.thread_path(&thread.id);
    let messages = std::fs::read(&jsonl).unwrap();
    drop(ctx);
    install_v3_index(dir.path());
    std::fs::write(&index, "{interrupted").unwrap();
    std::fs::write(index.with_file_name("threads.json.bak"), "{interrupted").unwrap();

    let ctx = open_ctx(dir.path());
    assert!(ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(ctx.search.num_docs(), 0);
    drop(ctx);
    std::fs::write(&index, &saved).unwrap();

    let ctx = open_ctx(dir.path());
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(
        ctx.search
            .search_messages("Migrationoranges", None, None, &[], 8)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(std::fs::read(index).unwrap(), saved);
    assert_eq!(std::fs::read(jsonl).unwrap(), messages);
}

#[test]
fn thread_inventory_backup_restores_without_rewriting_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = open_ctx(dir.path());
    let thread = Conversations::create(&ctx.paths, None).unwrap();
    Conversations::append(
        &ctx.paths,
        &thread.id,
        &message("Migrationoranges survive backup recovery.", "user", "done"),
    )
    .unwrap();
    let index = ctx.paths.threads_index();
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
    metadata["threads"][0]
        .as_object_mut()
        .unwrap()
        .remove("avatarId");
    let saved = serde_json::to_vec(&metadata).unwrap();
    let backup = index.with_file_name("threads.json.bak");
    drop(ctx);
    install_v3_index(dir.path());
    std::fs::write(&index, "{interrupted").unwrap();
    std::fs::write(&backup, &saved).unwrap();

    let ctx = open_ctx(dir.path());
    assert!(!ctx.paths.search_dir().join(PENDING).exists());
    assert_eq!(
        ctx.search
            .search_messages("Migrationoranges", None, None, &[], 8)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(std::fs::read(index).unwrap(), b"{interrupted");
    assert_eq!(std::fs::read(backup).unwrap(), saved);
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_library_metadata_retries_with_the_linked_drive_offline() {
    for damaged in ["registry.json", "documents.json", "shelf.yml"] {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("app");
        let external = dir.path().join("external");
        let ctx = open_ctx(&base);
        let (shelf_id, doc) = linked_document(&ctx, &external).await;
        let metadata = match damaged {
            "registry.json" => ctx.paths.shelf_registry(),
            "documents.json" => ctx.paths.documents_registry(&shelf_id),
            _ => ctx
                .library
                .read()
                .unwrap()
                .shelf(&shelf_id)
                .unwrap()
                .managed_path
                .join("shelf.yml"),
        };
        let saved = std::fs::read(&metadata).unwrap();
        drop(ctx);
        std::fs::rename(&external, dir.path().join("disconnected-drive")).unwrap();
        install_v3_index(&base);
        std::fs::write(&metadata, "{interrupted").unwrap();
        if damaged.ends_with(".json") {
            std::fs::write(
                metadata.with_file_name(format!("{damaged}.bak")),
                "{interrupted",
            )
            .unwrap();
        }

        let ctx = open_ctx(&base);
        assert!(ctx.paths.search_dir().join(PENDING).exists(), "{damaged}");
        assert!(!ctx.search.has_document(&doc.id), "{damaged}");
        drop(ctx);
        std::fs::write(&metadata, &saved).unwrap();

        let ctx = open_ctx(&base);
        assert!(ctx.search.has_document(&doc.id), "{damaged}");
        assert!(!ctx.paths.search_dir().join(PENDING).exists(), "{damaged}");
        assert_eq!(std::fs::read(metadata).unwrap(), saved, "{damaged}");
    }
}
