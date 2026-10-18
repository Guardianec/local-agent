use crate::domain::*;

pub const GPU_RESERVE: f64 = 512.0 * 1024.0 * 1024.0;
fn requires_strong(model: &ModelInfo) -> bool {
    model.size >= 15.0 * 1024.0_f64.powi(3)
        || crate::registry::identity(&model.name).is_some_and(|s| s.offload_expected)
}
pub fn fully_gpu(model: &ModelInfo) -> Option<bool> {
    match (model.vram, model.loaded_bytes) {
        (Some(gpu), Some(total)) if total > 0.0 => Some(gpu + 1048576.0 >= total),
        _ => None,
    }
}

pub struct Choice {
    pub name: String,
    pub profile: String,
    pub reason: String,
    pub strong: bool,
}

pub fn calibration<'a>(
    health: &'a RuntimeInfo,
    model: &ModelInfo,
    context: u32,
) -> Option<&'a ModelCalibration> {
    health.calibrations.iter().find(|c| {
        c.digest == model.digest
            && Some(c.runtime.as_str()) == health.version.as_deref()
            && c.context_size == context
    })
}

pub fn strong_ready(health: &RuntimeInfo, model: &ModelInfo, context: u32) -> bool {
    calibration(health, model, context).is_some_and(|c| c.passed)
}

pub fn choose(
    settings: &Settings,
    health: &RuntimeInfo,
    level: u8,
    override_name: Option<&str>,
    calibration_run: bool,
) -> Result<Choice> {
    if level == 0 || level == 4 {
        return Err(AppError::new(
            "MODEL_ROUTE",
            "This task does not use a local model.",
        ));
    }
    let profiles = &settings.model_profiles;
    let installed = |name: &str| {
        health
            .models
            .iter()
            .find(|m| m.name == name && m.local && m.capability == "verified")
    };
    let allowed = |model: &ModelInfo, strong: bool| {
        calibration_run
            || !strong
            || (profiles.strong_enabled
                && strong_ready(health, model, crate::registry::context(settings, "strong")))
    };
    if let Some(name) = override_name {
        let forced_profile = name.strip_prefix("@profile/");
        let name = match forced_profile {
            Some("fast") => profiles.fast.as_deref(),
            Some("balanced") => profiles.balanced.as_deref(),
            Some("strong") => profiles.strong.as_deref(),
            Some(_) => None,
            None => Some(name),
        }
        .ok_or_else(|| {
            AppError::new(
                "MODEL_PROFILE_REQUIRED",
                "Assign an installed model to the selected profile first.",
            )
        })?;
        let model = installed(name).ok_or_else(|| AppError::new("MODEL_UNAVAILABLE", "The overridden model is unavailable or its tool compatibility needs checking. Choose Auto or review Local models."))?;
        let strong = forced_profile == Some("strong")
            || (forced_profile.is_none() && profiles.strong.as_deref() == Some(name))
            || requires_strong(model);
        if !allowed(model, strong) {
            return Err(AppError::new("MODEL_CALIBRATION_REQUIRED", "Enable Strong after a successful coding benchmark at its selected context size, or choose Auto."));
        }
        let profile = if strong {
            "strong"
        } else if let Some(profile) = forced_profile {
            profile
        } else if profiles.fast.as_deref() == Some(name) {
            "fast"
        } else {
            "balanced"
        };
        return Ok(Choice { name: name.into(), profile: profile.into(), reason: "Using your task-specific model override; task scope and safety limits are unchanged.".into(), strong });
    }
    let candidates = match level {
        1 => vec![("fast", &profiles.fast), ("balanced", &profiles.balanced)],
        3 if profiles.strong_enabled => vec![
            ("strong", &profiles.strong),
            ("balanced", &profiles.balanced),
            ("fast", &profiles.fast),
        ],
        _ => vec![("balanced", &profiles.balanced), ("fast", &profiles.fast)],
    };
    let mut skipped = Vec::new();
    for (profile, name) in candidates {
        let Some(name) = name else {
            skipped.push(format!("{profile} is unassigned"));
            continue;
        };
        let Some(model) = installed(name) else {
            skipped.push(format!("{profile} is missing or unverified"));
            continue;
        };
        let strong = profile == "strong" || requires_strong(model);
        if !allowed(model, strong) {
            skipped.push(format!(
                "{profile} lacks a passing calibration with GPU headroom"
            ));
            continue;
        }
        let reason = format!(
            "Level {level} → {profile}. {}",
            if skipped.is_empty() {
                "Selected from your configured profiles.".into()
            } else {
                format!("Fallback: {}.", skipped.join("; "))
            }
        );
        return Ok(Choice {
            name: name.clone(),
            profile: if strong { "strong" } else { profile }.into(),
            reason,
            strong,
        });
    }
    Err(AppError::new("MODEL_REQUIRED", "No verified model is available for this route. Assign and check a Fast or Balanced model in Local models.").detail(skipped.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Settings, RuntimeInfo) {
        let settings = Settings {
            context_size: 8192,
            intelligence: IntelligenceSettings {
                strong_context: 8192,
                ..Default::default()
            },
            model_profiles: ModelProfiles {
                fast: Some("small".into()),
                balanced: Some("medium".into()),
                strong: Some("large".into()),
                strong_enabled: true,
            },
            ..Default::default()
        };
        let models = ["small", "medium", "large"]
            .into_iter()
            .map(|name| ModelInfo {
                name: name.into(),
                digest: name.into(),
                size: 1.0,
                parameter_size: "Unknown".into(),
                quantization: "Q4".into(),
                loaded: false,
                vram: None,
                loaded_bytes: None,
                capability: "verified".into(),
                local: true,
                vision: false,
                thinking: false,
            })
            .collect();
        let health = RuntimeInfo {
            status: "ready".into(),
            version: Some("1".into()),
            installed: true,
            models,
            message: String::new(),
            gpu: None,
            calibrations: vec![],
            memory: None,
        };
        (settings, health)
    }
    #[test]
    fn routes_and_fallbacks_do_not_bypass_hard_gates() {
        let (s, mut h) = fixture();
        assert_eq!(choose(&s, &h, 1, None, false).unwrap().name, "small");
        assert_eq!(choose(&s, &h, 2, None, false).unwrap().name, "medium");
        assert_eq!(choose(&s, &h, 3, None, false).unwrap().name, "medium");
        assert!(choose(&s, &h, 4, Some("small"), false).is_err());
        assert!(choose(&s, &h, 0, Some("small"), false).is_err());
        h.models[0].capability = "unknown".into();
        assert!(choose(&s, &h, 1, None, false)
            .unwrap()
            .reason
            .contains("Fallback"));
        assert!(choose(&s, &h, 1, Some("small"), false).is_err());
        assert_eq!(
            choose(&s, &h, 1, Some("@profile/balanced"), false)
                .unwrap()
                .name,
            "medium"
        );
        assert!(choose(&s, &h, 1, Some("@profile/strong"), false).is_err());
    }
    #[test]
    fn strong_requires_current_calibration_and_accepts_offload() {
        let (mut s, mut h) = fixture();
        h.calibrations.push(ModelCalibration {
            model: "large".into(),
            digest: "large".into(),
            runtime: "1".into(),
            context_size: 8192,
            passed: true,
            full_gpu: Some(true),
            elapsed_ms: 1.0,
            tokens_per_second: Some(1.0),
            allocation_bytes: None,
            free_gpu_bytes: Some(GPU_RESERVE),
            message: String::new(),
            evidence: String::new(),
            tested_at: now(),
            peak_ram_bytes: None,
            minimum_free_ram_bytes: None,
            time_to_first_token_ms: None,
            retries: 0,
            escalations: 0,
        });
        assert_eq!(choose(&s, &h, 3, None, false).unwrap().name, "large");
        s.intelligence.strong_context = 16384;
        assert_eq!(choose(&s, &h, 3, None, false).unwrap().name, "medium");
        assert!(choose(&s, &h, 3, Some("large"), false).is_err());
        s.intelligence.strong_context = 8192;
        h.calibrations[0].full_gpu = Some(false);
        assert_eq!(choose(&s, &h, 3, None, false).unwrap().name, "large");
    }
    #[test]
    fn gpu_placement_does_not_label_partial_offload_as_full() {
        let (_, mut health) = fixture();
        let model = &mut health.models[0];
        assert_eq!(fully_gpu(model), None);
        model.loaded_bytes = Some(6e9);
        model.vram = Some(5.9e9);
        assert_eq!(fully_gpu(model), Some(false));
        model.vram = Some(6e9);
        assert_eq!(fully_gpu(model), Some(true));
    }
}
