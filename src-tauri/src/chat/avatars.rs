//! Conversation faces. Ids match `src/lib/avatars.ts`.

use std::collections::HashSet;

pub struct Avatar {
    pub id: &'static str,
    pub name: &'static str,
}

/// Spoken name is the animal, not the filename. Numbered variants share a name.
pub const AVATARS: &[Avatar] = &[
    Avatar {
        id: "alpaca",
        name: "Alpaca",
    },
    Avatar {
        id: "arctic-fox",
        name: "Arctic fox",
    },
    Avatar {
        id: "bear",
        name: "Bear",
    },
    Avatar {
        id: "cheetah",
        name: "Cheetah",
    },
    Avatar {
        id: "cheetah-2",
        name: "Cheetah",
    },
    Avatar {
        id: "chimpanzee",
        name: "Chimpanzee",
    },
    Avatar {
        id: "dolphin",
        name: "Dolphin",
    },
    Avatar {
        id: "fennec",
        name: "Fennec",
    },
    Avatar {
        id: "gazelle",
        name: "Gazelle",
    },
    Avatar {
        id: "gazelle-2",
        name: "Gazelle",
    },
    Avatar {
        id: "giraffe",
        name: "Giraffe",
    },
    Avatar {
        id: "koala",
        name: "Koala",
    },
    Avatar {
        id: "llama",
        name: "Llama",
    },
    Avatar {
        id: "lynx",
        name: "Lynx",
    },
    Avatar {
        id: "panda",
        name: "Panda",
    },
    Avatar {
        id: "raccoon",
        name: "Raccoon",
    },
    Avatar {
        id: "red-panda",
        name: "Red panda",
    },
    Avatar {
        id: "red-panda-2",
        name: "Red panda",
    },
    Avatar {
        id: "retriever",
        name: "Retriever",
    },
    Avatar {
        id: "shiba",
        name: "Shiba",
    },
    Avatar {
        id: "snow-leopard",
        name: "Snow leopard",
    },
    Avatar {
        id: "snow-leopard-2",
        name: "Snow leopard",
    },
    Avatar {
        id: "tiger",
        name: "Tiger",
    },
    Avatar {
        id: "wolf",
        name: "Wolf",
    },
];

pub fn name_for(id: &str) -> Option<&'static str> {
    AVATARS
        .iter()
        .find(|avatar| avatar.id == id)
        .map(|a| a.name)
}

pub fn pick_id(thread_id: &str, used: &HashSet<String>) -> &'static str {
    let unused: Vec<&Avatar> = AVATARS
        .iter()
        .filter(|avatar| !used.contains(avatar.id))
        .collect();
    if unused.is_empty() {
        return AVATARS[hash_thread_id(thread_id) % AVATARS.len()].id;
    }
    unused[hash_thread_id(thread_id) % unused.len()].id
}

fn hash_thread_id(thread_id: &str) -> usize {
    let mut hash: u32 = 0;
    for byte in thread_id.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u32::from(byte));
    }
    hash as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_catalog_ids() {
        assert_eq!(name_for("cheetah"), Some("Cheetah"));
        assert_eq!(name_for("cheetah-2"), Some("Cheetah"));
        assert_eq!(name_for("arctic-fox"), Some("Arctic fox"));
        assert_eq!(name_for("nope"), None);
    }

    #[test]
    fn pick_skips_faces_already_in_use() {
        let used: HashSet<String> = AVATARS
            .iter()
            .skip(1)
            .map(|avatar| avatar.id.to_string())
            .collect();
        assert_eq!(pick_id("t_anything", &used), AVATARS[0].id);
    }

    #[test]
    fn pick_is_stable_when_every_face_is_taken() {
        let used: HashSet<String> = AVATARS.iter().map(|avatar| avatar.id.to_string()).collect();
        assert_eq!(pick_id("same-thread", &used), pick_id("same-thread", &used));
        assert!(name_for(pick_id("same-thread", &used)).is_some());
    }
}
