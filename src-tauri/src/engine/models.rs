//! Hugging Face and Ollama search, and GGUF install resolution.

use anyhow::{anyhow, Context, Result};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub use crate::engine::catalog::{
    recommend, runtime_need_bytes, smaller_alternatives, uninstalled_suggestions, CatalogEntry,
    MachineProfile, Recommendation, CATALOG,
};

/// Quantization preference for automatic file selection.
const QUANT_PREFERENCE: &[&str] = &[
    "Q4_K_M", "Q4_K_XL", "Q4_K_S", "IQ4_XS", "Q5_K_M", "Q4_0", "Q5_0", "Q6_K", "Q8_0", "F16",
    "BF16",
];

/// How many Hugging Face hits to show (after ranking) and resolve file sizes for.
const HF_RESULT_LIMIT: usize = 18;

/// Names that signal a file/repo llama-server can't chat with.
fn is_unusable_artifact(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "mmproj",
        "embedding",
        "embed-",
        "reranker",
        "rerank",
        "whisper",
        "clip",
        "vae",
        "encoder",
    ]
    .iter()
    .any(|t| lower.contains(t))
}

/// Pick the best single-file GGUF from a repo file listing.
pub fn pick_gguf(files: &[(String, Option<u64>)]) -> Option<(String, Option<u64>)> {
    let candidates: Vec<&(String, Option<u64>)> = files
        .iter()
        .filter(|(name, _)| name.to_lowercase().ends_with(".gguf"))
        .filter(|(name, _)| !is_unusable_artifact(name))
        // Multi-part files (…-00001-of-00003.gguf) need merging — skip.
        .filter(|(name, _)| !name.contains("-of-"))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    for quant in QUANT_PREFERENCE {
        if let Some(hit) = candidates
            .iter()
            .find(|(name, _)| name.to_uppercase().contains(quant))
        {
            return Some((*hit).clone());
        }
    }
    Some(candidates[0].clone())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSearchResult {
    /// Merge key (normalized base name).
    pub id: String,
    /// Display name, e.g. "Muse Glimmer 7B".
    pub name: String,
    /// "huggingface" | "ollama" | "huggingface+ollama"
    pub source: String,
    /// HF repo id or Ollama library name.
    pub reference: String,
    /// Chosen GGUF file, when known.
    pub file: Option<String>,
    pub size_bytes: Option<u64>,
    pub license: Option<String>,
    /// Repo creation date (`YYYY-MM-DD`), from Hugging Face `createdAt`.
    pub released: Option<String>,
    /// Hugging Face all-time downloads when the Hub publishes them.
    pub downloads: Option<u64>,
    /// Hugging Face namespace (the publisher), e.g. "Qwen".
    pub publisher: Option<String>,
    /// True when this repo is from the original maker for this search.
    pub official: bool,
    /// Whether it fits this computer (None = size unknown).
    pub fits: Option<bool>,
}

fn normalize_model_key(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name).to_lowercase();
    let base = base
        .trim_end_matches(".gguf")
        .replace("-gguf", "")
        .replace("_gguf", "");
    base.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn display_name_from_repo(repo: &str) -> String {
    let base = repo.rsplit('/').next().unwrap_or(repo);
    base.replace("-GGUF", "")
        .replace("-gguf", "")
        .replace(['-', '_'], " ")
        .trim()
        .to_string()
}

#[derive(Deserialize)]
struct HfModel {
    /// The API sends both `id` and `modelId` — keep them as separate
    /// optional fields (an alias would trip serde's duplicate-field check).
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "modelId")]
    model_id: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "createdAt")]
    created_at: Option<String>,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default, rename = "downloadsAllTime")]
    downloads_all_time: Option<u64>,
    #[serde(default)]
    siblings: Option<Vec<HfSibling>>,
}

impl HfModel {
    fn repo_id(&self) -> Option<String> {
        self.model_id.clone().or_else(|| self.id.clone())
    }

    fn download_count(&self) -> Option<u64> {
        self.downloads_all_time.or(self.downloads)
    }

    fn released_on(&self) -> Option<String> {
        self.created_at
            .as_deref()
            .or(self.last_modified.as_deref())
            .map(|d| d.chars().take(10).collect())
    }
}

#[derive(Deserialize)]
struct HfSibling {
    rfilename: String,
}

#[derive(Deserialize)]
struct HfTreeEntry {
    path: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    lfs: Option<HfLfs>,
}

#[derive(Deserialize)]
struct HfLfs {
    #[serde(default)]
    size: Option<u64>,
    /// SHA-256 of the file — used to verify model downloads.
    #[serde(default)]
    oid: Option<String>,
}

fn is_hf_namespace(value: &str) -> bool {
    let value = value.trim();
    (2..=64).contains(&value.len())
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// `Qwen3` / `Qwen2.5` → `Qwen`, so an author= query can hit the maker's org.
fn strip_trailing_version(query: &str) -> String {
    query
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
        .to_string()
}

fn publisher_guesses(query: &str) -> Vec<String> {
    let query = query.trim();
    let mut names = Vec::new();
    if is_hf_namespace(query) {
        names.push(query.to_string());
    }
    let stripped = strip_trailing_version(query);
    if is_hf_namespace(&stripped) && !stripped.eq_ignore_ascii_case(query) {
        names.push(stripped);
    }
    names
}

fn author_matches_search(author: &str, query: &str) -> bool {
    let author = author.to_ascii_lowercase();
    let query = query.trim().to_ascii_lowercase();
    if query.len() < 2 {
        return false;
    }
    if author == query {
        return true;
    }
    // Search "Qwen3" should still treat org "Qwen" as the maker.
    if query.starts_with(&author) && author.len() >= 3 {
        return true;
    }
    if author.starts_with(&query) && query.len() >= 3 {
        return true;
    }
    // Search "llama" vs org "meta-llama".
    query.len() >= 4 && author.contains(&query)
}

fn base_model_owner(tag: &str) -> Option<&str> {
    let rest = tag.strip_prefix("base_model:")?;
    // Skip Hub prefixes like quantized:, finetune:, adapter:.
    if rest.contains(':') {
        return None;
    }
    let owner = rest.split('/').next()?;
    (!owner.is_empty()).then_some(owner)
}

fn majority_base_model_owner(models: &[HfModel]) -> Option<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for model in models {
        for tag in &model.tags {
            if let Some(owner) = base_model_owner(tag) {
                *counts.entry(owner.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n >= 2)
        .max_by_key(|(_, n)| *n)
        .map(|(owner, _)| owner)
}

fn author_count(models: &[HfModel], name: &str) -> usize {
    models
        .iter()
        .filter(|model| {
            model
                .author
                .as_deref()
                .is_some_and(|author| author.eq_ignore_ascii_case(name))
        })
        .count()
}

fn base_model_count(models: &[HfModel], name: &str) -> usize {
    models
        .iter()
        .flat_map(|model| model.tags.iter())
        .filter_map(|tag| base_model_owner(tag))
        .filter(|owner| owner.eq_ignore_ascii_case(name))
        .count()
}

fn official_namespaces(query: &str, models: &[HfModel]) -> Vec<String> {
    let mut candidates = publisher_guesses(query);
    for model in models {
        if let Some(author) = &model.author {
            if author_matches_search(author, query)
                && !candidates.iter().any(|n| n.eq_ignore_ascii_case(author))
            {
                candidates.push(author.clone());
            }
        }
    }
    let strong: Vec<String> = candidates
        .iter()
        .filter(|name| author_count(models, name) >= 2 || base_model_count(models, name) >= 2)
        .cloned()
        .collect();
    if !strong.is_empty() {
        return strong;
    }
    if let Some(owner) = majority_base_model_owner(models) {
        return vec![owner];
    }
    candidates
        .into_iter()
        .filter(|name| author_count(models, name) > 0)
        .collect()
}

fn is_official_publisher(author: Option<&str>, namespaces: &[String]) -> bool {
    let Some(author) = author else {
        return false;
    };
    namespaces
        .iter()
        .any(|name| name.eq_ignore_ascii_case(author))
}

fn dedup_models(models: &mut Vec<HfModel>) {
    let mut seen = HashSet::new();
    models.retain(|model| {
        model
            .repo_id()
            .is_some_and(|id| seen.insert(id.to_ascii_lowercase()))
    });
}

fn rank_search_results(results: &mut [ModelSearchResult]) {
    results.sort_by(|a, b| {
        b.official
            .cmp(&a.official)
            .then_with(|| match (&a.released, &b.released) {
                (Some(left), Some(right)) => right.cmp(left),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then_with(|| b.downloads.unwrap_or(0).cmp(&a.downloads.unwrap_or(0)))
    });
}

const HF_EXPAND: &[&str] = &[
    "author",
    "createdAt",
    "downloads",
    "downloadsAllTime",
    "siblings",
    "tags",
];

/// Hugging Face Hub list endpoint. Empty on network/HTTP errors so search
/// can still return the other catalog.
async fn hf_list_models(
    client: &reqwest::Client,
    search: Option<&str>,
    author: Option<&str>,
    limit: u32,
    sort_by_created: bool,
) -> Vec<HfModel> {
    let mut query: Vec<(&str, String)> =
        vec![("filter", "gguf".into()), ("limit", limit.to_string())];
    if let Some(search) = search {
        query.push(("search", search.to_string()));
    }
    if let Some(author) = author {
        query.push(("author", author.to_string()));
    }
    if sort_by_created {
        query.push(("sort", "createdAt".into()));
        query.push(("direction", "-1".into()));
    }
    for field in HF_EXPAND {
        query.push(("expand", (*field).into()));
    }
    let response = match client
        .get("https://huggingface.co/api/models")
        .query(&query)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            log::debug!("hugging face list: {error:#}");
            return Vec::new();
        }
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    response.json().await.unwrap_or_default()
}

async fn hf_models_for_query(client: &reqwest::Client, query: &str) -> Vec<HfModel> {
    let guesses = publisher_guesses(query);
    let author_fetches = guesses.iter().map(|guess| {
        let client = client.clone();
        let author = guess.clone();
        let search = if guess.eq_ignore_ascii_case(query) {
            None
        } else {
            Some(query.to_string())
        };
        async move { hf_list_models(&client, search.as_deref(), Some(&author), 20, true).await }
    });
    let (mut models, author_lists) = tokio::join!(
        hf_list_models(client, Some(query), None, 40, false),
        join_all(author_fetches)
    );
    for list in author_lists {
        models.extend(list);
    }
    dedup_models(&mut models);

    let namespaces = official_namespaces(query, &models);
    let missing: Vec<String> = namespaces
        .iter()
        .filter(|name| author_count(&models, name) < 3)
        .cloned()
        .collect();
    if !missing.is_empty() {
        let extra =
            join_all(
                missing.into_iter().map(|name| {
                    let client = client.clone();
                    let search = if name.eq_ignore_ascii_case(query) {
                        None
                    } else {
                        Some(query.to_string())
                    };
                    async move {
                        hf_list_models(&client, search.as_deref(), Some(&name), 20, true).await
                    }
                }),
            )
            .await;
        for list in extra {
            models.extend(list);
        }
        dedup_models(&mut models);
    }
    models
}

async fn gguf_file_size(client: &reqwest::Client, repo: &str, file: &str) -> Option<u64> {
    let response = client
        .get(format!(
            "https://huggingface.co/api/models/{repo}/tree/main"
        ))
        .send()
        .await
        .ok()?;
    let entries: Vec<HfTreeEntry> = response.json().await.ok()?;
    entries
        .iter()
        .find(|entry| entry.path == file)
        .and_then(|entry| entry.size.or(entry.lfs.as_ref().and_then(|lfs| lfs.size)))
}

/// Search the Hugging Face catalog for GGUF builds.
async fn search_huggingface(
    client: &reqwest::Client,
    query: &str,
    profile: &MachineProfile,
) -> Result<Vec<ModelSearchResult>> {
    let models = hf_models_for_query(client, query).await;
    let namespaces = official_namespaces(query, &models);
    let mut results = Vec::new();
    for model in models {
        let Some(repo) = model.repo_id() else {
            continue;
        };
        if is_unusable_artifact(&repo) {
            continue;
        }
        let files: Vec<(String, Option<u64>)> = model
            .siblings
            .iter()
            .flatten()
            .map(|s| (s.rfilename.clone(), None))
            .collect();
        let Some((file, _)) = pick_gguf(&files) else {
            continue;
        };
        let license = model
            .tags
            .iter()
            .find_map(|t| t.strip_prefix("license:"))
            .map(|l| l.to_string());
        let official = is_official_publisher(model.author.as_deref(), &namespaces);
        results.push(ModelSearchResult {
            id: normalize_model_key(&repo),
            name: display_name_from_repo(&repo),
            source: "huggingface".into(),
            reference: repo,
            file: Some(file),
            size_bytes: None,
            license,
            released: model.released_on(),
            downloads: model.download_count(),
            publisher: model.author.clone(),
            official,
            fits: None,
        });
    }
    rank_search_results(&mut results);
    results.truncate(HF_RESULT_LIMIT);

    let budget = profile.model_budget_bytes();
    let sizes = join_all(results.iter().map(|result| {
        let client = client.clone();
        let repo = result.reference.clone();
        let file = result.file.clone();
        async move {
            match file {
                Some(file) => gguf_file_size(&client, &repo, &file).await,
                None => None,
            }
        }
    }))
    .await;
    for (result, size_bytes) in results.iter_mut().zip(sizes) {
        result.size_bytes = size_bytes;
        result.fits = size_bytes.map(|size| runtime_need_bytes(size) <= budget);
    }
    Ok(results)
}

#[derive(Deserialize)]
struct OllamaManifest {
    #[serde(default)]
    layers: Vec<OllamaLayer>,
}

#[derive(Deserialize, Clone)]
struct OllamaLayer {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    #[serde(default)]
    size: Option<u64>,
}

/// Resolve one Ollama library model via its registry manifest.
async fn ollama_manifest(
    client: &reqwest::Client,
    name: &str,
) -> Result<(OllamaLayer, Option<String>)> {
    let url = format!("https://registry.ollama.ai/v2/library/{name}/manifests/latest");
    let manifest: OllamaManifest = client
        .get(&url)
        .header(
            "Accept",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let model_layer = manifest
        .layers
        .iter()
        .find(|l| l.media_type == "application/vnd.ollama.image.model")
        .cloned()
        .ok_or_else(|| anyhow!("no model layer for {name}"))?;
    let license_digest = manifest
        .layers
        .iter()
        .find(|l| l.media_type == "application/vnd.ollama.image.license")
        .map(|l| l.digest.clone());

    let license = if let Some(digest) = license_digest {
        let blob_url = format!("https://registry.ollama.ai/v2/library/{name}/blobs/{digest}");
        match client.get(&blob_url).send().await {
            Ok(response) => response
                .text()
                .await
                .ok()
                .map(|text| classify_license(&text)),
            Err(_) => None,
        }
    } else {
        None
    };
    Ok((model_layer, license))
}

fn classify_license(text: &str) -> String {
    let head: String = text.chars().take(400).collect::<String>().to_lowercase();
    if head.contains("apache license") {
        "Apache-2.0".into()
    } else if head.contains("mit license") {
        "MIT".into()
    } else if head.contains("gemma") {
        "Gemma".into()
    } else if head.contains("llama 3") {
        "Llama 3".into()
    } else if head.contains("qwen") {
        "Qwen".into()
    } else {
        "See model page".into()
    }
}

/// Search the Ollama library (best effort — the library has no official
/// API, so this parses the search page and degrades gracefully).
async fn search_ollama(
    client: &reqwest::Client,
    query: &str,
    profile: &MachineProfile,
) -> Vec<ModelSearchResult> {
    let Ok(response) = client
        .get("https://ollama.com/search")
        .query(&[("q", query)])
        .send()
        .await
    else {
        return Vec::new();
    };
    let Ok(html) = response.text().await else {
        return Vec::new();
    };
    let re = regex::Regex::new(r#"href="/library/([a-z0-9][a-z0-9._-]*)""#).unwrap();
    let mut names: Vec<String> = Vec::new();
    for capture in re.captures_iter(&html) {
        let name = capture[1].to_string();
        if !names.contains(&name) {
            names.push(name);
        }
        if names.len() >= 6 {
            break;
        }
    }
    let budget = profile.model_budget_bytes();
    let mut results = Vec::new();
    for name in names {
        match ollama_manifest(client, &name).await {
            Ok((layer, license)) => {
                let fits = layer.size.map(|s| runtime_need_bytes(s) <= budget);
                results.push(ModelSearchResult {
                    id: normalize_model_key(&name),
                    name: name.replace(['-', '_'], " "),
                    source: "ollama".into(),
                    reference: name.clone(),
                    file: Some(format!("{}.gguf", name.replace([':', '/'], "-"))),
                    size_bytes: layer.size,
                    license,
                    released: None,
                    downloads: None,
                    publisher: None,
                    official: false,
                    fits,
                });
            }
            Err(error) => {
                log::debug!("ollama manifest {name}: {error:#}");
            }
        }
    }
    results
}

/// Explore other models: Hugging Face + Ollama, duplicates merged.
/// Company files are never sent anywhere — only this query string.
pub async fn search_models(
    client: &reqwest::Client,
    query: &str,
    profile: &MachineProfile,
) -> Result<Vec<ModelSearchResult>> {
    let (hf, ollama) = tokio::join!(
        search_huggingface(client, query, profile),
        search_ollama(client, query, profile)
    );
    let mut results = hf.unwrap_or_else(|error| {
        log::warn!("hugging face search: {error:#}");
        Vec::new()
    });
    for extra in ollama {
        if let Some(existing) = results.iter_mut().find(|r| r.id == extra.id) {
            existing.source = "huggingface+ollama".into();
            if existing.license.is_none() {
                existing.license = extra.license.clone();
            }
            if existing.size_bytes.is_none() {
                existing.size_bytes = extra.size_bytes;
                existing.fits = extra.fits;
            }
        } else {
            results.push(extra);
        }
    }
    rank_search_results(&mut results);
    Ok(results)
}

/// A concrete, verifiable model download.
pub struct ResolvedDownload {
    pub url: String,
    pub file_name: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

/// Map UI source strings onto the two catalogs we actually fetch from.
pub fn normalize_source(source: &str) -> Result<&'static str> {
    match source {
        "huggingface" | "huggingface+ollama" => Ok("huggingface"),
        "ollama" => Ok("ollama"),
        _ => Err(anyhow!("unsupported model source")),
    }
}

/// Hugging Face `owner/repo` or an Ollama library name (optional `:tag`).
pub fn validate_reference(source: &str, reference: &str) -> Result<()> {
    if reference.is_empty() || reference.len() > 200 {
        return Err(anyhow!("invalid model reference"));
    }
    if reference.contains("..") || reference.contains('\\') || reference.contains('\0') {
        return Err(anyhow!("invalid model reference"));
    }
    match source {
        "huggingface" => {
            let mut parts = reference.split('/');
            let owner = parts.next().unwrap_or("");
            let repo = parts.next().unwrap_or("");
            if parts.next().is_some() || owner.is_empty() || repo.is_empty() {
                return Err(anyhow!("invalid Hugging Face reference"));
            }
            let ok = |s: &str| {
                s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                    && s.starts_with(|c: char| c.is_ascii_alphanumeric())
            };
            if !ok(owner) || !ok(repo) {
                return Err(anyhow!("invalid Hugging Face reference"));
            }
            Ok(())
        }
        "ollama" => {
            let (name, tag) = match reference.split_once(':') {
                Some((name, tag)) => (name, Some(tag)),
                None => (reference, None),
            };
            if name.is_empty()
                || name.contains('/')
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                return Err(anyhow!("invalid Ollama reference"));
            }
            if let Some(tag) = tag {
                if tag.is_empty()
                    || !tag
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
                {
                    return Err(anyhow!("invalid Ollama reference"));
                }
            }
            Ok(())
        }
        _ => Err(anyhow!("unsupported model source")),
    }
}

/// Keep GGUF weights inside the models directory: basename only, `.gguf`.
pub fn safe_model_file_name(name: &str) -> Result<String> {
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if base != name || base.is_empty() || base.contains("..") {
        return Err(anyhow!("invalid model file name"));
    }
    if !base.to_ascii_lowercase().ends_with(".gguf") {
        return Err(anyhow!("model file must be .gguf"));
    }
    if !base
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(anyhow!("invalid model file name"));
    }
    Ok(base.to_string())
}

/// Resolve the concrete GGUF (url + file name + size + checksum) for an
/// install. Quantization and exact file name are chosen automatically.
pub async fn resolve_download(
    client: &reqwest::Client,
    source: &str,
    reference: &str,
) -> Result<ResolvedDownload> {
    let source = normalize_source(source)?;
    validate_reference(source, reference)?;
    match source {
        "ollama" => {
            let (layer, _) = ollama_manifest(client, reference).await?;
            let url = format!(
                "https://registry.ollama.ai/v2/library/{reference}/blobs/{}",
                layer.digest
            );
            let file_name = format!("{}.gguf", reference.replace([':', '/'], "-"));
            let sha256 = layer.digest.strip_prefix("sha256:").map(str::to_string);
            Ok(ResolvedDownload {
                url,
                file_name,
                size: layer.size,
                sha256,
            })
        }
        _ => {
            let entries: Vec<HfTreeEntry> = client
                .get(format!(
                    "https://huggingface.co/api/models/{reference}/tree/main"
                ))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await
                .context("hugging face file listing")?;
            let files: Vec<(String, Option<u64>)> = entries
                .iter()
                .map(|e| {
                    (
                        e.path.clone(),
                        e.size.or(e.lfs.as_ref().and_then(|l| l.size)),
                    )
                })
                .collect();
            let (file, size) =
                pick_gguf(&files).ok_or_else(|| anyhow!("no usable model file in {reference}"))?;
            let sha256 = entries
                .iter()
                .find(|e| e.path == file)
                .and_then(|e| e.lfs.as_ref())
                .and_then(|l| l.oid.clone());
            let url =
                format!("https://huggingface.co/{reference}/resolve/main/{file}?download=true");
            let file_name = file.rsplit('/').next().unwrap_or(&file).to_string();
            Ok(ResolvedDownload {
                url,
                file_name,
                size,
                sha256,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gguf_picker_prefers_q4_k_m_and_skips_junk() {
        let files = vec![
            ("model-mmproj-F16.gguf".to_string(), None),
            ("Model-Q8_0.gguf".to_string(), None),
            ("Model-Q4_K_M.gguf".to_string(), None),
            ("Model-Q4_K_M-00001-of-00002.gguf".to_string(), None),
            ("README.md".to_string(), None),
        ];
        let (picked, _) = pick_gguf(&files).unwrap();
        assert_eq!(picked, "Model-Q4_K_M.gguf");
    }

    #[test]
    fn merge_keys_align_hf_and_ollama() {
        assert_eq!(
            normalize_model_key("Qwen/Qwen3-14B-GGUF"),
            normalize_model_key("qwen3-14b")
        );
    }

    #[test]
    fn install_rejects_malicious_references_and_names() {
        assert!(normalize_source("ftp").is_err());
        assert_eq!(
            normalize_source("huggingface+ollama").unwrap(),
            "huggingface"
        );
        assert!(validate_reference("huggingface", "../../etc/passwd").is_err());
        assert!(validate_reference("huggingface", "owner/repo/../../../tmp").is_err());
        assert!(validate_reference("huggingface", "https://evil.example/x").is_err());
        assert!(validate_reference("huggingface", "owner/repo?download=1").is_err());
        assert!(validate_reference("ollama", "library/../../../etc").is_err());
        assert!(validate_reference("huggingface", "unsloth/Muse-Glimmer-30B-GGUF").is_ok());
        assert!(safe_model_file_name("../x.gguf").is_err());
        assert!(safe_model_file_name("foo/bar.gguf").is_err());
        assert!(safe_model_file_name("weights.bin").is_err());
        assert_eq!(
            safe_model_file_name("Muse-Glimmer-30B-Q4_K_M.gguf").unwrap(),
            "Muse-Glimmer-30B-Q4_K_M.gguf"
        );
    }

    fn hf_hit(author: &str, tags: &[&str]) -> HfModel {
        HfModel {
            id: Some(format!("{author}/model")),
            model_id: None,
            author: Some(author.into()),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            created_at: None,
            last_modified: None,
            downloads: None,
            downloads_all_time: None,
            siblings: None,
        }
    }

    fn search_hit(
        name: &str,
        official: bool,
        released: Option<&str>,
        downloads: Option<u64>,
    ) -> ModelSearchResult {
        ModelSearchResult {
            id: name.into(),
            name: name.into(),
            source: "huggingface".into(),
            reference: format!("org/{name}"),
            file: None,
            size_bytes: None,
            license: None,
            released: released.map(str::to_string),
            downloads,
            publisher: Some("org".into()),
            official,
            fits: None,
        }
    }

    #[test]
    fn qwen_search_guesses_the_qwen_org() {
        assert_eq!(publisher_guesses("Qwen"), vec!["Qwen"]);
        assert_eq!(publisher_guesses("Qwen3"), vec!["Qwen3", "Qwen"]);
        assert_eq!(publisher_guesses("Qwen2.5"), vec!["Qwen2.5", "Qwen"]);
        assert!(author_matches_search("Qwen", "Qwen3"));
        assert!(author_matches_search("meta-llama", "llama"));
        assert!(!author_matches_search("unsloth", "Qwen"));
    }

    #[test]
    fn official_namespaces_prefer_the_maker_not_quantizers() {
        let models = vec![
            hf_hit("unsloth", &["base_model:Qwen/Qwen3-8B"]),
            hf_hit("unsloth", &["base_model:Qwen/Qwen3-14B"]),
            hf_hit("bartowski", &["base_model:Qwen/Qwen3-8B"]),
        ];
        let names = official_namespaces("Qwen", &models);
        assert!(
            names.iter().any(|n| n.eq_ignore_ascii_case("Qwen")),
            "Qwen org should be inferred: {names:?}"
        );
        assert!(!names.iter().any(|n| n.eq_ignore_ascii_case("unsloth")));
    }

    #[test]
    fn gemma_search_uses_base_model_owner_when_org_is_not_the_query() {
        let models = vec![
            hf_hit("unsloth", &["base_model:google/gemma-4-9b-it"]),
            hf_hit("bartowski", &["base_model:google/gemma-4-9b-it"]),
            hf_hit("google", &["base_model:google/gemma-4-9b-it"]),
        ];
        let names = official_namespaces("Gemma", &models);
        assert!(
            names.iter().any(|n| n.eq_ignore_ascii_case("google")),
            "google should win via base_model tags: {names:?}"
        );
    }

    #[test]
    fn rank_puts_official_first_then_newest() {
        let mut results = vec![
            search_hit("community-new", false, Some("2026-08-14"), Some(9_000_000)),
            search_hit("official-old", true, Some("2025-01-01"), Some(100)),
            search_hit("official-new", true, Some("2026-02-02"), Some(50)),
            search_hit("undated", false, None, Some(1)),
        ];
        rank_search_results(&mut results);
        let names: Vec<_> = results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["official-new", "official-old", "community-new", "undated"]
        );
    }

    #[test]
    fn base_model_owner_ignores_quantized_and_adapter_prefixes() {
        assert_eq!(base_model_owner("base_model:Qwen/Qwen3-8B"), Some("Qwen"));
        assert_eq!(base_model_owner("base_model:quantized:Qwen/Qwen3-8B"), None);
        assert_eq!(base_model_owner("base_model:adapter:DavidAU/foo"), None);
    }
}
