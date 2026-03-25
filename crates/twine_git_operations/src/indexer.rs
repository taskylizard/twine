use std::sync::Arc;

use eyre::{Context, Result};
use gix::refs::Category;
use hashbrown::HashMap;
use tracing::{debug, info};

use crate::{
    config::GitConfig,
    metadata::{IndexState, MetadataStore, RefsSnapshot, RepoKey, now_unix_secs},
    paths,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReindexOutcome {
    SkippedNotDue,
    SkippedUnchanged,
    Indexed,
}

pub struct MetadataIndexer {
    config: Arc<GitConfig>,
    metadata: Arc<MetadataStore>,
}

impl MetadataIndexer {
    pub fn new(config: Arc<GitConfig>, metadata: Arc<MetadataStore>) -> Self {
        Self { config, metadata }
    }

    pub async fn reindex_if_due(&self, repo: &RepoKey) -> Result<ReindexOutcome> {
        let now = now_unix_secs();
        if let Some(state) = self.metadata.get_index_state(repo)? {
            let elapsed = now.saturating_sub(state.last_indexed_at_unix_secs);
            if elapsed < self.config.metadata_reindex_interval.as_secs() {
                return Ok(ReindexOutcome::SkippedNotDue);
            }
        }

        self.reindex(repo).await
    }

    pub async fn reindex(&self, repo: &RepoKey) -> Result<ReindexOutcome> {
        let repo_path = paths::repo_path(&self.config.repo_scan_path, &repo.owner, &repo.repo)?;

        let gix_repo = gix::open(&repo_path)
            .with_context(|| format!("failed to open repo at {}", repo_path.display()))?;

        let mut branch_names = Vec::new();
        let mut tag_names = Vec::new();
        let mut head_oids = HashMap::new();

        for reference in gix_repo
            .references()
            .context("failed to iterate refs")?
            .all()
            .context("failed to list all refs")?
        {
            let Ok(mut reference) = reference else {
                continue;
            };

            let ref_name = reference.name().as_bstr().to_string();

            if let Ok(id) = reference.peel_to_id() {
                head_oids.insert(ref_name.clone(), id.to_string());
            }

            match reference.name().category() {
                Some(Category::LocalBranch) => {
                    branch_names.push(reference.name().shorten().to_string());
                }
                Some(Category::Tag) => {
                    tag_names.push(reference.name().shorten().to_string());
                }
                _ => {}
            }
        }

        let default_branch = gix_repo
            .head_name()
            .ok()
            .flatten()
            .map(|name| name.shorten().to_string())
            .or_else(|| branch_names.first().cloned())
            .unwrap_or_else(|| "main".to_owned());

        if let Some(previous) = self.metadata.get_index_state(repo)?
            && previous.head_oids == head_oids
        {
            debug!(owner = %repo.owner, repo = %repo.repo, "skipped metadata reindex because refs are unchanged");
            let refreshed_state = IndexState {
                last_indexed_at_unix_secs: now_unix_secs(),
                head_oids: previous.head_oids,
            };
            self.metadata.upsert_index_state(repo, refreshed_state)?;
            return Ok(ReindexOutcome::SkippedUnchanged);
        }

        let snapshot = RefsSnapshot {
            default_branch,
            branches: branch_names,
            tags: tag_names,
            updated_at_unix_secs: now_unix_secs(),
        };

        let state = IndexState {
            last_indexed_at_unix_secs: now_unix_secs(),
            head_oids,
        };

        self.metadata.write_index_bundle(repo, snapshot, state)?;
        info!(owner = %repo.owner, repo = %repo.repo, "metadata reindexed");

        Ok(ReindexOutcome::Indexed)
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, sync::Arc, time::Duration};

    use super::{MetadataIndexer, ReindexOutcome};
    use crate::{
        config::GitConfig,
        metadata::{IndexState, MetadataStore, RepoKey, now_unix_secs},
    };

    fn temp_path(test_name: &str) -> std::path::PathBuf {
        let base = env::temp_dir().join(format!("twine-{test_name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        base
    }

    #[tokio::test]
    async fn test_reindex_if_due_skips_when_interval_not_elapsed() {
        let db_path = temp_path("reindex-if-due");
        let metadata = Arc::new(MetadataStore::open(&db_path).expect("metadata db should open"));
        let key = RepoKey::new("alice", "demo");
        metadata
            .upsert_index_state(
                &key,
                IndexState {
                    last_indexed_at_unix_secs: now_unix_secs(),
                    head_oids: Default::default(),
                },
            )
            .expect("state should persist");

        let config = Arc::new(GitConfig {
            metadata_reindex_interval: Duration::from_secs(300),
            ..GitConfig::default()
        });
        let indexer = MetadataIndexer::new(config, metadata.clone());

        let outcome = indexer
            .reindex_if_due(&key)
            .await
            .expect("check should succeed");
        assert_eq!(outcome, ReindexOutcome::SkippedNotDue);

        drop(indexer);
        drop(metadata);
        let _ = fs::remove_dir_all(db_path);
    }
}
