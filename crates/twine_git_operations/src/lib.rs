pub mod cache;
pub mod config;
pub mod indexer;
pub mod metadata;
pub mod paths;
pub mod repo_reader;

use std::{path::Path, sync::Arc};

use eyre::Result;

pub use config::GitConfig;
pub use indexer::{MetadataIndexer, ReindexOutcome};
pub use metadata::{MetadataStore, RepoKey};
pub use repo_reader::RepoReader;

#[derive(Clone)]
pub struct Git {
    config: Arc<GitConfig>,
    metadata: Arc<MetadataStore>,
    indexer: Arc<MetadataIndexer>,
    reader: Arc<RepoReader>,
}

impl Git {
    pub fn new(config: GitConfig) -> Result<Self> {
        let config = Arc::new(config);
        let metadata = Arc::new(MetadataStore::open(&config.metadata_db_path)?);
        let cache = cache::RenderCache::new(
            config.readme_cache_capacity,
            config.diff_cache_capacity,
            config.cache_ttl,
        );
        let reader = Arc::new(RepoReader::new(config.clone(), cache));
        let indexer = Arc::new(MetadataIndexer::new(config.clone(), metadata.clone()));

        Ok(Self {
            config,
            metadata,
            indexer,
            reader,
        })
    }

    pub fn config(&self) -> &GitConfig {
        &self.config
    }

    pub fn metadata_store(&self) -> Arc<MetadataStore> {
        self.metadata.clone()
    }

    pub fn indexer(&self) -> Arc<MetadataIndexer> {
        self.indexer.clone()
    }

    pub fn reader(&self) -> Arc<RepoReader> {
        self.reader.clone()
    }

    pub fn repo_path(&self, owner: &str, repo: &str) -> Result<std::path::PathBuf> {
        paths::repo_path(Path::new(&self.config.repo_scan_path), owner, repo)
    }

    pub fn upsert_metadata_snapshot(
        &self,
        owner: &str,
        repo: &str,
        default_branch: &str,
        branches: Vec<String>,
        tags: Vec<String>,
    ) -> Result<()> {
        let key = RepoKey::new(owner, repo);
        self.metadata
            .upsert_refs_snapshot(&key, default_branch, branches, tags)
    }

    pub fn list_branches(&self, owner: &str, repo: &str) -> Result<Option<(Vec<String>, String)>> {
        let key = RepoKey::new(owner, repo);
        self.metadata.get_branches(&key)
    }

    pub fn list_tags(&self, owner: &str, repo: &str) -> Result<Option<Vec<String>>> {
        let key = RepoKey::new(owner, repo);
        self.metadata.get_tags(&key)
    }
}
