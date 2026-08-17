//! Dev utility: populate Rebost app data with demo content
//! so the UI can be exercised realistically. Run only while the app is
//! closed (the search index writer is exclusive).
//!
//!   cargo run --example seed -- [--model /path/to/tiny.gguf] [--fresh]

use rebost::core::{Ctx, NoopEvents};
use rebost::ingest::extract::ExtractorSettings;
use rebost::paths::Paths;
use rebost::reset::BUNDLE_IDENTIFIER;
use std::path::PathBuf;
use std::sync::Arc;

fn fixture(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, contents).unwrap();
    path
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let model_arg = args
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let fresh = args.iter().any(|a| a == "--fresh");

    let data_dir = dirs::data_dir().unwrap().join(BUNDLE_IDENTIFIER);
    let existing = match rebost::instance::try_acquire(&data_dir) {
        Err(rebost::instance::AcquireError::Busy) => {
            anyhow::bail!("Quit Rebost before seeding.");
        }
        other => other?,
    };
    let _lock = if fresh {
        drop(existing);
        if data_dir.exists() {
            rebost::reset::wipe_app_data_contents(&data_dir)?;
            println!("wiped {} (kept library/)", data_dir.display());
        }
        rebost::instance::try_acquire(&data_dir)?
    } else {
        existing
    };
    let paths = Paths::new(&data_dir);
    paths.ensure()?;

    let tessdata = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("tessdata");
    let ctx = Ctx::new(
        paths,
        Arc::new(NoopEvents),
        ExtractorSettings {
            tessdata_dir: Some(tessdata),
            timeout_secs: 120,
            ..Default::default()
        },
    )?;

    // Demo shelves use the same library folder as the app.
    let shelf_root = ctx.paths.library_dir();
    {
        let mut settings = ctx.settings.write().unwrap();
        settings.onboarding_done = true;
        if let Some(model_path) = &model_arg {
            let model_path = PathBuf::from(model_path);
            let file_name = model_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let dest = ctx.paths.models_dir().join(&file_name);
            if !dest.exists() {
                std::fs::copy(&model_path, &dest)?;
            }
            settings.active_model = Some(rebost::settings::ActiveModel {
                file: file_name,
                name: "Qwen3 0.6B".into(),
                source: "huggingface".into(),
                reference: "unsloth/Qwen3-0.6B-GGUF".into(),
                license: Some("Apache-2.0".into()),
                size_bytes: std::fs::metadata(&model_path)?.len(),
            });
            println!("active model set");
        }
    }
    ctx.save_settings();

    // Demo files. Synthetic personal information is intentional so Privacy
    // Lens has something to count; it is not anyone's real data.
    let staging = data_dir.join("seed-staging");
    std::fs::create_dir_all(&staging)?;
    let handbook = fixture(
        &staging,
        "Staff handbook.md",
        r#"# Staff handbook

## Kitchen

The office kitchen is restocked every Tuesday morning. Milk and coffee are
in the cupboard above the sink.

## Time off

Ask your manager at least two weeks ahead for more than three days off.
Contact: office@example.com.

## Building

The door code is not written down here. Reception: +34 612 345 678.
"#,
    );
    let notes = fixture(
        &staging,
        "Weekly standup 12 Aug.md",
        r#"# Weekly standup — 12 August

Present: Ana, Jordi, Marc.

## Done

- Shipped the handbook rewrite.
- Booked the painter for the office move.

## Next

- Jordi drafts the one-page brief for the office move.
- Ana confirms the kitchen restock with the supplier (ops@acme.example).

## Open

Should the move wait until after the August holiday?
"#,
    );
    let vacances = fixture(
        &staging,
        "Politica de vacances.md",
        r#"# Política de vacances i permisos

## Dies de vacances

Cada persona treballadora té dret a vint-i-tres (23) dies laborables de
vacances per any complet treballat. Les vacances es demanen amb un mínim de
quinze dies d'antelació al responsable d'equip.

## Permisos retribuïts

Es concedeixen permisos per matrimoni, naixement i trasllat de domicili
segons el conveni col·lectiu aplicable. Dubtes: rrhh@empresa.example.
"#,
    );
    let invoice = fixture(
        &staging,
        "Invoice INV-2026-0042.md",
        r#"# Invoice INV-2026-0042

Client: Acme Logistics SL
Date: 2026-07-31
Amount due: 4,850.00 EUR
Due date: 2026-08-30

Concept: July consulting retainer, 22 days on site.
IBAN: ES7620770024003102575766
"#,
    );

    // Spreadsheet with plenty of PII, and a PDF with a native text layer —
    // static fixtures shared with the test suite.
    let payroll = staging.join("Payroll July.xlsx");
    std::fs::write(
        &payroll,
        include_bytes!("../tests/fixtures/payroll-july.xlsx"),
    )?;
    let brief = staging.join("Office move brief.pdf");
    std::fs::write(
        &brief,
        include_bytes!("../tests/fixtures/office-move-brief.pdf"),
    )?;

    // Two shelves.
    for (shelf_name, files) in [
        (
            "Notes",
            vec![handbook.clone(), notes.clone(), vacances.clone()],
        ),
        (
            "Projects",
            vec![invoice.clone(), payroll.clone(), brief.clone()],
        ),
    ] {
        let exists = {
            let library = ctx.library.read().unwrap();
            library.shelves().iter().any(|s| s.name == shelf_name)
        };
        if exists {
            println!("shelf {shelf_name} already present, skipping");
            continue;
        }
        let shelf = {
            let mut library = ctx.library.write().unwrap();
            library.create_shelf(&ctx.paths, shelf_name, &shelf_root)?
        };
        let copied =
            rebost::shelf::import_into_shelf(&shelf, &files, rebost::shelf::MAX_FILES_PER_SHELF)?
                .files;
        for file in copied {
            let rel = file
                .strip_prefix(&shelf.managed_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let job = rebost::ingest::ProcessJob {
                shelf_id: shelf.id.clone(),
                source_id: rebost::shelf::Shelf::IMPORTED_SOURCE.to_string(),
                source_type: rebost::types::SourceType::Imported,
                source_label: "Imported".into(),
                abs_path: file,
                rel_path: rel,
                force: false,
                epoch: 0,
            };
            rebost::ingest::process_file(&ctx, &job).await?;
        }
        println!("shelf {shelf_name} ready");
    }

    // A finished demo conversation with real retrieved citations, so Chat
    // opens onto something meaningful.
    let has_threads = !rebost::chat::conversations::Conversations::list(&ctx.paths).is_empty();
    if !has_threads {
        let notes_shelf = {
            let library = ctx.library.read().unwrap();
            library
                .shelves()
                .iter()
                .find(|s| s.name == "Notes")
                .map(|s| s.id.clone())
        };
        if let Some(shelf_id) = notes_shelf {
            let question = "When is the kitchen restocked?";
            let tokens = ctx.search.query_tokens(question);
            let hits = ctx.search.search_passages(question, &shelf_id, 24)?;
            let gated = rebost::search::gate::gate_passages(hits, &tokens);
            let (sources, _) = rebost::search::gate::fit_to_budget(gated, Vec::new(), 9000);

            let thread = rebost::chat::conversations::Conversations::create(
                &ctx.paths,
                Some(shelf_id.clone()),
            )?;
            let now = chrono::Utc::now();
            let user = rebost::chat::conversations::StoredMessage {
                id: rebost::ids::message_id(),
                role: "user".into(),
                text: question.into(),
                thinking: None,
                activity: Vec::new(),
                ts: now.to_rfc3339(),
                shelf_id: Some(shelf_id.clone()),
                sources: Vec::new(),
                status: "done".into(),
            };
            rebost::chat::conversations::Conversations::append(&ctx.paths, &thread.id, &user)?;
            ctx.search
                .index_message(&thread.id, &user.id, "user", &user.text, Some("en"), now)?;

            let answer_text = "The kitchen is restocked every Tuesday morning [S1]. Milk and \
coffee are in the cupboard above the sink [S1].";
            let assistant = rebost::chat::conversations::StoredMessage {
                id: rebost::ids::message_id(),
                role: "assistant".into(),
                text: answer_text.into(),
                thinking: None,
                activity: Vec::new(),
                ts: chrono::Utc::now().to_rfc3339(),
                shelf_id: Some(shelf_id.clone()),
                sources: sources.into_iter().filter(|s| s.sid == "S1").collect(),
                status: "done".into(),
            };
            rebost::chat::conversations::Conversations::append(&ctx.paths, &thread.id, &assistant)?;
            ctx.search.index_message(
                &thread.id,
                &assistant.id,
                "assistant",
                &assistant.text,
                Some("en"),
                chrono::Utc::now(),
            )?;
            println!("demo conversation ready");
        }
    }

    let shelves = ctx.library.read().unwrap();
    for shelf in shelves.shelves() {
        let stats = shelves.stats(&shelf.id);
        println!(
            "  {} — {} files, {} searchable, {} PII matches",
            shelf.name, stats.files, stats.searchable, stats.pii.total
        );
    }
    println!("seeded at {}", data_dir.display());
    Ok(())
}
