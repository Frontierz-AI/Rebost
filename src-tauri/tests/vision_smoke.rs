//! Real local vision inference through the same persistence and chat path as the desktop.

mod common;

use common::test_app;
use rebost::chat::{conversations::Conversations, images, ChatService};
use rebost::engine::Engine;
use rebost::settings::{ActiveModel, VisionProjector};
use std::io::Cursor;
use std::path::Path;

fn link(source: &Path, target: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(source, target).unwrap();
    #[cfg(windows)]
    if std::fs::hard_link(source, target).is_err() {
        std::fs::copy(source, target).unwrap();
    }
}

#[ignore = "needs REBOST_ENGINE_ARCHIVE, REBOST_VISION_MODEL and REBOST_VISION_PROJECTOR"]
#[tokio::test(flavor = "multi_thread")]
async fn images_reach_the_model_and_survive_reopening() {
    let model = std::env::var("REBOST_VISION_MODEL").expect("vision model path");
    let projector = std::env::var("REBOST_VISION_PROJECTOR").expect("vision projector path");
    let app = test_app();
    std::fs::create_dir_all(app.ctx.paths.models_dir()).unwrap();
    link(
        Path::new(&model),
        &app.ctx.paths.models_dir().join("vision-test.gguf"),
    );
    link(
        Path::new(&projector),
        &app.ctx.paths.models_dir().join("vision-projector.gguf"),
    );
    app.ctx.settings.write().unwrap().active_model = Some(ActiveModel {
        projector: Some(VisionProjector {
            file: "vision-projector.gguf".into(),
            size_bytes: std::fs::metadata(&projector).unwrap().len(),
        }),
        file: "vision-test.gguf".into(),
        name: Path::new(&model)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        source: "local".into(),
        reference: "local/vision".into(),
        license: None,
        size_bytes: std::fs::metadata(&model).unwrap().len(),
    });
    let engine = Engine::new(app.ctx.clone());
    // Optional network-backed check of the existing-install upgrade path, using cached bytes.
    if let Ok(repo) = std::env::var("REBOST_VISION_REPO") {
        let resolved = rebost::engine::models::resolve_projector(
            &engine.client,
            "huggingface",
            &repo,
            Path::new(&model).file_name().unwrap().to_str().unwrap(),
        )
        .await
        .unwrap()
        .expect("matching projector");
        assert_eq!(
            resolved.size,
            Some(std::fs::metadata(&projector).unwrap().len())
        );
        link(
            Path::new(&projector),
            &app.ctx
                .paths
                .models_dir()
                .join(format!("vision-{}.gguf", resolved.sha256.unwrap())),
        );
        {
            let mut settings = app.ctx.settings.write().unwrap();
            let active = settings.active_model.as_mut().unwrap();
            active.source = "huggingface".into();
            active.reference = repo;
            active.projector = None;
        }
        engine.ensure_ready().await.unwrap();
        assert!(engine.status().vision.is_none());
        assert!(engine.vision_offer().await.unwrap().is_some());
        engine
            .enable_vision()
            .await
            .expect("upgrade existing model to vision");
        assert!(app
            .ctx
            .settings
            .read()
            .unwrap()
            .active_model
            .as_ref()
            .unwrap()
            .projector
            .is_some());
    }

    engine.ensure_ready().await.expect("vision engine ready");
    let limits = engine
        .status()
        .vision
        .expect("running engine reports vision");
    let thread = Conversations::create(&app.ctx.paths, None).unwrap();
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        320,
        240,
        image::Rgb([240, 0, 0]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    let image = images::store(
        &app.ctx.paths,
        &thread.id,
        "color.png",
        png.get_ref(),
        limits,
    )
    .unwrap();
    let chat = ChatService::new(app.ctx.clone(), engine.clone());
    let mut probe = rebost::engine::ChatMessage::text(
        "user",
        "What color is this image? Answer with one word.",
    );
    probe
        .images
        .push(images::data_url(&app.ctx.paths, &thread.id, &image.id, false).unwrap());
    let direct = engine
        .chat_once(
            &[probe],
            0.0,
            64,
            &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
        .unwrap();
    eprintln!("direct vision probe: {:?}", direct.answer);
    assert!(direct.answer.to_lowercase().contains("red"));
    let answer = chat
        .send_with_images(
            &thread.id,
            "What is the main color of this image? Answer with one color word.",
            None,
            std::slice::from_ref(&image.id),
        )
        .await
        .unwrap();
    eprintln!("vision answer: {:?} ({})", answer.text, answer.status);
    assert_eq!(answer.status, "done");
    assert!(
        answer.text.to_lowercase().contains("red"),
        "the model must identify the pixels: {}",
        answer.text
    );
    let saved = Conversations::messages(&app.ctx.paths, &thread.id);
    assert_eq!(saved[0].images, vec![image.clone()]);
    let reopened = ChatService::new(app.ctx.clone(), engine.clone());
    let followup = reopened
        .send_message(
            &thread.id,
            "Look at the image again. What color is it? Answer with one word.",
            None,
        )
        .await
        .unwrap();
    eprintln!(
        "vision follow-up: {:?} ({})",
        followup.text, followup.status
    );
    assert_eq!(followup.status, "done");
    assert!(followup.text.to_lowercase().contains("red"));
    if limits.max_images >= 2 {
        let mut blue = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            320,
            240,
            image::Rgb([0, 0, 240]),
        ))
        .write_to(&mut blue, image::ImageFormat::Png)
        .unwrap();
        let second = images::store(
            &app.ctx.paths,
            &thread.id,
            "second.png",
            blue.get_ref(),
            limits,
        )
        .unwrap();
        let comparison = chat.send_with_images(&thread.id, "What are the colors of these two images, in order? Answer only with the two color names.", None, &[image.id.clone(), second.id]).await.unwrap();
        eprintln!("two image comparison: {:?}", comparison.text);
        assert_eq!(comparison.status, "done");
        assert!(
            comparison.text.to_lowercase().contains("red")
                && comparison.text.to_lowercase().contains("blue")
        );
    }
    engine.stop().await;
    app.ctx
        .settings
        .write()
        .unwrap()
        .active_model
        .as_mut()
        .unwrap()
        .projector = None;
    engine.ensure_ready().await.expect("text-only engine ready");
    assert!(engine.status().vision.is_none());
    // Switching to text-only must reject the image before saving a misleading user turn.
    let before = Conversations::messages(&app.ctx.paths, &thread.id).len();
    assert!(chat
        .send_with_images(&thread.id, "Read it", None, &[image.id])
        .await
        .is_err());
    assert_eq!(
        Conversations::messages(&app.ctx.paths, &thread.id).len(),
        before
    );
    engine.stop().await;
    rebost::chat::delete_thread(&app.ctx, &thread.id).unwrap();
    assert!(!app
        .ctx
        .paths
        .conversations_dir()
        .join(&thread.id)
        .join("images")
        .exists());
}
