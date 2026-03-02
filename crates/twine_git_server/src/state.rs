use std::{
    collections::HashMap,
    env,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct Repo {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    pub members: HashMap<String, RepoRole>,
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) repos: Arc<RwLock<HashMap<(String, String), Repo>>>,
    pub(crate) git: Arc<twine_git_operations::Git>,
}

impl Default for AppState {
    fn default() -> Self {
        let git = match twine_git_operations::Git::new(twine_git_operations::GitConfig::default()) {
            Ok(git) => git,
            Err(error) => {
                let mut config = twine_git_operations::GitConfig::default();
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_nanos())
                    .unwrap_or_default();
                config.metadata_db_path = env::temp_dir().join(format!(
                    "twine-git-operations-metadata-{}-{nonce}",
                    std::process::id()
                ));

                tracing::warn!(
                    ?error,
                    fallback_path = %config.metadata_db_path.display(),
                    "failed to initialize twine_git_operations with default metadata path; using temporary metadata db path"
                );

                twine_git_operations::Git::new(config)
                    .expect("failed to initialize twine_git_operations service with fallback metadata path")
            }
        };

        Self {
            repos: Arc::default(),
            git: Arc::new(git),
        }
    }
}
