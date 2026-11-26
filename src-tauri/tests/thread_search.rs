//! Sidebar conversation search over the real message index.

mod common;

use common::{test_app, TestApp};
use rebost::chat::conversations::Conversations;
use rebost::chat::thread_search::{search, ThreadSearchHit};

fn thread(app: &TestApp) -> String {
    Conversations::create(&app.ctx.paths, None).unwrap().id
}

fn say(app: &TestApp, thread: &str, message: &str, text: &str, lang: Option<&str>) {
    app.ctx
        .search
        .index_message(
            thread,
            message,
            "assistant",
            text,
            lang,
            jiff::Timestamp::now(),
        )
        .unwrap();
}

fn marks(hit: &ThreadSearchHit) -> Vec<String> {
    hit.snippet
        .iter()
        .filter(|segment| segment.mark)
        .map(|segment| segment.text.clone())
        .collect()
}

fn text(hit: &ThreadSearchHit) -> String {
    hit.snippet
        .iter()
        .map(|segment| segment.text.as_str())
        .collect()
}

#[test]
fn hits_group_by_conversation_with_a_match_count() {
    let app = test_app();
    let supplier = thread(&app);
    let other = thread(&app);
    say(
        &app,
        &supplier,
        "m_1",
        "The supplier quote from Acme is 4200 EUR.",
        Some("en"),
    );
    say(
        &app,
        &supplier,
        "m_2",
        "Compare the supplier quote with last year.",
        Some("en"),
    );
    say(
        &app,
        &supplier,
        "m_3",
        "A second supplier quote arrived today.",
        Some("en"),
    );
    say(
        &app,
        &other,
        "m_4",
        "The plumber sent a quote for the kitchen.",
        Some("en"),
    );
    say(&app, &other, "m_5", "Lunch is at noon.", Some("en"));

    let hits = search(&app.ctx, "supplier quote", 50).unwrap();
    assert_eq!(hits.len(), 2, "one row per conversation");
    assert_eq!(hits[0].thread_id, supplier);
    assert_eq!(hits[0].matches, 3);
    assert!(!hits[0].matches_capped);
    assert_eq!(hits[1].thread_id, other);
    assert_eq!(hits[1].matches, 1);
    assert_eq!(hits[1].message_id, "m_4");
    assert_eq!(hits[1].role, "assistant");
    assert!(!hits[1].created_at.is_empty());
    assert_eq!(marks(&hits[1]), ["quote"]);
}

#[test]
fn deleted_conversations_never_appear() {
    let app = test_app();
    let kept = thread(&app);
    let deleted = thread(&app);
    say(
        &app,
        &kept,
        "m_1",
        "Northwind invoice for March",
        Some("en"),
    );
    say(
        &app,
        &deleted,
        "m_2",
        "Northwind invoice for April",
        Some("en"),
    );
    say(
        &app,
        "t_never_saved",
        "m_3",
        "Northwind invoice for May",
        Some("en"),
    );
    // Metadata gone but index records left behind: a race with deletion.
    Conversations::delete(&app.ctx.paths, &deleted).unwrap();

    let hits = search(&app.ctx, "northwind invoice", 50).unwrap();
    let threads: Vec<&str> = hits.iter().map(|hit| hit.thread_id.as_str()).collect();
    assert_eq!(threads, [kept.as_str()]);
}

#[test]
fn stopword_only_queries_return_nothing() {
    let app = test_app();
    let id = thread(&app);
    say(
        &app,
        &id,
        "m_1",
        "What is the plan for the move and the budget?",
        Some("en"),
    );
    assert!(search(&app.ctx, "the and of", 50).unwrap().is_empty());
    assert!(search(&app.ctx, "   ", 50).unwrap().is_empty());
}

#[test]
fn inflections_match_through_catalan_and_spanish_stems() {
    let app = test_app();
    let catalan = thread(&app);
    let spanish = thread(&app);
    say(
        &app,
        &catalan,
        "m_ca",
        "Quan comença la vacança de l'equip? Ho hem de saber per al calendari.",
        Some("ca"),
    );
    say(
        &app,
        &spanish,
        "m_es",
        "¿Cuándo empiezan las vacaciones de la oficina este año?",
        Some("es"),
    );

    let hits = search(&app.ctx, "vacances", 50).unwrap();
    let hit = hits
        .iter()
        .find(|hit| hit.thread_id == catalan)
        .expect("ca stem");
    assert_eq!(marks(hit), ["vacança"]);

    let hits = search(&app.ctx, "vacación", 50).unwrap();
    let hit = hits
        .iter()
        .find(|hit| hit.thread_id == spanish)
        .expect("es stem");
    assert_eq!(marks(hit), ["vacaciones"]);
}

#[test]
fn snippets_are_plain_text_around_the_match() {
    let app = test_app();
    let id = thread(&app);
    let body = format!(
        "## Summary\n\n{}\n\n- The **termination notice** is ninety days [S1].\n\n{}",
        "Background paragraph with nothing relevant in it at all. ".repeat(6),
        "Closing words that go on for a while. ".repeat(6),
    );
    say(&app, &id, "m_1", &body, Some("en"));

    let hits = search(&app.ctx, "Termination", 50).unwrap();
    let snippet = text(&hits[0]);
    assert!(
        snippet.starts_with('…') && snippet.ends_with('…'),
        "{snippet}"
    );
    assert!(snippet.chars().count() <= 124, "{snippet}");
    for syntax in ["**", "[S1]", "##", "- The"] {
        assert!(!snippet.contains(syntax), "{syntax} left in {snippet}");
    }
    assert_eq!(marks(&hits[0]), ["termination"]);
}

#[test]
fn an_unfinished_last_word_finds_longer_words() {
    let app = test_app();
    let garden = thread(&app);
    let other = thread(&app);
    say(
        &app,
        &garden,
        "m_1",
        "Plant the tomato seedlings after the last frost.",
        Some("en"),
    );
    say(
        &app,
        &other,
        "m_2",
        "The tomb was closed on Monday.",
        Some("en"),
    );

    let hits = search(&app.ctx, "tomat", 50).unwrap();
    assert_eq!(hits.len(), 1, "tomb does not start with tomat");
    assert_eq!(hits[0].thread_id, garden);
    assert_eq!(marks(&hits[0]), ["tomato"]);

    // An earlier word still has to match whole.
    assert!(search(&app.ctx, "seed tomat", 50).unwrap().len() == 1);
    assert!(search(&app.ctx, "plan", 50).unwrap().len() == 1);
    // Two letters are too few to expand.
    assert!(search(&app.ctx, "to", 50).unwrap().is_empty());
    // A finished word ranks above a word that only starts with it.
    say(
        &app,
        &other,
        "m_3",
        "Tomatoes are cheap this week.",
        Some("en"),
    );
    let hits = search(&app.ctx, "tomato", 50).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].thread_id, garden);
}
