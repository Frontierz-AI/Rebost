//! Recipes: a library of saved prompts.
//!
//! A Recipe is a name plus a prompt. Clicking one starts a new conversation
//! with the composer pre-filled; the user completes the «placeholders»,
//! picks a Shelf if the ask needs those files, and sends. Retrieval, the
//! gate, and citations are ordinary Chat.
//!
//! Users can add their own Recipes and delete any of them, defaults
//! included. Stored locally in `recipes.json`.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub prompt: String,
    /// Shipped with Rebost (still deletable; restorable in one click).
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecipesFile {
    recipes: Vec<Recipe>,
}

/// Shipped Recipes.
pub const DEFAULTS: &[(&str, &str, &str)] = &[
    (
        "reply-to-client",
        "Reply to a client",
        "Draft a reply to this client message in your usual voice. Keep it warm and concrete, \
end with a clear next step, and write it in the client's language.\n\nClient message:\n«paste \
the message here»",
    ),
    (
        "one-page-brief",
        "One-page brief",
        "Give me a one-page brief of «document name» from this Shelf: what it is, the key \
points, exact dates and amounts, and any risks or open questions.",
    ),
    (
        "compare-documents",
        "Compare two documents",
        "Compare «document A» and «document B» from this Shelf. Start with a two-sentence \
verdict, then list the differences that matter (payment, dates, obligations, prices, \
termination) and finish with what we should double-check.",
    ),
    (
        "document-key-terms",
        "Document key terms",
        "List the key terms of «document name» from this Shelf as a table: term, what it says, \
the exact date or amount, and where it says so.",
    ),
    (
        "minutes-actions",
        "Minutes → action list",
        "Turn these meeting notes into an action list with an owner and a deadline for each \
item, then note any open questions.\n\nNotes:\n«paste the notes here»",
    ),
    (
        "translate",
        "Translate this",
        "Translate the text below into «language», keeping the structure and keeping names, \
numbers and legal references exact.\n\n«paste the text here»",
    ),
    (
        "policy-qa",
        "Policy Q&A",
        "What do our documents on this Shelf say about «topic»? Answer only from those \
documents and cite where it says so. If it isn't covered, tell me plainly.",
    ),
    (
        "campaign-ideas",
        "Five campaign ideas",
        "Give me five ideas for «campaign, product or promotion». For each one give me a hook, \
the channel it fits best, and an example first line.",
    ),
];

fn default_recipes() -> Vec<Recipe> {
    DEFAULTS
        .iter()
        .map(|(id, name, prompt)| Recipe {
            id: (*id).to_string(),
            name: (*name).to_string(),
            prompt: (*prompt).to_string(),
            builtin: true,
        })
        .collect()
}

fn read(paths: &Paths) -> Option<Vec<Recipe>> {
    let text = std::fs::read_to_string(paths.recipes_path()).ok()?;
    serde_json::from_str::<RecipesFile>(&text)
        .ok()
        .map(|f| f.recipes)
}

fn write(paths: &Paths, recipes: &[Recipe]) -> Result<()> {
    let file = RecipesFile {
        recipes: recipes.to_vec(),
    };
    crate::paths::atomic_write(&paths.recipes_path(), serde_json::to_string_pretty(&file)?)?;
    Ok(())
}

/// All recipes; seeds the defaults on first use. Built-in entries can't be
/// edited in the app, so their wording is kept in sync with the current
/// release.
pub fn list(paths: &Paths) -> Vec<Recipe> {
    match read(paths) {
        Some(mut recipes) => {
            let mut changed = false;
            for recipe in recipes.iter_mut() {
                if recipe.id == "contract-key-terms" {
                    recipe.id = "document-key-terms".into();
                    changed = true;
                }
            }
            for recipe in recipes.iter_mut().filter(|r| r.builtin) {
                if let Some((_, name, prompt)) = DEFAULTS.iter().find(|(id, _, _)| *id == recipe.id)
                {
                    if recipe.name != *name || recipe.prompt != *prompt {
                        recipe.name = (*name).to_string();
                        recipe.prompt = (*prompt).to_string();
                        changed = true;
                    }
                }
            }
            if changed {
                if let Err(error) = write(paths, &recipes) {
                    log::error!("saving recipes: {error:#}");
                }
            }
            recipes
        }
        None => {
            let defaults = default_recipes();
            if let Err(error) = write(paths, &defaults) {
                log::error!("saving default recipes: {error:#}");
            }
            defaults
        }
    }
}

pub fn create(paths: &Paths, name: &str, prompt: &str) -> Result<Recipe> {
    let name = name.trim();
    let prompt = prompt.trim();
    if name.is_empty() {
        return Err(anyhow!("Give the Recipe a name."));
    }
    if prompt.is_empty() {
        return Err(anyhow!("Write the prompt the Recipe should start with."));
    }
    let mut recipes = list(paths);
    let recipe = Recipe {
        id: format!("r_{}", uuid::Uuid::new_v4().simple()),
        name: name.to_string(),
        prompt: prompt.to_string(),
        builtin: false,
    };
    recipes.push(recipe.clone());
    write(paths, &recipes)?;
    Ok(recipe)
}

pub fn delete(paths: &Paths, id: &str) -> Result<()> {
    let mut recipes = list(paths);
    let before = recipes.len();
    recipes.retain(|r| r.id != id);
    if recipes.len() == before {
        return Err(anyhow!("Recipe not found"));
    }
    write(paths, &recipes)
}

/// Re-add any missing default Recipes (kept alongside the user's own).
pub fn restore_defaults(paths: &Paths) -> Result<Vec<Recipe>> {
    let mut recipes = list(paths);
    for default in default_recipes() {
        if !recipes.iter().any(|r| r.id == default.id) {
            recipes.push(default);
        }
    }
    // Defaults first, in their canonical order; user recipes after.
    recipes.sort_by_key(|r| {
        DEFAULTS
            .iter()
            .position(|(id, _, _)| *id == r.id)
            .unwrap_or(usize::MAX)
    });
    write(paths, &recipes)?;
    Ok(recipes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        paths.ensure().unwrap();
        (dir, paths)
    }

    #[test]
    fn first_use_seeds_the_defaults() {
        let (_dir, paths) = paths();
        let recipes = list(&paths);
        assert_eq!(recipes.len(), DEFAULTS.len());
        assert!(recipes.iter().all(|r| r.builtin));
        assert!(recipes.iter().any(|r| r.name == "Reply to a client"));
        assert!(recipes.iter().any(|r| r.id == "document-key-terms"));
        // Persisted.
        assert!(paths.recipes_path().exists());
    }

    #[test]
    fn create_delete_and_restore() {
        let (_dir, paths) = paths();
        let mine = create(
            &paths,
            "Weekly update",
            "Write our weekly team update: «notes»",
        )
        .unwrap();
        assert!(!mine.builtin);
        assert_eq!(list(&paths).len(), DEFAULTS.len() + 1);

        // Defaults are deletable too.
        delete(&paths, "translate").unwrap();
        delete(&paths, &mine.id).unwrap();
        let after = list(&paths);
        assert_eq!(after.len(), DEFAULTS.len() - 1);
        assert!(!after.iter().any(|r| r.id == "translate"));

        // Restore brings the missing default back, keeps the rest.
        let restored = restore_defaults(&paths).unwrap();
        assert_eq!(restored.len(), DEFAULTS.len());
        assert!(restored.iter().any(|r| r.id == "translate"));
    }

    #[test]
    fn create_validates_input() {
        let (_dir, paths) = paths();
        assert!(create(&paths, "", "prompt").is_err());
        assert!(create(&paths, "name", "  ").is_err());
    }

    #[test]
    fn old_contract_key_terms_id_is_renamed() {
        let (_dir, paths) = paths();
        let _ = list(&paths);
        let mut recipes = list(&paths);
        if let Some(recipe) = recipes.iter_mut().find(|r| r.id == "document-key-terms") {
            recipe.id = "contract-key-terms".into();
        }
        write(&paths, &recipes).unwrap();
        let loaded = list(&paths);
        assert!(loaded.iter().any(|r| r.id == "document-key-terms"));
        assert!(!loaded.iter().any(|r| r.id == "contract-key-terms"));
    }
}
