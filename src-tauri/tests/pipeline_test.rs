//! Pipeline integration: real files through hash → Xberg → PII → Card →
//! passages → Tantivy, using the deterministic path only (no LLM).

mod common;

use common::*;
use rebost::types::DocStatus;

#[tokio::test(flavor = "multi_thread")]
async fn markdown_contract_becomes_ready_with_card_and_pii() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Contracts");
    let file = fixture_contract_md(app.dir.path());

    let meta = ingest_file(&app, &shelf_id, &file).await;
    assert_eq!(meta.status, DocStatus::Ready);
    assert!(meta.passage_count >= 3, "sections should become passages");
    assert!(!meta.ocr, "native text needs no OCR");

    // Card
    let card = rebost::ingest::card::read_card(&app.ctx.paths.card_path(&shelf_id, &meta.id))
        .expect("card written");
    assert_eq!(card.schema, "rebost-card/v1");
    assert!(card.title.contains("Santander"));
    assert_eq!(card.quality, "full");
    assert!(!card.summary.is_empty(), "TextRank summary expected");
    assert!(!card.keywords.is_empty(), "YAKE keywords expected");
    assert!(
        card.outline.iter().any(|o| o.title.contains("Termination")),
        "outline should list the Termination heading, got {:?}",
        card.outline
    );

    // Privacy Lens: IBAN + email present, values never stored.
    assert!(card.privacy.total >= 2);
    assert_eq!(card.privacy.categories.get("iban"), Some(&1));
    assert!(card.privacy.categories.get("email").copied().unwrap_or(0) >= 1);
    let card_text = std::fs::read_to_string(app.ctx.paths.card_path(&shelf_id, &meta.id)).unwrap();
    assert!(
        !card_text.contains("ES7620770024003102575766"),
        "cards must never store PII values"
    );

    // Extracted-text cache backs "View extracted text".
    let extracted =
        std::fs::read_to_string(app.ctx.paths.extracted_path(&shelf_id, &meta.id)).unwrap();
    assert!(extracted.contains("ninety (90) days"));
}

#[tokio::test(flavor = "multi_thread")]
async fn xlsx_payroll_counts_pii_per_sheet() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Finance");
    let file = fixture_payroll_xlsx(app.dir.path());

    let meta = ingest_file(&app, &shelf_id, &file).await;
    assert_eq!(meta.status, DocStatus::Ready, "error: {:?}", meta.error);
    assert!(meta.passage_count >= 1);

    let card = rebost::ingest::card::read_card(&app.ctx.paths.card_path(&shelf_id, &meta.id))
        .expect("card");
    assert_eq!(
        card.privacy.categories.get("nif"),
        Some(&3),
        "{:?}",
        card.privacy
    );
    assert_eq!(card.privacy.categories.get("iban"), Some(&3));
    assert_eq!(card.privacy.categories.get("email"), Some(&3));
}

#[tokio::test(flavor = "multi_thread")]
async fn pdf_with_native_text_extracts_pages() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Operations");
    let file = fixture_brief_pdf(app.dir.path());

    let meta = ingest_file(&app, &shelf_id, &file).await;
    assert_eq!(meta.status, DocStatus::Ready, "error: {:?}", meta.error);
    assert!(!meta.ocr, "typst PDFs carry a text layer");
    let extracted =
        std::fs::read_to_string(app.ctx.paths.extracted_path(&shelf_id, &meta.id)).unwrap();
    assert!(extracted.contains("18,000 EUR"));
}

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_files_are_skipped_and_changed_files_reprocessed() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Docs");
    let file = fixture_invoice_md(app.dir.path());

    let first = ingest_file(&app, &shelf_id, &file).await;
    let first_updated = first.updated_at.clone();

    // Same content → skip (updated_at unchanged).
    let second = ingest_file(&app, &shelf_id, &file).await;
    assert_eq!(second.updated_at, first_updated);

    // Changed content → same id, new hash, reprocessed.
    std::fs::write(
        &file,
        "# Invoice INV-2026-0042\n\nAmount due: 9,999.00 EUR\n",
    )
    .unwrap();
    let third = ingest_file(&app, &shelf_id, &file).await;
    assert_eq!(third.id, first.id);
    assert_ne!(third.hash, first.hash);
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_files_are_ready_not_errors() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Notes");

    // 0-byte file and a whitespace-only file — both fine, both Ready.
    let empty = app.dir.path().join("empty.md");
    std::fs::write(&empty, "").unwrap();
    let blank = app.dir.path().join("Changelog-PENDING.md");
    std::fs::write(&blank, "\n\n").unwrap();

    for file in [&empty, &blank] {
        let meta = ingest_file(&app, &shelf_id, file).await;
        assert_eq!(meta.status, DocStatus::Ready, "error: {:?}", meta.error);
        assert_eq!(meta.passage_count, 0);
        assert!(meta.error.is_none());
    }
    let stats = app.ctx.library.read().unwrap().stats(&shelf_id);
    assert_eq!(stats.searchable, 2);
    assert_eq!(stats.errors, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_files_stay_visible_with_error() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Broken");
    let file = app.dir.path().join("broken.pdf");
    std::fs::write(&file, b"this is not a pdf at all").unwrap();

    let meta = ingest_file(&app, &shelf_id, &file).await;
    assert_eq!(meta.status, DocStatus::Error);
    assert!(meta.error.is_some());
    // Not indexed as evidence.
    assert_eq!(meta.passage_count, 0);

    // The shelf table still shows it.
    let stats = app.ctx.library.read().unwrap().stats(&shelf_id);
    assert_eq!(stats.files, 1);
    assert_eq!(stats.errors, 1);
    assert_eq!(stats.searchable, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn removal_deletes_derived_data_only() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Cleanup");
    let file = fixture_invoice_md(app.dir.path());
    let meta = ingest_file(&app, &shelf_id, &file).await;

    let card_path = app.ctx.paths.card_path(&shelf_id, &meta.id);
    assert!(card_path.exists());

    rebost::ingest::remove_document(&app.ctx, &shelf_id, &meta.id);
    assert!(!card_path.exists());
    assert!(
        !app.ctx.paths.extracted_path(&shelf_id, &meta.id).exists(),
        "extracted cache removed"
    );
    // The original file is never touched.
    assert!(file.exists());
    assert!(app
        .ctx
        .library
        .read()
        .unwrap()
        .document(&shelf_id, &meta.id)
        .is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn csv_and_eml_become_ready() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Inbox");
    let csv = ingest_file(&app, &shelf_id, &fixture_clients_csv(app.dir.path())).await;
    assert_eq!(csv.status, DocStatus::Ready, "csv error: {:?}", csv.error);
    let eml = ingest_file(&app, &shelf_id, &fixture_notice_eml(app.dir.path())).await;
    assert_eq!(eml.status, DocStatus::Ready, "eml error: {:?}", eml.error);
    let extracted =
        std::fs::read_to_string(app.ctx.paths.extracted_path(&shelf_id, &eml.id)).unwrap();
    assert!(extracted.contains("ninety (90) days"));
}

#[tokio::test(flavor = "multi_thread")]
async fn html_page_becomes_ready() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Web");
    let meta = ingest_file(&app, &shelf_id, &fixture_notice_html(app.dir.path())).await;
    assert_eq!(
        meta.status,
        DocStatus::Ready,
        "html error: {:?}",
        meta.error
    );
    let extracted =
        std::fs::read_to_string(app.ctx.paths.extracted_path(&shelf_id, &meta.id)).unwrap();
    assert!(extracted.to_lowercase().contains("tuesday"));
}

#[tokio::test(flavor = "multi_thread")]
async fn docx_office_note_becomes_ready() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Office");
    let meta = ingest_file(&app, &shelf_id, &fixture_office_note_docx(app.dir.path())).await;
    assert_eq!(
        meta.status,
        DocStatus::Ready,
        "docx error: {:?}",
        meta.error
    );
    assert!(!meta.ocr, "docx has a text layer");
    let extracted =
        std::fs::read_to_string(app.ctx.paths.extracted_path(&shelf_id, &meta.id)).unwrap();
    assert!(
        extracted.contains("18,000 EUR") || extracted.contains("ninety"),
        "docx text missing, got {extracted:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn scanned_pdf_uses_ocr() {
    let app = test_app();
    let shelf_id = make_shelf(&app, "Scans");
    let meta = ingest_file(&app, &shelf_id, &fixture_scanned_pdf(app.dir.path())).await;
    assert_eq!(meta.status, DocStatus::Ready, "error: {:?}", meta.error);
    assert!(meta.ocr, "image-only PDF should go through OCR");
    let extracted =
        std::fs::read_to_string(app.ctx.paths.extracted_path(&shelf_id, &meta.id)).unwrap();
    assert!(
        extracted.to_uppercase().contains("REBOST"),
        "OCR should read the page, got {extracted:?}"
    );
}
