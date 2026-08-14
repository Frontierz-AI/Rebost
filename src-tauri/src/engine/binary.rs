//! Download, verify, and unpack the pinned llama.cpp archive.

use anyhow::{anyhow, Context, Result};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use super::download;
use super::pin::{current_engine_pin, llama_server_file_name, ENGINE_BUILD};
use super::process::unpack_engine_archive;
use super::{Engine, EngineState};

impl Engine {
    fn engine_binary(&self) -> PathBuf {
        self.ctx
            .paths
            .engine_dir()
            .join(ENGINE_BUILD)
            .join(llama_server_file_name())
    }

    /// Make sure the pinned llama.cpp build is present (unpack + verify on
    /// first need). Preference: already extracted, `REBOST_ENGINE_ARCHIVE`,
    /// the archive bundled in the app, then a GitHub download.
    pub async fn ensure_binary(&self) -> Result<PathBuf> {
        let binary = self.engine_binary();
        if binary.exists() {
            return Ok(binary);
        }
        let pin = current_engine_pin()?;
        self.set_status(EngineState::Starting, Some("Preparing the AI…".into()));
        let engine_dir = self.ctx.paths.engine_dir();
        std::fs::create_dir_all(&engine_dir)?;
        let archive = engine_dir.join(pin.file_name);
        let env_archive = std::env::var_os("REBOST_ENGINE_ARCHIVE").filter(|p| !p.is_empty());
        let bundled = self.ctx.paths.bundled_engine_archive();

        if let Some(local) = env_archive {
            let local = PathBuf::from(local);
            log::info!(
                "unpacking AI engine {ENGINE_BUILD} from {}",
                local.display()
            );
            if local != archive {
                std::fs::copy(&local, &archive).context("copy engine archive")?;
            }
            download::verify_sha256_blocking(&archive, pin.sha256)
                .context("engine archive SHA-256")?;
        } else if let Some(local) = bundled.filter(|path| path.is_file()) {
            log::info!(
                "unpacking bundled AI engine {ENGINE_BUILD} from {}",
                local.display()
            );
            if local != archive {
                std::fs::copy(local, &archive).context("copy engine archive")?;
            }
            // Notary unpacks this tar.gz, so signed Mac builds re-sign Mach-O
            // inside it and the bytes no longer match the GitHub pin SHA.
        } else {
            log::info!("downloading AI engine {ENGINE_BUILD} ({})", pin.file_name);
            download::download(
                &self.download_client,
                pin.url,
                &archive,
                &download::DownloadTicket {
                    kind: "engine",
                    id: "engine".into(),
                    name: "AI engine".into(),
                },
                Some(pin.sha256),
                None,
                &self.ctx.events.clone(),
                &download::DownloadControl::new(),
            )
            .await?;
        }

        let extract_dir = engine_dir.join("extract-tmp");
        std::fs::remove_dir_all(&extract_dir).ok();
        std::fs::create_dir_all(&extract_dir)?;
        unpack_engine_archive(&archive, &extract_dir)?;
        let found = super::process::find_llama_server(&extract_dir)
            .ok_or_else(|| anyhow!("engine archive did not contain llama-server"))?;
        let home = found
            .parent()
            .ok_or_else(|| anyhow!("engine binary has no parent directory"))?;
        let target = engine_dir.join(ENGINE_BUILD);
        std::fs::remove_dir_all(&target).ok();
        if home == extract_dir.as_path() {
            std::fs::rename(&extract_dir, &target)?;
        } else {
            std::fs::rename(home, &target)?;
            std::fs::remove_dir_all(&extract_dir).ok();
        }
        std::fs::remove_file(&archive).ok();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for entry in std::fs::read_dir(&target)?.flatten() {
                let path = entry.path();
                if path.is_file() {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).ok();
                }
            }
        }

        let binary = self.engine_binary();
        if !binary.exists() {
            return Err(anyhow!("engine archive did not contain llama-server"));
        }
        log::info!("AI engine {ENGINE_BUILD} is ready");
        Ok(binary)
    }
}

/// Tests win via `REBOST_ENGINE_ARCHIVE`. Release builds use the bundled
/// archive. GitHub is only the fallback when neither is present.
#[cfg(test)]
fn local_engine_archive(
    env_archive: Option<std::ffi::OsString>,
    bundled: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = env_archive {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    bundled.filter(|path| path.is_file()).map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_archive_wins_over_bundle() {
        let bundled = PathBuf::from("/bundle/engine.tar.gz");
        let found = local_engine_archive(
            Some(std::ffi::OsString::from("/tmp/test-engine.tar.gz")),
            Some(&bundled),
        )
        .unwrap();
        assert_eq!(found, PathBuf::from("/tmp/test-engine.tar.gz"));
    }

    #[test]
    fn bundle_is_used_when_env_unset() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("engine.tar.gz");
        std::fs::write(&archive, b"x").unwrap();
        assert_eq!(
            local_engine_archive(None, Some(&archive)).as_deref(),
            Some(archive.as_path())
        );
        assert!(local_engine_archive(None, None).is_none());
        assert!(local_engine_archive(Some(std::ffi::OsString::new()), Some(&archive)).is_some());
    }

    #[test]
    fn pin_file_name_is_stable() {
        let pin = current_engine_pin().unwrap();
        assert!(!pin.file_name.is_empty());
    }
}
