//! Fork-local metadata layered over provider catalogs, never written to their caches.

use crate::manager::ModelsManager;
use crate::manager::ModelsManagerFuture;
use crate::manager::RefreshStrategy;
use crate::manager::SharedModelsManager;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use serde::Deserialize;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::TryLockError;

const FILE_NAME: &str = "models.extend.json";
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MODELS: usize = 256;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Extensions {
    models: Vec<Map<String, Value>>,
}

impl Extensions {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let extensions: Self = serde_json::from_slice(bytes).map_err(|err| err.to_string())?;
        if extensions.models.len() > MAX_MODELS {
            return Err(format!("at most {MAX_MODELS} model entries are allowed"));
        }
        let mut slugs = HashSet::new();
        for entry in &extensions.models {
            let slug = entry
                .get("slug")
                .and_then(Value::as_str)
                .filter(|slug| !slug.trim().is_empty() && slug.trim() == *slug)
                .ok_or("each entry needs a non-empty slug without surrounding whitespace")?;
            if !slugs.insert(slug) {
                return Err(format!("duplicate model slug `{slug}`"));
            }
        }
        Ok(extensions)
    }

    fn apply(&self, mut models: Vec<ModelInfo>) -> Result<Vec<ModelInfo>, String> {
        for entry in &self.models {
            let slug = entry["slug"].as_str().ok_or("invalid slug")?;
            let index = models.iter().position(|model| model.slug == slug);
            let mut value = match index {
                Some(index) => {
                    serde_json::to_value(&models[index]).map_err(|err| err.to_string())?
                }
                None => Value::Object(Map::new()),
            };
            let object = value.as_object_mut().ok_or("invalid model object")?;
            object.extend(entry.clone());
            let model: ModelInfo = serde_json::from_value(value)
                .map_err(|err| format!("invalid metadata for `{slug}`: {err}"))?;
            if index.is_none()
                && (model.context_window.is_none() || !entry.contains_key("input_modalities"))
            {
                return Err(format!(
                    "new model `{slug}` must declare context_window and input_modalities"
                ));
            }
            if model.context_window.is_some_and(|size| size <= 0)
                || model.max_context_window.is_some_and(|size| size <= 0)
                || model.auto_compact_token_limit.is_some_and(|size| size <= 0)
                || !(1..=100).contains(&model.effective_context_window_percent)
                || matches!((model.context_window, model.max_context_window), (Some(window), Some(max)) if window > max)
            {
                return Err(format!("invalid context limits for `{slug}`"));
            }
            match index {
                Some(index) => models[index] = model,
                None => models.push(model),
            }
        }
        Ok(models)
    }
}

/// Load `$CODEX_HOME/models.extend.json` once for a local model manager.
/// Missing files preserve upstream behavior; invalid files warn and leave the catalog intact.
/// Metadata does not implement new wire protocols or bypass server/client compatibility checks.
pub fn with_local_model_extensions(
    inner: SharedModelsManager,
    codex_home: &Path,
) -> SharedModelsManager {
    let path = codex_home.join(FILE_NAME);
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return inner,
        Err(err) => {
            tracing::warn!(path = %path.display(), "cannot read model extensions: {err}");
            return inner;
        }
    };
    let mut bytes = Vec::new();
    let extensions = file
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())
        .and_then(|_| {
            if bytes.len() as u64 > MAX_BYTES {
                Err(format!("file exceeds {MAX_BYTES} bytes"))
            } else {
                Extensions::parse(&bytes)
            }
        });
    match extensions {
        Ok(extensions) => Arc::new(ExtendedModelsManager {
            inner,
            extensions,
            warned: AtomicBool::new(false),
        }),
        Err(err) => {
            tracing::warn!(path = %path.display(), "ignoring model extensions: {err}");
            inner
        }
    }
}

#[derive(Debug)]
struct ExtendedModelsManager {
    inner: SharedModelsManager,
    extensions: Extensions,
    warned: AtomicBool,
}

impl ExtendedModelsManager {
    fn extend(&self, models: Vec<ModelInfo>) -> Vec<ModelInfo> {
        match self.extensions.apply(models.clone()) {
            Ok(models) => models,
            Err(err) => {
                if !self.warned.swap(true, Ordering::Relaxed) {
                    tracing::warn!("ignoring {FILE_NAME}: {err}");
                }
                models
            }
        }
    }
}

impl ModelsManager for ExtendedModelsManager {
    fn raw_model_catalog(
        &self,
        strategy: RefreshStrategy,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        Box::pin(async move {
            let catalog = self.inner.raw_model_catalog(strategy, factory).await;
            ModelsResponse {
                models: self.extend(catalog.models),
            }
        })
    }

    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        Box::pin(async move { self.extend(self.inner.get_remote_models().await) })
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        Ok(self.extend(self.inner.try_get_remote_models()?))
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        self.inner.auth_manager()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.inner.list_collaboration_modes()
    }

    fn get_default_model<'a>(
        &'a self,
        model: &'a Option<String>,
        allow_provider_model_fallback: bool,
        strategy: RefreshStrategy,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'a, String> {
        Box::pin(async move {
            if let Some(requested) = model.as_ref()
                && self
                    .extensions
                    .models
                    .iter()
                    .any(|entry| entry["slug"].as_str() == Some(requested.as_str()))
                && self
                    .list_models(strategy, factory.clone())
                    .await
                    .iter()
                    .any(|entry| &entry.model == requested)
            {
                return requested.clone();
            }
            self.inner
                .get_default_model(model, allow_provider_model_fallback, strategy, factory)
                .await
        })
    }

    fn refresh_if_new_etag(
        &self,
        etag: String,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ()> {
        self.inner.refresh_if_new_etag(etag, factory)
    }
}

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
