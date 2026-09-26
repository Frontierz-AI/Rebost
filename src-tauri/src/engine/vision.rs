//! Conservative image limits based on the complete installed AI and runtime.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{catalog::MachineProfile, Engine};

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionLimits {
    pub max_images: usize,
    pub max_edge: u32,
    pub tokens_per_image: u32,
}

impl VisionLimits {
    pub fn fit_context(mut self, context: u32, answer: u32) -> Option<Self> {
        let room = context.saturating_sub(answer + 1280);
        self.max_images = self
            .max_images
            .min((room / (self.tokens_per_image + 256)) as usize);
        (self.max_images > 0).then_some(self)
    }
}

/// Leave room for the OS, normal chat, and image encoder working buffers.
pub fn limits_for(profile: &MachineProfile, weights: u64, projector: u64) -> Option<VisionLimits> {
    if weights == 0 || projector == 0 || profile.total_ram_bytes < 8 * GIB {
        return None;
    }
    let runtime = profile.runtime_need_bytes(weights.saturating_add(projector));
    if runtime.saturating_add(GIB / 2) > profile.model_budget_bytes() {
        return None;
    }
    // CPU and unmeasured discrete GPUs stay modest even when system RAM is large.
    let accelerated = matches!(profile.accelerator.as_str(), "Metal" | "CUDA" | "Vulkan");
    let (max_images, max_edge, tokens_per_image) = if accelerated
        && profile.accelerator == "Metal"
        && profile.total_ram_bytes >= 32 * GIB
        && profile.model_budget_bytes().saturating_sub(runtime) >= 4 * GIB
    {
        (4, 1536, 1536)
    } else if accelerated && profile.total_ram_bytes >= 16 * GIB {
        (2, 1024, 1024)
    } else {
        (1, 768, 1024)
    };
    Some(VisionLimits {
        max_images,
        max_edge,
        tokens_per_image,
    })
}

impl Engine {
    pub fn vision_limits(&self) -> Option<VisionLimits> {
        *crate::core::mutex_lock(&self.vision)
    }

    /// Why image support last failed to start, if it did.
    pub fn vision_error(&self) -> Option<String> {
        crate::core::mutex_lock(&self.vision_error).clone()
    }

    pub(super) async fn verify_vision(&self, base: &str, candidate: Option<VisionLimits>) {
        let Some(limits) = candidate else {
            *crate::core::mutex_lock(&self.vision) = None;
            return;
        };
        let problem = match self
            .client
            .get(format!("{base}/props"))
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                match response.json::<serde_json::Value>().await {
                    Ok(props) if props["modalities"]["vision"] == true => None,
                    Ok(_) => Some("the AI started but did not report image support".to_string()),
                    Err(error) => Some(format!("unreadable /props reply: {error}")),
                }
            }
            Ok(response) => Some(format!("/props answered {}", response.status())),
            Err(error) => Some(format!("/props failed: {error}")),
        };
        if let Some(problem) = &problem {
            log::error!("image support not confirmed: {problem}");
        }
        *crate::core::mutex_lock(&self.vision_error) = problem.clone();
        *crate::core::mutex_lock(&self.vision) = problem.is_none().then_some(limits);
    }
}

/// Stops Settings from offering the same image download again after it
/// failed to start with this AI on this engine release.
pub fn failed_vision_key(model_file: &str) -> String {
    format!("{}/{model_file}", super::ENGINE_RELEASE)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn machine(ram: u64, accelerator: &str) -> MachineProfile {
        MachineProfile {
            total_ram_bytes: ram * GIB,
            accelerator: accelerator.into(),
            cpu: String::new(),
            free_disk_bytes: 0,
            process_arch: String::new(),
            os_arch: String::new(),
        }
    }

    #[test]
    fn context_capacity_can_reduce_or_disable_hardware_limits() {
        let limits = VisionLimits {
            max_images: 4,
            max_edge: 1536,
            tokens_per_image: 1536,
        };
        assert!(limits.fit_context(2048, 1024).is_none());
        assert_eq!(limits.fit_context(4096, 1024).unwrap().max_images, 1);
        assert_eq!(limits.fit_context(8192, 1024).unwrap().max_images, 3);
    }
    #[test]
    fn limits_follow_memory_and_compute_instead_of_model_name() {
        assert!(limits_for(&machine(4, "Metal"), GIB, GIB / 4).is_none());
        assert!(limits_for(&machine(8, "Metal"), 4 * GIB, GIB).is_none());
        assert_eq!(
            limits_for(&machine(8, "Metal"), GIB, GIB / 4)
                .unwrap()
                .max_images,
            1
        );
        assert_eq!(
            limits_for(&machine(16, "Metal"), 3 * GIB, GIB / 2)
                .unwrap()
                .max_images,
            2
        );
        assert_eq!(
            limits_for(&machine(32, "Metal"), 4 * GIB, GIB)
                .unwrap()
                .max_images,
            4
        );
        assert_eq!(
            limits_for(&machine(64, "CPU"), 4 * GIB, GIB)
                .unwrap()
                .max_images,
            1
        );
        assert_eq!(
            limits_for(&machine(64, "Vulkan"), 4 * GIB, GIB)
                .unwrap()
                .max_images,
            2
        );
        assert!(limits_for(&machine(64, "Metal"), 40 * GIB, 4 * GIB).is_none());
    }
}
