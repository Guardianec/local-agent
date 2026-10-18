use crate::{agent::AppCore, domain::*, registry};
use serde_json::json;
fn cleanup_candidate(
    settings: &Settings,
    model: &ModelInfo,
    receipt: Option<serde_json::Value>,
) -> Option<CleanupCandidate> {
    if !registry::obsolete(&model.name)
        || [
            &settings.model_profiles.fast,
            &settings.model_profiles.balanced,
            &settings.model_profiles.strong,
        ]
        .into_iter()
        .flatten()
        .any(|name| name == &model.name)
    {
        return None;
    }
    let owned = receipt.is_some_and(|v| v["origin"] == "app" && v["digest"] == model.digest);
    Some(CleanupCandidate {
        name: model.name.clone(),
        size: model.size,
        owned,
        reason: if owned {
            "Superseded app installation; exact digest matches its receipt."
        } else {
            "Origin is ambiguous. Preserved unless you explicitly choose removal."
        }
        .into(),
    })
}
impl AppCore {
    pub async fn install_model(&self, tier: &str) -> Result<()> {
        let spec = registry::spec(tier)
            .ok_or_else(|| AppError::new("MODEL_TIER", "Choose Fast, Balanced or Strong."))?;
        let cancel = self.reserve_service("model-install")?;
        let existed = self
            .provider
            .health(&self.store)
            .await
            .models
            .iter()
            .any(|m| m.name == spec.model_id);
        let result: Result<()> = async {
            self.provider
                .pull(&spec.model_id, &self.store, cancel)
                .await?;
            if !existed {
                let health = self.provider.health(&self.store).await;
                let model=health.models.iter().find(|m|m.name==spec.model_id&&m.local).ok_or_else(||AppError::new("MODEL_DOWNLOAD","Ollama finished the download, but local installation could not be confirmed. Refresh and retry."))?;
                self.store.put(
                        "model_installations",
                        &model.name,
                        &json!({"digest":model.digest,"origin":"app","at":now()}),
                    )?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = &result {
            let _ = self.store.put(
                "services",
                "model-install",
                &ModelInstallProgress {
                    model: spec.model_id,
                    status: if error.code == "CANCELLED" {
                        "Stopped"
                    } else {
                        "Needs attention"
                    }
                    .into(),
                    completed: 0.0,
                    total: None,
                    done: true,
                    error: Some(error.message.clone()),
                },
            );
        }
        self.finish_service();
        result
    }
    pub async fn cleanup_candidates(&self) -> Result<Vec<CleanupCandidate>> {
        let health = self.provider.health(&self.store).await;
        let settings = self.store.settings();
        Ok(health
            .models
            .iter()
            .filter_map(|model| {
                cleanup_candidate(
                    &settings,
                    model,
                    self.store.get("model_installations", &model.name).ok(),
                )
            })
            .collect())
    }

    pub async fn remove_obsolete_model(&self, name: &str) -> Result<()> {
        let _token = self.reserve_service("model-cleanup")?;
        let result = async {
            let candidates = self.cleanup_candidates().await?;
            if !candidates.iter().any(|m| m.name == name) {
                return Err(AppError::new(
                    "MODEL_CLEANUP",
                    "Only a superseded, unassigned model can be removed here.",
                ));
            }
            let health = self.provider.health(&self.store).await;
            if health.models.iter().any(|m| m.name == name && m.loaded) {
                return Err(AppError::new(
                    "MODEL_IN_USE",
                    "This model is loaded. Stop its current use before removing it.",
                ));
            }
            self.provider.delete_model(name).await
        }
        .await;
        self.finish_service();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_unrelated_assigned_and_ambiguous_models() {
        let registry: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/model-registry.json")).unwrap();
        let old = registry["legacy"][0]["modelId"].as_str().unwrap();
        let mut model = ModelInfo {
            name: old.into(),
            digest: "digest".into(),
            size: 1.0,
            parameter_size: String::new(),
            quantization: String::new(),
            loaded: false,
            vram: None,
            loaded_bytes: None,
            capability: String::new(),
            local: true,
            vision: false,
            thinking: false,
        };
        let mut s = Settings::default();
        assert!(!cleanup_candidate(&s, &model, None).unwrap().owned);
        assert!(
            cleanup_candidate(&s, &model, Some(json!({"origin":"app","digest":"digest"})))
                .unwrap()
                .owned
        );
        assert!(
            !cleanup_candidate(
                &s,
                &model,
                Some(json!({"origin":"app","digest":"other-build"}))
            )
            .unwrap()
            .owned
        );
        s.model_profiles.fast = Some(old.into());
        assert!(cleanup_candidate(&s, &model, None).is_none());
        s.model_profiles.fast = None;
        model.name = "unrelated:user-model".into();
        assert!(
            cleanup_candidate(&s, &model, Some(json!({"origin":"app","digest":"digest"})))
                .is_none()
        );
    }
}
