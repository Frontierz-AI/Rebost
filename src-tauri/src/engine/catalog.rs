//! Curated model catalog and the machine-fit recommendation.

use serde::Serialize;

/// Machine profile that drives recommendation and fit labels.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineProfile {
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub cpu: String,
    pub apple_silicon: bool,
    pub accelerator: String,
    pub free_disk_bytes: u64,
    pub process_arch: String,
    pub os_arch: String,
}

impl MachineProfile {
    pub fn detect(data_dir: &std::path::Path) -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
        let cpu = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());
        let free_disk_bytes = sysinfo::Disks::new_with_refreshed_list()
            .iter()
            .filter(|d| data_dir.starts_with(d.mount_point()))
            .map(|d| d.available_space())
            .max()
            .unwrap_or(0);
        let (process_arch, os_arch) = host_arch_labels();
        Self {
            total_ram_bytes: sys.total_memory(),
            available_ram_bytes: sys.available_memory(),
            cpu,
            apple_silicon: cfg!(all(target_os = "macos", target_arch = "aarch64")),
            accelerator: super::pin::preferred_engine_pin()
                .map(|pin| pin.accelerator.to_string())
                .unwrap_or_else(|_| "none".into()),
            free_disk_bytes,
            process_arch,
            os_arch,
        }
    }

    /// RAM the model may use, leaving headroom for the OS, Rebost, Shelves,
    /// search and OCR.
    pub fn model_budget_bytes(&self) -> u64 {
        (self.total_ram_bytes as f64 * 0.55) as u64
    }
}

fn host_arch_labels() -> (String, String) {
    let process = std::env::consts::ARCH.to_string();
    let os = if super::gpu::windows_host_is_arm64() {
        "aarch64".into()
    } else {
        process.clone()
    };
    (process, os)
}

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * 1024 * 1024;

/// Estimated runtime need for a GGUF of `file_bytes` (weights + KV/overhead).
pub fn runtime_need_bytes(file_bytes: u64) -> u64 {
    (file_bytes as f64 * 1.15) as u64 + 2 * GIB
}

/// A curated, ordered catalog used for the first-run recommendation.
/// The app never fetches Artificial Analysis. Rows are general document
/// models (mixed-language shelves, chat, recipes). Coding checkpoints and
/// single-language specialists are not listed; Explore can still find them.
/// Order is capability, highest first. The default install is the first
/// `Documents` row that fits RAM.
pub struct CatalogEntry {
    pub name: &'static str,
    pub family: &'static str,
    pub provider: &'static str,
    pub work: CatalogWork,
    pub hf_repo: &'static str,
    pub approx_bytes: u64,
    pub license: &'static str,
    pub released: &'static str,
    pub blurb: &'static str,
}

/// What Rebost uses the weights for. The first install is always
/// `Documents` — reading and drafting around files, not programming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogWork {
    Documents,
    Code,
}

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        name: "Qwen3.8 27B",
        family: "Qwen",
        provider: "Alibaba",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/Qwen3.8-27B-GGUF",
        approx_bytes: 16314 * MIB,
        license: "Apache-2.0",
        released: "2026-08",
        blurb: "Many languages. Strong on documents and everyday writing.",
    },
    CatalogEntry {
        name: "Muse Glimmer",
        family: "Muse",
        provider: "Meta",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/Muse-Glimmer-30B-GGUF",
        approx_bytes: 15143 * MIB,
        license: "Apache-2.0",
        released: "2026-08",
        blurb: "From Meta. Needs a computer with plenty of memory.",
    },
    CatalogEntry {
        name: "Gemma 4 31B",
        family: "Gemma",
        provider: "Google",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/gemma-4-31B-it-GGUF",
        approx_bytes: 17475 * MIB,
        license: "Apache-2.0",
        released: "2026-04",
        blurb: "Larger Gemma from Google. Big download.",
    },
    CatalogEntry {
        name: "Gemma 4 12B",
        family: "Gemma",
        provider: "Google",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/gemma-4-12b-it-GGUF",
        approx_bytes: 6792 * MIB,
        license: "Apache-2.0",
        released: "2026-05",
        blurb: "From Google. Documents and chat on computers with 24–32 GB of memory.",
    },
    CatalogEntry {
        name: "gpt-oss 20B",
        family: "GPT",
        provider: "OpenAI",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/gpt-oss-20b-GGUF",
        approx_bytes: 11086 * MIB,
        license: "Apache-2.0",
        released: "2025-08",
        blurb: "Open weights from OpenAI.",
    },
    CatalogEntry {
        name: "Gemma 4 E4B",
        family: "Gemma",
        provider: "Google",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/gemma-4-E4B-it-GGUF",
        approx_bytes: 4747 * MIB,
        license: "Apache-2.0",
        released: "2026-03",
        blurb: "From Google. Sized for 16 GB of memory.",
    },
    CatalogEntry {
        name: "Ministral 3 14B",
        family: "Mistral",
        provider: "Mistral",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/Ministral-3-14B-Instruct-2512-GGUF",
        approx_bytes: 7857 * MIB,
        license: "Apache-2.0",
        released: "2025-12",
        blurb: "Comfortable with long documents.",
    },
    CatalogEntry {
        name: "Ministral 3 8B",
        family: "Mistral",
        provider: "Mistral",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/Ministral-3-8B-Instruct-2512-GGUF",
        approx_bytes: 4958 * MIB,
        license: "Apache-2.0",
        released: "2025-12",
        blurb: "Quicker replies.",
    },
    CatalogEntry {
        name: "Ministral 3 3B",
        family: "Mistral",
        provider: "Mistral",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/Ministral-3-3B-Instruct-2512-GGUF",
        approx_bytes: 2047 * MIB,
        license: "Apache-2.0",
        released: "2025-12",
        blurb: "Fits when memory is tight.",
    },
    CatalogEntry {
        name: "Qwen3.5 2B",
        family: "Qwen",
        provider: "Alibaba",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/Qwen3.5-2B-GGUF",
        approx_bytes: 1222 * MIB,
        license: "Apache-2.0",
        released: "2026-01",
        blurb: "For trying Rebost without waiting.",
    },
    CatalogEntry {
        name: "Granite 4.1 3B",
        family: "Granite",
        provider: "IBM",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/granite-4.1-3b-GGUF",
        approx_bytes: 2002 * MIB,
        license: "Apache-2.0",
        released: "2026-04",
        blurb: "Small file, short answers.",
    },
    CatalogEntry {
        name: "Gemma 3 1B",
        family: "Gemma",
        provider: "Google",
        work: CatalogWork::Documents,
        hf_repo: "unsloth/gemma-3-1b-it-GGUF",
        approx_bytes: 769 * MIB,
        license: "Gemma",
        released: "2025-03",
        blurb: "Smallest pick. Installs in a minute.",
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub name: String,
    pub reference: String,
    pub provider: String,
    pub approx_bytes: u64,
    pub license: String,
    pub released: String,
    pub blurb: String,
}

fn to_recommendation(entry: &CatalogEntry) -> Recommendation {
    Recommendation {
        name: entry.name.to_string(),
        reference: entry.hf_repo.to_string(),
        provider: entry.provider.to_string(),
        approx_bytes: entry.approx_bytes,
        license: entry.license.to_string(),
        released: entry.released.to_string(),
        blurb: entry.blurb.to_string(),
    }
}

fn fits(entry: &CatalogEntry, profile: &MachineProfile) -> bool {
    runtime_need_bytes(entry.approx_bytes) <= profile.model_budget_bytes()
}

fn is_default_work(entry: &CatalogEntry) -> bool {
    match entry.work {
        CatalogWork::Documents => true,
        CatalogWork::Code => false,
    }
}

fn forced_recommend_index() -> Option<usize> {
    let name = std::env::var("REBOST_FORCE_RECOMMEND").ok()?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    CATALOG
        .iter()
        .position(|entry| entry.name.eq_ignore_ascii_case(name))
}

fn pick_index(profile: &MachineProfile) -> usize {
    if let Some(index) = forced_recommend_index() {
        return index;
    }
    CATALOG
        .iter()
        .position(|entry| is_default_work(entry) && fits(entry, profile))
        .or_else(|| CATALOG.iter().rposition(is_default_work))
        .unwrap_or(CATALOG.len() - 1)
}

/// Recommended model: the first document-work catalog row that fits in RAM
/// with headroom (catalog is capability order).
pub fn recommend(profile: &MachineProfile) -> Recommendation {
    to_recommendation(&CATALOG[pick_index(profile)])
}

/// Up to `n` smaller catalog models that also fit, each a different family.
/// First is a bit smaller (~40–85% of the primary file); second is
/// significantly smaller (≤40%) when one exists.
pub fn smaller_alternatives(profile: &MachineProfile, n: usize) -> Vec<Recommendation> {
    if n == 0 {
        return Vec::new();
    }
    let primary = &CATALOG[pick_index(profile)];
    let bit_max = primary.approx_bytes.saturating_mul(85) / 100;
    let bit_min = primary.approx_bytes.saturating_mul(40) / 100;
    let significant_max = primary.approx_bytes.saturating_mul(40) / 100;

    let mut by_family: Vec<&CatalogEntry> = Vec::new();
    for entry in CATALOG {
        if entry.name == primary.name || entry.family == primary.family {
            continue;
        }
        if !is_default_work(entry) {
            continue;
        }
        if !fits(entry, profile) || entry.approx_bytes > bit_max {
            continue;
        }
        match by_family
            .iter_mut()
            .find(|picked| picked.family == entry.family)
        {
            Some(existing) => {
                if entry.approx_bytes > existing.approx_bytes {
                    *existing = entry;
                }
            }
            None => by_family.push(entry),
        }
    }
    by_family.sort_by_key(|b| std::cmp::Reverse(b.approx_bytes));

    let mut chosen: Vec<&CatalogEntry> = Vec::new();
    if let Some(bit) = by_family
        .iter()
        .find(|entry| entry.approx_bytes >= bit_min)
        .copied()
        .or_else(|| by_family.first().copied())
    {
        chosen.push(bit);
    }
    if n >= 2 && !chosen.is_empty() {
        if let Some(significant) = by_family
            .iter()
            .copied()
            .find(|entry| entry.name != chosen[0].name && entry.approx_bytes <= significant_max)
        {
            chosen.push(significant);
        } else if let Some(fallback) = by_family.last().copied() {
            if chosen.iter().all(|picked| picked.name != fallback.name) {
                chosen.push(fallback);
            }
        }
    }
    chosen.truncate(n);
    chosen.into_iter().map(to_recommendation).collect()
}

/// Up to `n` catalog models that fit this machine and are not already
/// installed. Same order as first run: the default document pick, then
/// smaller alternatives from other families.
pub fn uninstalled_suggestions(
    profile: &MachineProfile,
    installed_reference: Option<&str>,
    n: usize,
) -> Vec<Recommendation> {
    if n == 0 {
        return Vec::new();
    }
    let skip = |reference: &str| {
        installed_reference.is_some_and(|installed| installed.eq_ignore_ascii_case(reference))
    };
    let mut out = Vec::new();
    let primary = recommend(profile);
    if fits(&CATALOG[pick_index(profile)], profile) && !skip(&primary.reference) {
        out.push(primary);
    }
    for alt in smaller_alternatives(profile, n) {
        if skip(&alt.reference) {
            continue;
        }
        if out.iter().any(|picked| picked.reference == alt.reference) {
            continue;
        }
        out.push(alt);
        if out.len() >= n {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommendation_scales_with_ram() {
        let mk = |gb: u64| MachineProfile {
            total_ram_bytes: gb * GIB,
            available_ram_bytes: gb * GIB / 2,
            cpu: "test".into(),
            apple_silicon: true,
            accelerator: "Metal".into(),
            free_disk_bytes: 500 * GIB,
            process_arch: "test".into(),
            os_arch: "test".into(),
        };
        assert_eq!(recommend(&mk(48)).name, "Qwen3.8 27B");
        assert_eq!(recommend(&mk(32)).name, "Gemma 4 12B");
        assert_eq!(recommend(&mk(24)).name, "Gemma 4 12B");
        assert_eq!(recommend(&mk(16)).name, "Gemma 4 E4B");
        assert_eq!(recommend(&mk(8)).name, "Ministral 3 3B");

        let alts48: Vec<_> = smaller_alternatives(&mk(48), 2)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(alts48, ["gpt-oss 20B", "Granite 4.1 3B"]);
        let alts24: Vec<_> = smaller_alternatives(&mk(24), 2)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(alts24, ["Ministral 3 8B", "Granite 4.1 3B"]);
        assert_eq!(
            smaller_alternatives(&mk(16), 2)
                .into_iter()
                .map(|r| r.name)
                .collect::<Vec<_>>(),
            ["Ministral 3 3B", "Qwen3.5 2B"]
        );
        assert_eq!(
            smaller_alternatives(&mk(8), 2)
                .into_iter()
                .map(|r| r.name)
                .collect::<Vec<_>>(),
            ["Qwen3.5 2B", "Gemma 3 1B"]
        );
        assert!(smaller_alternatives(&mk(4), 2).is_empty());
    }

    #[test]
    fn default_install_is_document_work() {
        assert!(
            CATALOG
                .iter()
                .all(|entry| entry.work == CatalogWork::Documents),
            "coding checkpoints do not belong in the first-run catalog"
        );
    }

    #[test]
    fn uninstalled_suggestions_skip_the_active_model() {
        let mk = |gb: u64| MachineProfile {
            total_ram_bytes: gb * GIB,
            available_ram_bytes: gb * GIB / 2,
            cpu: "test".into(),
            apple_silicon: true,
            accelerator: "Metal".into(),
            free_disk_bytes: 500 * GIB,
            process_arch: "test".into(),
            os_arch: "test".into(),
        };
        let none: Vec<_> = uninstalled_suggestions(&mk(48), None, 2)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(none, ["Qwen3.8 27B", "gpt-oss 20B"]);

        let installed = recommend(&mk(48)).reference;
        let skipped: Vec<_> = uninstalled_suggestions(&mk(48), Some(&installed), 2)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(skipped, ["gpt-oss 20B", "Granite 4.1 3B"]);
        assert!(uninstalled_suggestions(&mk(4), None, 2).is_empty());
    }

    #[test]
    fn gemma_license_strings_match_upstream_policy() {
        // Gemma 4 is Apache-2.0; Gemma 3 (and earlier) stay on Gemma Terms.
        // https://opensource.googleblog.com/2026/03/gemma-4-expanding-the-gemmaverse-with-apache-20.html
        for entry in CATALOG {
            if entry.name.starts_with("Gemma 4") {
                assert_eq!(entry.license, "Apache-2.0", "{}", entry.name);
            }
            if entry.name.starts_with("Gemma 3") {
                assert_eq!(entry.license, "Gemma", "{}", entry.name);
            }
        }
    }
}
