//! Tauri commands: thin adapters between the webview and the core.
//! Heavy work stays in core modules; errors become friendly strings here.

mod chat;
mod model;
mod recipes;
mod settings;
mod shelves;

pub use chat::*;
pub use model::*;
pub use recipes::*;
pub use settings::*;
pub use shelves::*;

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn require_id(id: &str) -> CmdResult<()> {
    crate::ids::require_safe_id(id).map_err(friendly)
}

pub(crate) fn require_optional_id(id: Option<&str>) -> CmdResult<()> {
    match id {
        Some(id) => require_id(id),
        None => Ok(()),
    }
}

pub(crate) fn friendly<E: std::fmt::Display>(error: E) -> String {
    let text = error.to_string();
    log::error!("command error: {text}");
    let lower = text.to_lowercase();
    if lower.contains("invalid id") {
        return "That request was not valid.".into();
    }
    if lower.contains("not in a shelf") || lower.contains("not allowed") {
        return "That file is not in a Shelf Rebost knows.".into();
    }
    if lower.contains("shelf not found") || lower.contains("file not found") {
        return "That item is no longer available.".into();
    }
    if text.chars().count() > 180 {
        return "Something went wrong. Try again.".into();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_id_rejects_path_traversal() {
        assert!(require_id("s_ab12").is_ok());
        assert!(require_id("../etc").is_err());
        assert!(require_id("a/b").is_err());
        assert_eq!(require_optional_id(None), Ok(()));
        assert!(require_optional_id(Some("..")).is_err());
    }

    #[test]
    fn friendly_maps_known_errors() {
        assert_eq!(friendly("invalid id"), "That request was not valid.");
        assert_eq!(
            friendly("not in a Shelf"),
            "That file is not in a Shelf Rebost knows."
        );
        assert_eq!(
            friendly("shelf not found"),
            "That item is no longer available."
        );
        assert_eq!(
            friendly("x".repeat(200)),
            "Something went wrong. Try again."
        );
    }
}
