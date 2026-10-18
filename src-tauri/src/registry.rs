//! Physical identities live here; routing and tools use logical tier names.
use crate::domain::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/")]
pub struct ModelSpec {
    pub tier: String,
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
    pub family: String,
    pub architecture: String,
    pub quantization: String,
    pub vision: bool,
    pub tools: bool,
    pub thinking: bool,
    pub context: u32,
    pub download_bytes: f64,
    pub offload_expected: bool,
    pub purpose: String,
}
#[derive(Deserialize)]
struct Registry {
    version: u32,
    tiers: Vec<ModelSpec>,
    legacy: Vec<Legacy>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Legacy {
    tier: String,
    model_id: String,
}
fn registry() -> Registry {
    serde_json::from_str(include_str!("../../assets/model-registry.json"))
        .expect("built-in model registry")
}
pub fn specs() -> Vec<ModelSpec> {
    registry().tiers
}
pub fn spec(tier: &str) -> Option<ModelSpec> {
    specs().into_iter().find(|s| s.tier == tier)
}
pub fn identity(name: &str) -> Option<ModelSpec> {
    specs().into_iter().find(|s| s.model_id == name)
}
pub fn context(settings: &Settings, tier: &str) -> u32 {
    match tier {
        "fast" => settings.intelligence.fast_context,
        "strong" => settings.intelligence.strong_context,
        _ => settings.intelligence.balanced_context,
    }
    .clamp(4096, 32768)
}
pub fn migrate(settings: &mut Settings) {
    let registry = registry();
    if settings.intelligence.registry_version >= registry.version {
        return;
    }
    // Context defaults changed; even a preserved custom Strong assignment needs
    // fresh evidence before re-enabling it.
    settings.model_profiles.strong_enabled = false;
    for entry in registry.tiers {
        let slot = match entry.tier.as_str() {
            "fast" => &mut settings.model_profiles.fast,
            "strong" => &mut settings.model_profiles.strong,
            _ => &mut settings.model_profiles.balanced,
        };
        let superseded = slot.as_ref().is_none_or(|name| {
            registry
                .legacy
                .iter()
                .any(|old| old.tier == entry.tier && old.model_id == *name)
        });
        if superseded {
            *slot = Some(entry.model_id);
            if entry.tier == "strong" {
                settings.model_profiles.strong_enabled = false;
            }
        }
    }
    settings.model = settings.model_profiles.balanced.clone();
    settings.intelligence.registry_version = registry.version;
}
pub fn context_for_model(settings: &Settings, name: &str) -> u32 {
    if settings.model_profiles.strong.as_deref() == Some(name) {
        context(settings, "strong")
    } else if settings.model_profiles.balanced.as_deref() == Some(name) {
        context(settings, "balanced")
    } else if settings.model_profiles.fast.as_deref() == Some(name) {
        context(settings, "fast")
    } else {
        identity(name)
            .map(|s| context(settings, &s.tier))
            .unwrap_or(settings.context_size)
    }
}
pub fn obsolete(name: &str) -> bool {
    registry().legacy.iter().any(|m| m.model_id == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_migrates_stale_but_preserves_custom() {
        let mut s = Settings::default();
        s.model_profiles.balanced = Some("my-custom:latest".into());
        migrate(&mut s);
        assert_eq!(s.model_profiles.fast, Some(spec("fast").unwrap().model_id));
        assert_eq!(
            s.model_profiles.balanced.as_deref(),
            Some("my-custom:latest")
        );
        let before = serde_json::to_value(&s).unwrap();
        migrate(&mut s);
        assert_eq!(before, serde_json::to_value(s).unwrap());
        assert_eq!(specs().len(), 3);
        assert_eq!(spec("fast").unwrap().quantization, "Q8_0");
    }
}
