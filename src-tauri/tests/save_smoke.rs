//! Real-model smoke for `save_document`: a document request on a Shelf ends
//! with a saved, readable file in the Shelf folder; a plain question does not.
//!
//! Same env vars as `core_smoke`:
//!   REBOST_ENGINE_ARCHIVE — path to the host llama.cpp archive from engine/pin.rs
//!   REBOST_TEST_MODEL     — path to a chat GGUF
//! Run: cargo test --test save_smoke -- --ignored --nocapture

mod common;

use common::*;
use rebost::chat::save_doc::SaveStatus;
use rebost::chat::ChatService;
use rebost::engine::Engine;
use rebost::settings::InstalledModel;

#[ignore = "needs REBOST_ENGINE_ARCHIVE and REBOST_TEST_MODEL"]
#[tokio::test(flavor = "multi_thread")]
async fn a_document_request_saves_the_answer_to_the_shelf() {
    let model_path = std::env::var("REBOST_TEST_MODEL").expect("REBOST_TEST_MODEL");
    assert!(std::path::Path::new(&std::env::var("REBOST_ENGINE_ARCHIVE").unwrap()).exists());

    let app = test_app();
    let shelf_id = make_shelf(&app, "Contracts");
    ingest_file(&app, &shelf_id, &fixture_contract_md(app.dir.path())).await;

    let model_file = "smoke-model.gguf".to_string();
    std::fs::create_dir_all(app.ctx.models_dir()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&model_path, app.ctx.models_dir().join(&model_file)).unwrap();
    #[cfg(windows)]
    std::fs::copy(&model_path, app.ctx.models_dir().join(&model_file)).unwrap();
    app.ctx.settings.write().unwrap().active_model = Some(InstalledModel {
        projector: None,
        file: model_file,
        name: "Smoke test model".into(),
        source: "local".into(),
        reference: "local/smoke".into(),
        license: None,
        size_bytes: std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0),
    });
    let engine = Engine::new(app.ctx.clone());
    engine.ensure_ready().await.expect("engine ready");
    let chat = ChatService::new(app.ctx.clone(), engine.clone());
    let managed = app
        .ctx
        .library
        .read()
        .unwrap()
        .shelf(&shelf_id)
        .unwrap()
        .managed_path
        .clone();

    let mut saved_any = false;
    for (prompt, expect_save) in [
        ("Write a short report on the termination terms of the agreement and save it as a document called Termination summary.", true),
        ("Redacta una carta breu al proveïdor recordant el termini de preavís i desa-la com a document.", true),
        ("According to the agreement, how many days of notice are needed to terminate?", false),
    ] {
        let thread = rebost::chat::conversations::Conversations::create(
            &app.ctx.paths,
            Some(shelf_id.clone()),
        )
        .unwrap();
        let answer = chat
            .send_message(&thread.id, prompt, Some(shelf_id.clone()))
            .await
            .expect("chat answer");
        eprintln!(
            "─ prompt ─ {prompt}\n─ status {} · saved {:?}\n{}\n",
            answer.status,
            answer.saved.as_ref().map(|s| (&s.status, &s.file_name)),
            answer.text
        );
        assert_eq!(answer.status, "done");
        match (&answer.saved, expect_save) {
            (Some(saved), true) => {
                assert_eq!(saved.status, SaveStatus::Saved, "{saved:?}");
                let path = std::path::PathBuf::from(saved.path.as_deref().unwrap());
                assert_eq!(path.parent().unwrap(), managed);
                let bytes = std::fs::read(&path).unwrap();
                if path.extension().is_some_and(|e| e == "docx") {
                    assert!(bytes.starts_with(b"PK"), "a .docx is a zip");
                } else {
                    assert!(!bytes.is_empty());
                }
                saved_any = true;
            }
            (None, true) => eprintln!("(model did not call save_document for this prompt)"),
            (Some(saved), false) => panic!("a plain question saved {saved:?}"),
            (None, false) => {}
        }
    }
    let listed: Vec<_> = std::fs::read_dir(&managed)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    eprintln!("─ Shelf folder ─ {listed:?}");
    assert!(saved_any, "no document request was saved");
    engine.stop().await;
}
