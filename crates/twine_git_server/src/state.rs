use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result, eyre};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RepoRole {
    Owner,
    Member,
    Collaborator,
}

impl RepoRole {
    pub(crate) fn can_read(self) -> bool {
        true
    }

    pub(crate) fn can_write(self) -> bool {
        matches!(self, Self::Owner | Self::Member)
    }

    pub(crate) fn can_admin(self) -> bool {
        matches!(self, Self::Owner)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Repo {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    pub members: HashMap<String, RepoRole>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RepoCatalog {
    repos: Vec<Repo>,
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) repos: Arc<RwLock<HashMap<(String, String), Repo>>>,
    pub(crate) git: Arc<twine_git_operations::Git>,
    repo_catalog_path: Arc<PathBuf>,
}

impl AppState {
    pub fn from_git_config(config: twine_git_operations::GitConfig) -> Self {
        let repo_catalog_path = repo_catalog_path_from_config(&config);
        let repos = match load_repo_catalog(&repo_catalog_path) {
            Ok(repos) => repos,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    catalog_path = %repo_catalog_path.display(),
                    "failed to load repository catalog; starting with empty catalog"
                );
                HashMap::default()
            }
        };

        let git = match twine_git_operations::Git::new(config.clone()) {
            Ok(git) => git,
            Err(error) => {
                let mut fallback_config = config;
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_nanos())
                    .unwrap_or_default();
                fallback_config.metadata_db_path = env::temp_dir().join(format!(
                    "twine-git-operations-metadata-{}-{nonce}",
                    std::process::id()
                ));

                tracing::warn!(
                    ?error,
                    fallback_path = %fallback_config.metadata_db_path.display(),
                    "failed to initialize twine_git_operations with configured metadata path; using temporary metadata db path"
                );

                twine_git_operations::Git::new(fallback_config).expect(
                    "failed to initialize twine_git_operations service with fallback metadata path",
                )
            }
        };

        Self {
            repos: Arc::new(RwLock::new(repos)),
            git: Arc::new(git),
            repo_catalog_path: Arc::new(repo_catalog_path),
        }
    }

    pub(crate) async fn persist_repo_catalog(&self, repos: Vec<Repo>) -> Result<()> {
        let path = self.repo_catalog_path.as_ref();
        let Some(parent) = path.parent() else {
            return Err(eyre!("repo catalog path does not have a parent directory"));
        };
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "failed to create repo catalog parent directory {}",
                parent.display()
            )
        })?;

        let catalog = RepoCatalog { repos };
        let payload = serde_json::to_vec_pretty(&catalog).context("serialize repo catalog")?;
        let temp_path = path.with_extension("tmp");

        tokio::fs::write(&temp_path, payload)
            .await
            .with_context(|| {
                format!(
                    "failed to write repo catalog temp file {}",
                    temp_path.display()
                )
            })?;
        tokio::fs::rename(&temp_path, path).await.with_context(|| {
            format!(
                "failed to move repo catalog temp file {} to {}",
                temp_path.display(),
                path.display()
            )
        })?;

        Ok(())
    }
}

fn repo_catalog_path_from_config(config: &twine_git_operations::GitConfig) -> PathBuf {
    let candidate = if let Ok(path) = env::var("TWINE_REPO_CATALOG_PATH") {
        PathBuf::from(path)
    } else {
        config
            .metadata_db_path
            .parent()
            .map(|parent| parent.join("twine-repos.json"))
            .unwrap_or_else(|| env::temp_dir().join("twine-repos.json"))
    };

    match candidate.parent() {
        Some(parent) => {
            if let Err(error) = std::fs::create_dir_all(parent) {
                let fallback = env::temp_dir().join(format!(
                    "twine-repos-{}-{}.json",
                    std::process::id(),
                    now_nanos()
                ));
                tracing::warn!(
                    ?error,
                    candidate = %candidate.display(),
                    fallback = %fallback.display(),
                    "failed to prepare repo catalog directory; using temp catalog path"
                );
                fallback
            } else {
                candidate
            }
        }
        None => candidate,
    }
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn load_repo_catalog(path: &Path) -> Result<HashMap<(String, String), Repo>> {
    if !path.exists() {
        return Ok(HashMap::default());
    }

    let raw = std::fs::read(path)
        .with_context(|| format!("failed to read repo catalog from {}", path.display()))?;
    let catalog: RepoCatalog = serde_json::from_slice(&raw)
        .with_context(|| format!("failed to parse repo catalog from {}", path.display()))?;

    let mut repos = HashMap::with_capacity(catalog.repos.len());
    for repo in catalog.repos {
        let key = (repo.owner.clone(), repo.name.clone());
        repos.insert(key, repo);
    }

    Ok(repos)
}

impl Default for AppState {
    fn default() -> Self {
        Self::from_git_config(twine_git_operations::GitConfig::default())
    }
}
