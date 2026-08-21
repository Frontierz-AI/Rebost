//! Demo library for UI work and website screenshots.
//!
//! Quit Rebost first. The search index writer is exclusive.
//!
//! ```text
//! cargo run --example seed -- --fresh --model /path/to.gguf
//! cargo run --example seed -- --fresh --empty --model /path/to.gguf
//! ```
//!
//! `--fresh` wipes app data (keeps `library/` unless you delete it).
//! `--empty` finishes first run and installs the AI name, with no Shelves
//! or conversations. Omit it for Harbor, Notes, and a full chat list.
//! `--ai-name` is the Settings label (default: Muse Glimmer).

use rebost::chat::conversations::{ActivityStep, Conversations, StoredMessage};
use rebost::core::{Ctx, NoopEvents};
use rebost::ingest::extract::ExtractorSettings;
use rebost::paths::Paths;
use rebost::reset::BUNDLE_IDENTIFIER;
use rebost::types::SourcePassage;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_AI_NAME: &str = "Muse Glimmer";
const DEFAULT_AI_REF: &str = "unsloth/Muse-Glimmer-30B-GGUF";

fn arg_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn fixture(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, contents).unwrap();
    path
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

fn pause() {
    std::thread::sleep(Duration::from_millis(40));
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let model_arg = arg_after(&args, "--model");
    let ai_name = arg_after(&args, "--ai-name").unwrap_or_else(|| DEFAULT_AI_NAME.into());
    let ai_ref = arg_after(&args, "--ai-ref").unwrap_or_else(|| DEFAULT_AI_REF.into());
    let fresh = args.iter().any(|a| a == "--fresh");
    let empty = args.iter().any(|a| a == "--empty");

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

    let tessdata = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("tessdata");
    let ctx = Arc::new(Ctx::new(
        paths,
        Arc::new(NoopEvents),
        ExtractorSettings {
            tessdata_dir: Some(tessdata),
            timeout_secs: 120,
            ..Default::default()
        },
    )?);

    {
        let mut settings = ctx.settings.write().unwrap();
        settings.onboarding_done = true;
        settings.allow_online_research = false;
        settings.house_rules = if empty {
            String::new()
        } else {
            "Answer in the language of the documents unless asked otherwise.\n\
Use first names.\n\
Short sentences. No slogans.\n\
The name is Harbor. Hours are written 10:00–14:00.\n\
If a date is not in Decisions, say you will check.\n\
Never put a personal mobile on the door."
                .into()
        };
        if let Some(model_path) = &model_arg {
            let model_path = PathBuf::from(model_path);
            let file_name = model_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let dest = ctx.paths.models_dir().join(&file_name);
            if model_path != dest {
                std::fs::create_dir_all(ctx.paths.models_dir())?;
                if !dest.exists() {
                    std::fs::copy(&model_path, &dest)?;
                }
            }
            settings.active_model = Some(rebost::settings::ActiveModel {
                file: file_name,
                name: ai_name.clone(),
                source: "huggingface".into(),
                reference: ai_ref.clone(),
                license: Some("Apache-2.0".into()),
                size_bytes: std::fs::metadata(&model_path)?.len(),
            });
            println!("active AI: {ai_name}");
        }
    }
    ctx.save_settings();

    let _ = rebost::recipes::list(&ctx.paths);
    if empty {
        println!("empty first-run (no Shelves, no conversations)");
        println!("seeded at {}", data_dir.display());
        return Ok(());
    }

    rebost::recipes::create(
        &ctx.paths,
        "Harbor hours",
        "What hours do we keep, and which file should I trust if two notes disagree?",
    )?;
    rebost::recipes::create(
        &ctx.paths,
        "Door number",
        "What number goes on the door? Quote the FAQ and Decisions.",
    )?;

    let staging = data_dir.join("seed-staging");
    std::fs::create_dir_all(&staging)?;
    let harbor_files = harbor_files(&staging);
    let notes_files = notes_files(&staging);

    let harbor_id = import_shelf(&ctx, "Harbor", &harbor_files).await?;
    let notes_id = import_shelf(&ctx, "Notes", &notes_files).await?;

    seed_conversations(&ctx, &harbor_id, &notes_id)?;

    let shelves = ctx.library.read().unwrap();
    for shelf in shelves.shelves() {
        let stats = shelves.stats(&shelf.id);
        println!(
            "  {} — {} files, {} searchable, {} personal-information counts",
            shelf.name, stats.files, stats.searchable, stats.pii.total
        );
    }
    println!("seeded at {}", data_dir.display());
    Ok(())
}

async fn import_shelf(ctx: &Arc<Ctx>, name: &str, files: &[PathBuf]) -> anyhow::Result<String> {
    let exists = {
        let library = ctx.library.read().unwrap();
        library.shelves().iter().any(|s| s.name == name)
    };
    if exists {
        let library = ctx.library.read().unwrap();
        let id = library
            .shelves()
            .iter()
            .find(|s| s.name == name)
            .unwrap()
            .id
            .clone();
        println!("shelf {name} already present, skipping");
        return Ok(id);
    }
    let shelf_root = ctx.paths.library_dir();
    let shelf = {
        let mut library = ctx.library.write().unwrap();
        library.create_shelf(&ctx.paths, name, &shelf_root)?
    };
    let copied =
        rebost::shelf::import_into_shelf(&shelf, files, rebost::shelf::MAX_FILES_PER_SHELF)?.files;
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
        rebost::ingest::process_file(ctx, &job).await?;
    }
    println!("shelf {name} ready");
    Ok(shelf.id)
}

fn doc_named(ctx: &Ctx, shelf_id: &str, file_name: &str) -> Option<SourcePassage> {
    let library = ctx.library.read().unwrap();
    let doc = library.documents(shelf_id).into_iter().find(|d| {
        d.file_name == file_name || d.rel_path.ends_with(file_name) || d.path.ends_with(file_name)
    })?;
    let title = doc
        .file_name
        .trim_end_matches(".md")
        .trim_end_matches(".pdf")
        .to_string();
    Some(SourcePassage {
        sid: "S1".into(),
        document_id: doc.id,
        shelf_id: doc.shelf_id,
        title,
        section: None,
        page_start: None,
        page_end: None,
        body: String::new(),
        path: doc.path,
        score: 1.0,
    })
}

fn sources(ctx: &Ctx, shelf_id: &str, names: &[&str]) -> Vec<SourcePassage> {
    names
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let mut passage = doc_named(ctx, shelf_id, name)?;
            passage.sid = format!("S{}", i + 1);
            Some(passage)
        })
        .collect()
}

fn new_thread(ctx: &Ctx, shelf_id: Option<&str>, title: &str) -> anyhow::Result<String> {
    pause();
    let thread = Conversations::create(&ctx.paths, shelf_id.map(str::to_string))?;
    Conversations::rename(&ctx.paths, &thread.id, title)?;
    Ok(thread.id)
}

fn user(ctx: &Ctx, thread_id: &str, shelf_id: Option<&str>, text: &str) -> anyhow::Result<()> {
    let ts = now();
    let message = StoredMessage {
        id: rebost::ids::message_id(),
        role: "user".into(),
        text: text.into(),
        thinking: None,
        activity: Vec::new(),
        ts: ts.to_rfc3339(),
        shelf_id: shelf_id.map(str::to_string),
        sources: Vec::new(),
        status: "done".into(),
    };
    Conversations::append(&ctx.paths, thread_id, &message)?;
    ctx.search
        .index_message(thread_id, &message.id, "user", text, Some("en"), ts)?;
    Ok(())
}

fn assistant(
    ctx: &Ctx,
    thread_id: &str,
    shelf_id: Option<&str>,
    text: &str,
    cited: Vec<SourcePassage>,
    thinking: Option<&str>,
    activity: Vec<ActivityStep>,
) -> anyhow::Result<()> {
    let ts = now();
    let message = StoredMessage {
        id: rebost::ids::message_id(),
        role: "assistant".into(),
        text: text.into(),
        thinking: thinking.map(str::to_string),
        activity,
        ts: ts.to_rfc3339(),
        shelf_id: shelf_id.map(str::to_string),
        sources: cited,
        status: "done".into(),
    };
    Conversations::append(&ctx.paths, thread_id, &message)?;
    ctx.search
        .index_message(thread_id, &message.id, "assistant", text, Some("en"), ts)?;
    Ok(())
}

fn looked(file: &str) -> Vec<ActivityStep> {
    vec![
        ActivityStep {
            stage: "looking".into(),
            file: None,
        },
        ActivityStep {
            stage: "opening".into(),
            file: Some(file.into()),
        },
    ]
}

fn seed_conversations(ctx: &Ctx, harbor: &str, notes: &str) -> anyhow::Result<()> {
    // Oldest first so the opening-date thread is the one Chat reopens.

    let parking = new_thread(ctx, Some(notes), "Parking on the 8th")?;
    user(
        ctx,
        &parking,
        Some(notes),
        "Where do we park on opening day?",
    )?;
    assistant(
        ctx,
        &parking,
        Some(notes),
        "Two spaces behind the building [S1]. The street ones fill by 9:30 [S1]. On the 8th, Sam arrives first and cones the back pair [S1].",
        sources(ctx, notes, &["Parking.md"]),
        None,
        looked("Parking.md"),
    )?;
    user(ctx, &parking, Some(notes), "Is that written down for Sam?")?;
    assistant(
        ctx,
        &parking,
        Some(notes),
        "Yes. It is in Parking, in Notes [S1].",
        sources(ctx, notes, &["Parking.md"]),
        None,
        Vec::new(),
    )?;

    let printer = new_thread(ctx, Some(notes), "Printer proofs")?;
    user(
        ctx,
        &printer,
        Some(notes),
        "When do the sign proofs come back?",
    )?;
    assistant(
        ctx,
        &printer,
        Some(notes),
        "25 August is the date they gave [S1]. They have Elena's copy from How we write [S1].",
        sources(ctx, notes, &["Call with the printer.md"]),
        None,
        looked("Call with the printer.md"),
    )?;
    user(
        ctx,
        &printer,
        Some(notes),
        "What if the gold on navy is too dark?",
    )?;
    assistant(
        ctx,
        &printer,
        Some(notes),
        "They will try amber on navy instead [S1]. No new words. Use the hours as written [S1].",
        sources(ctx, notes, &["Call with the printer.md"]),
        None,
        Vec::new(),
    )?;

    let write = new_thread(ctx, Some(harbor), "How we write")?;
    user(
        ctx,
        &write,
        Some(harbor),
        "How should we write the name and the hours?",
    )?;
    assistant(
        ctx,
        &write,
        Some(harbor),
        "The name is Harbor. Not Harbor & Co. Not Harbor Studio [S1]. Hours are written 10:00–14:00, not 10-2 [S1]. Short sentences. First names. No slogans [S1].",
        sources(ctx, harbor, &["How we write.md"]),
        Some("How we write is Elena's note for anything that leaves the building."),
        looked("How we write.md"),
    )?;
    user(
        ctx,
        &write,
        Some(harbor),
        "Someone asked if we open on the 1st. Can I put that on a sign?",
    )?;
    assistant(
        ctx,
        &write,
        Some(harbor),
        "No. If a date is not in Decisions, do not invent one. Say you will check [S1].",
        sources(ctx, harbor, &["How we write.md"]),
        None,
        Vec::new(),
    )?;

    let painter = new_thread(ctx, Some(harbor), "Painter on Friday")?;
    user(ctx, &painter, Some(harbor), "When is the painter coming?")?;
    assistant(
        ctx,
        &painter,
        Some(harbor),
        "Friday 21 August [S1]. The room has to be empty Thursday night. Marc [S1].",
        sources(ctx, harbor, &["Meeting 12 August.md"]),
        None,
        looked("Meeting 12 August.md"),
    )?;
    user(
        ctx,
        &painter,
        Some(harbor),
        "Does the hall need to be cleared too?",
    )?;
    assistant(
        ctx,
        &painter,
        Some(harbor),
        "That is still an open note. Scratch from 20 August says to ask Marc whether the painter needs the hall cleared, or only the front room. It is not in Decisions.",
        sources(ctx, notes, &["Scratch 20 August.md"]),
        None,
        looked("Scratch 20 August.md"),
    )?;
    user(ctx, &painter, Some(harbor), "Who confirms the painter?")?;
    assistant(
        ctx,
        &painter,
        Some(harbor),
        "Marc confirms the painter [S1]. The week-of note says he already confirmed Friday [S2].",
        sources(
            ctx,
            harbor,
            &["Meeting 12 August.md", "Week of 18 August.md"],
        ),
        None,
        Vec::new(),
    )?;

    let friday = new_thread(ctx, Some(harbor), "Friday evening")?;
    user(
        ctx,
        &friday,
        Some(harbor),
        "Are we doing Friday evening in October?",
    )?;
    assistant(
        ctx,
        &friday,
        Some(harbor),
        "Not yet. Jordi wants to try it. Keep it out of Decisions until we say yes [S1].",
        sources(ctx, harbor, &["Brief.md"]),
        Some(
            "Brief and Decisions both leave Friday evening open. Week of 18 August says the same.",
        ),
        looked("Brief.md"),
    )?;
    user(ctx, &friday, Some(harbor), "Did anyone write a date?")?;
    assistant(
        ctx,
        &friday,
        Some(harbor),
        "No date. Decisions says Jordi still wants to try Friday evening in October, and that it is not a decision yet [S1].",
        sources(ctx, harbor, &["Decisions.md"]),
        None,
        looked("Decisions.md"),
    )?;
    user(
        ctx,
        &friday,
        Some(harbor),
        "So what do I tell a new person?",
    )?;
    assistant(
        ctx,
        &friday,
        Some(harbor),
        "Say it is still open. Point them at Brief, then Decisions [S1].",
        sources(ctx, harbor, &["Brief.md"]),
        None,
        Vec::new(),
    )?;

    let owners = new_thread(ctx, Some(harbor), "Who owns what")?;
    user(
        ctx,
        &owners,
        Some(harbor),
        "Who owns the folder and the FAQ?",
    )?;
    assistant(
        ctx,
        &owners,
        Some(harbor),
        "Ana: the folder, the dates, the FAQ [S1]. Jordi: the brief and the one-pager for new people [S1]. Marc: the room and the painter [S1]. Elena: signs and the street-facing copy [S1]. Sam: hours on the door and the phone [S1].",
        sources(ctx, harbor, &["Decisions.md"]),
        None,
        looked("Decisions.md"),
    )?;
    user(ctx, &owners, Some(harbor), "Who do I write to about signs?")?;
    assistant(
        ctx,
        &owners,
        Some(harbor),
        "elena@harbor.example for signs and street copy [S1].",
        sources(ctx, harbor, &["FAQ.md"]),
        None,
        Vec::new(),
    )?;

    let names = new_thread(ctx, None, "Other names we tried")?;
    user(
        ctx,
        &names,
        None,
        "We already picked Harbor. Remind me what we ruled out, in one line.",
    )?;
    assistant(
        ctx,
        &names,
        None,
        "Harbor & Co and Harbor Studio. The name is Harbor.",
        Vec::new(),
        None,
        Vec::new(),
    )?;

    let door = new_thread(ctx, Some(harbor), "Number on the door")?;
    user(ctx, &door, Some(harbor), "What number goes on the door?")?;
    assistant(
        ctx,
        &door,
        Some(harbor),
        "The desk number: +34 612 000 110 [S1]. Do not put a personal mobile on the door [S2].",
        sources(ctx, harbor, &["FAQ.md", "Decisions.md"]),
        Some("FAQ and Decisions agree. The number is the desk. No personal mobiles."),
        looked("FAQ.md"),
    )?;
    user(ctx, &door, Some(harbor), "Who answers that phone?")?;
    assistant(
        ctx,
        &door,
        Some(harbor),
        "Sam: hours on the door and the phone [S1].",
        sources(ctx, harbor, &["Decisions.md"]),
        None,
        Vec::new(),
    )?;
    user(
        ctx,
        &door,
        Some(harbor),
        "Can we put Ana's mobile as a backup?",
    )?;
    assistant(
        ctx,
        &door,
        Some(harbor),
        "No. We will not put a personal mobile on the door [S1]. The number on the FAQ is the desk [S1].",
        sources(ctx, harbor, &["Decisions.md"]),
        None,
        looked("Decisions.md"),
    )?;

    // Hero thread: last, so Chat opens here.
    let open = new_thread(ctx, Some(harbor), "When do we open?")?;
    user(ctx, &open, Some(harbor), "When do we open?")?;
    assistant(
        ctx,
        &open,
        Some(harbor),
        "Monday 8 September 2026 [S1]. Not the 1st. Marc needs that week for the fit-out [S1].",
        sources(ctx, harbor, &["Decisions.md"]),
        Some(
            "The question is the opening date. Decisions is the source Ana keeps.\n\
Brief says the same date. FAQ repeats it for people who write in.\n\
The 1st was asked and turned down.",
        ),
        vec![
            ActivityStep {
                stage: "looking".into(),
                file: None,
            },
            ActivityStep {
                stage: "opening".into(),
                file: Some("Decisions.md".into()),
            },
            ActivityStep {
                stage: "opening".into(),
                file: Some("Brief.md".into()),
            },
        ],
    )?;
    user(ctx, &open, Some(harbor), "And Saturday hours?")?;
    assistant(
        ctx,
        &open,
        Some(harbor),
        "Saturdays are 10:00–14:00 [S1]. Elena asked for a full day. We tried that in July and it was quiet after lunch, so we cut it [S1].",
        sources(ctx, harbor, &["Decisions.md"]),
        None,
        looked("Decisions.md"),
    )?;
    user(
        ctx,
        &open,
        Some(harbor),
        "What should a new person read first?",
    )?;
    assistant(
        ctx,
        &open,
        Some(harbor),
        "This file, then Decisions, then the FAQ, then How we write [S1]. Until we open, this folder is the source of truth. If it is not written here, it is not decided [S1].",
        sources(ctx, harbor, &["Brief.md"]),
        None,
        looked("Brief.md"),
    )?;
    user(
        ctx,
        &open,
        Some(harbor),
        "Write the hours the way Elena wants them on the door.",
    )?;
    assistant(
        ctx,
        &open,
        Some(harbor),
        "Weekdays 10:00–19:00. Saturdays 10:00–14:00. Closed Sunday [S1].",
        sources(ctx, harbor, &["FAQ.md"]),
        None,
        looked("FAQ.md"),
    )?;
    user(
        ctx,
        &open,
        Some(harbor),
        "Who updates Decisions after this?",
    )?;
    assistant(
        ctx,
        &open,
        Some(harbor),
        "Ana. She keeps Decisions current [S1]. After the 12 August meeting she was the one to update Decisions and the FAQ [S2].",
        sources(ctx, harbor, &["Decisions.md", "Meeting 12 August.md"]),
        Some("Decisions is Ana's file. The 12 August minutes ask her to update it, and the FAQ."),
        looked("Decisions.md"),
    )?;

    println!("conversations ready");
    Ok(())
}

fn harbor_files(staging: &Path) -> Vec<PathBuf> {
    vec![
        fixture(
            staging,
            "Brief.md",
            r#"# Harbor

Jordi, 18 August 2026.

Harbor is the working name. Five of us. The work already lives in this folder.

We open 8 September. Until then this folder is the source of truth. If it is not written here, it is not decided.

## What we are doing

Opening the room on 8 September. Saturday hours 10:00–14:00. Desk phone on the FAQ, not anyone's mobile.

## What a new person should read first

This file, then Decisions, then the FAQ, then How we write.

## Still open

Friday evening in October. Jordi wants to try it. Keep it out of Decisions until we say yes.
"#,
        ),
        fixture(
            staging,
            "Decisions.md",
            r#"# Decisions

Kept by Ana. Last updated 12 August 2026.

## Opening date

We open on Monday 8 September 2026. Not the 1st. Marc needs that week for the fit-out.

## Saturday hours

Saturdays are 10:00–14:00. Elena asked for a full day. We tried that in July and it was quiet after lunch, so we cut it.

Jordi still wants to try Friday evening in October. That is not a decision yet.

## Who owns what

Ana: the folder, the dates, the FAQ.
Jordi: the brief and the one-pager for new people.
Marc: the room and the painter.
Elena: signs and the street-facing copy.
Sam: hours on the door and the phone.

## What we will not do

We will not put a personal mobile on the door. The number on the FAQ is the desk: +34 612 000 110.
"#,
        ),
        fixture(
            staging,
            "FAQ.md",
            r#"# FAQ

Questions we keep getting. Ana keeps this current.

## When do you open?

Monday 8 September 2026.

## What are the hours?

Weekdays 10:00–19:00. Saturdays 10:00–14:00. Closed Sunday.

## What is the number on the door?

+34 612 000 110. That is the desk. Do not give out personal mobiles.

## Who do I write to?

ana@harbor.example for dates and the folder.
jordi@harbor.example for the brief.
elena@harbor.example for signs and street copy.

## Can we stay later on Saturday?

Not this year. We tried a full Saturday in July. It was quiet after lunch.
"#,
        ),
        fixture(
            staging,
            "How we write.md",
            r#"# How we write

Elena, for anyone who writes something that leaves the building.

Short sentences. First names. No slogans.

The name is Harbor. Not Harbor & Co. Not Harbor Studio.

Hours are written 10:00–14:00, not 10-2 or 10 to 2.

If a date is not in Decisions, do not invent one. Say you will check.
"#,
        ),
        fixture(
            staging,
            "Meeting 12 August.md",
            r#"# Meeting — 12 August 2026

Present: Ana, Jordi, Marc, Elena. Sam on the phone.

## What we closed

Opening is 8 September. Saturday hours stay 10:00–14:00.

Marc booked the painter for Friday 21 August. The room has to be empty Thursday night.

Elena orders the signs this week. Copy comes from How we write.

## Open

Jordi still owes the one-page brief. He said Monday.

Should we put the shared folder on Sam's Windows PC the same way Ana has it? Ana will send the steps.

## Next

Ana updates Decisions and the FAQ.
Marc confirms the painter.
Elena sends sign proofs to the group.
"#,
        ),
        fixture(
            staging,
            "Timeline.md",
            r#"# Timeline

21 August — Painter. Room empty Thursday night. Marc.
25 August — Sign proofs back from the printer. Elena.
1 September — Soft walkthrough, no public hours. Ana and Sam.
8 September — We open. 10:00. Everyone.
Saturdays — 10:00–14:00. Sam on the door.
"#,
        ),
        fixture(
            staging,
            "Week of 18 August.md",
            r#"# Week of 18 August

## Done

Jordi posted the brief.
Ana rewrote the FAQ.
Marc confirmed the painter for Friday.
Elena sent the first sign proof. It is in the mail, not in this folder.

## This week

Painter on Friday. Room empty Thursday night.

Sam is setting up the shared folder on the office PC the same way Ana has it.

## Open

Friday evening in October. Still not a decision.
"#,
        ),
    ]
}

fn notes_files(staging: &Path) -> Vec<PathBuf> {
    vec![
        fixture(
            staging,
            "Parking.md",
            r#"# Parking

Two spaces behind the building. The street ones fill by 9:30.

On the 8th, Sam arrives first and cones the back pair.
"#,
        ),
        fixture(
            staging,
            "Call with the printer.md",
            r#"# Call with the printer

25 August is the date they gave for proofs. They have Elena's copy from How we write.

If the gold on navy is too dark, they will try amber on navy instead.

No new words. Use the hours as written.
"#,
        ),
        fixture(
            staging,
            "Scratch 20 August.md",
            r#"# Scratch — 20 August

Ask Marc if the painter needs the hall cleared too, or only the front room.

Elena's proof looked small on the door mock. Ask for the 40 cm version.

Sam's PC is Windows. Same folder. Same Shelf name: Harbor.
"#,
        ),
    ]
}
