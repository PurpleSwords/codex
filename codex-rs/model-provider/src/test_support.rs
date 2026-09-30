//! Test-only provider construction with stable model metadata.
//! Production callers must use the normal provider factory.

use std::path::PathBuf;
use std::sync::Arc;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::test_support::test_models_response;

use crate::auth::auth_manager_for_provider;
use crate::create_model_provider;
use crate::models_endpoint::OpenAiModelsEndpoint;

/// Build a remote-capable manager whose offline fallback is a fixed test fixture.
pub fn models_manager_with_provider(
    codex_home: PathBuf,
    auth_manager: Arc<AuthManager>,
    provider: ModelProviderInfo,
) -> SharedModelsManager {
    if provider.is_amazon_bedrock() {
        return create_model_provider(provider, Some(auth_manager))
            .models_manager(codex_home, /*config_model_catalog*/ None);
    }
    let auth_manager = auth_manager_for_provider(Some(auth_manager), &provider);
    let endpoint = Arc::new(OpenAiModelsEndpoint::new(provider, auth_manager.clone()));
    let manager = OpenAiModelsManager::new(codex_home.clone(), endpoint, auth_manager)
        .with_fallback_catalog(
            test_models_response()
                .unwrap_or_else(|err| panic!("test models.json should parse: {err}")),
        );
    codex_models_manager::with_local_model_extensions(Arc::new(manager), &codex_home)
}
