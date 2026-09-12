use super::*;
use crate::ModelsManagerConfig;
use crate::manager::StaticModelsManager;
use codex_http_client::DEFAULT_HTTP_CLIENT_FACTORY;
use pretty_assertions::assert_eq;
use serde_json::json;

fn model(slug: &str) -> ModelInfo {
    let mut model = crate::bundled_models_response()
        .expect("bundled catalog")
        .models
        .remove(0);
    model.slug = slug.to_string();
    model.supported_in_api = true;
    model.context_window = Some(100_000);
    model.max_context_window = Some(200_000);
    model
}

#[test]
fn overlays_only_explicit_fields_and_appends_new_models() {
    let original = model("original");
    let untouched = model("untouched");
    let added = model("custom-model");
    let extensions = Extensions::parse(&serde_json::to_vec(&json!({"models": [
        {"slug": "original", "context_window": 120_000, "supported_reasoning_levels": []}, added,
    ]})).expect("serialize extensions")).expect("parse extensions");
    let mut expected = original.clone();
    expected.context_window = Some(120_000);
    expected.supported_reasoning_levels.clear();
    assert_eq!(
        extensions
            .apply(vec![original, untouched.clone()])
            .expect("merge"),
        vec![expected, untouched, added]
    );
}

#[test]
fn invalid_entries_are_rejected() {
    for value in [
        json!({"models": [{"slug": "duplicate"}, {"slug": "duplicate"}]}),
        json!({"models": [{"slug": " "}]}),
        json!({"models": [{}]}),
        json!({"models": [], "unsupported": true}),
        json!({"models": vec![json!({"slug": "x"}); MAX_MODELS + 1]}),
    ] {
        assert!(Extensions::parse(&serde_json::to_vec(&value).expect("serialize")).is_err());
    }
    for value in [
        json!({"models": [{"slug": "new-model", "context_window": 100_000}]}),
        json!({"models": [{"slug": "original", "context_window": -1}]}),
        json!({"models": [{"slug": "original", "context_window": 300_000}]}),
        json!({"models": [{"slug": "original", "supported_reasoning_levels": "invalid"}]}),
    ] {
        let extensions =
            Extensions::parse(&serde_json::to_vec(&value).expect("serialize")).expect("parse");
        assert!(extensions.apply(vec![model("original")]).is_err());
    }
}

#[tokio::test]
async fn file_overlay_is_shared_by_listing_lookup_and_explicit_selection() {
    let home = tempfile::tempdir().expect("temp home");
    let original = model("original");
    let added = model("custom-model");
    std::fs::write(
        home.path().join(FILE_NAME),
        serde_json::to_vec(&json!({"models": [
            {"slug": "original", "context_window": 120_000}, added,
        ]}))
        .expect("serialize"),
    )
    .expect("write extensions");
    let inner: SharedModelsManager = Arc::new(StaticModelsManager::new(
        /*auth_manager*/ None,
        ModelsResponse {
            models: vec![original.clone()],
        },
    ));
    let manager = with_local_model_extensions(inner.clone(), home.path());
    let catalog = manager
        .raw_model_catalog(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
        .await;
    let mut expected = original.clone();
    expected.context_window = Some(120_000);
    assert_eq!(catalog.models, vec![expected, added]);
    assert_eq!(
        manager.try_get_remote_models().expect("snapshot"),
        catalog.models
    );
    assert_eq!(manager.get_remote_models().await, catalog.models);
    let info = manager
        .get_model_info("custom-model", &ModelsManagerConfig::default())
        .await;
    assert_eq!(info.context_window, Some(100_000));
    assert!(!info.used_fallback_model_metadata);
    let listed = manager
        .list_models(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
        .await;
    assert!(listed.iter().any(|entry| entry.model == "custom-model"));
    assert_eq!(
        manager
            .get_default_model(
                &Some("custom-model".to_string()),
                /*allow_provider_model_fallback*/ true,
                RefreshStrategy::Offline,
                DEFAULT_HTTP_CLIENT_FACTORY
            )
            .await,
        "custom-model"
    );
    assert_eq!(inner.get_remote_models().await, vec![original]);
}

#[tokio::test]
async fn missing_invalid_and_oversized_files_leave_catalog_unchanged() {
    let home = tempfile::tempdir().expect("temp home");
    let original = model("original");
    let inner: SharedModelsManager = Arc::new(StaticModelsManager::new(
        /*auth_manager*/ None,
        ModelsResponse {
            models: vec![original.clone()],
        },
    ));
    assert!(Arc::ptr_eq(
        &inner,
        &with_local_model_extensions(inner.clone(), home.path())
    ));
    for bytes in [
        b"invalid JSON".to_vec(),
        vec![b' '; (MAX_BYTES + 1) as usize],
        serde_json::to_vec(&json!({"models": [
            {"slug": "original", "context_window": 120_000}, {"slug": "incomplete-new-model"}
        ]}))
        .expect("serialize"),
    ] {
        std::fs::write(home.path().join(FILE_NAME), bytes).expect("write extensions");
        let manager = with_local_model_extensions(inner.clone(), home.path());
        assert_eq!(manager.get_remote_models().await, vec![original.clone()]);
    }
}

#[derive(Debug)]
struct RefreshingCatalog {
    before: Vec<ModelInfo>,
    after: Vec<ModelInfo>,
    refreshed: AtomicBool,
}

impl ModelsManager for RefreshingCatalog {
    fn raw_model_catalog(
        &self,
        strategy: RefreshStrategy,
        _factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        Box::pin(async move {
            if matches!(strategy, RefreshStrategy::Online) {
                self.refreshed.store(true, Ordering::Relaxed);
            }
            ModelsResponse {
                models: self.get_remote_models().await,
            }
        })
    }
    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        Box::pin(async { self.try_get_remote_models().expect("test snapshot") })
    }
    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        Ok(if self.refreshed.load(Ordering::Relaxed) {
            self.after.clone()
        } else {
            self.before.clone()
        })
    }
    fn auth_manager(&self) -> Option<&AuthManager> {
        None
    }
    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        Vec::new()
    }
    fn refresh_if_new_etag(
        &self,
        _etag: String,
        _factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ()> {
        Box::pin(async {
            self.refreshed.store(true, Ordering::Relaxed);
        })
    }
}

#[tokio::test]
async fn refreshed_catalog_keeps_extensions_and_new_upstream_fields() {
    let before = model("original");
    let mut after = before.clone();
    after.description = Some("new upstream description".to_string());
    let inner = Arc::new(RefreshingCatalog {
        before: vec![before],
        after: vec![after.clone()],
        refreshed: AtomicBool::new(false),
    });
    let manager = ExtendedModelsManager {
        inner: inner.clone(),
        extensions: Extensions::parse(
            br#"{"models":[{"slug":"original","context_window":120000}]}"#,
        )
        .expect("parse"),
        warned: AtomicBool::new(false),
    };
    let mut expected = after.clone();
    expected.context_window = Some(120_000);
    assert_eq!(
        manager
            .raw_model_catalog(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
            .await
            .models,
        vec![expected.clone()]
    );
    assert_eq!(inner.get_remote_models().await, vec![after]);
    inner.refreshed.store(false, Ordering::Relaxed);
    manager
        .refresh_if_new_etag("new".to_string(), DEFAULT_HTTP_CLIENT_FACTORY)
        .await;
    assert_eq!(
        manager.try_get_remote_models().expect("snapshot"),
        vec![expected]
    );
}
