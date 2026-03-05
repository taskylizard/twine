use std::{path::PathBuf, time::Duration};

use confique::Config;

#[derive(Debug, Config)]
pub struct AppConfig {
    #[config(default = "0.0.0.0:3000", env = "TWINE_BIND_ADDR")]
    pub bind_addr: String,

    #[config(default = "info", env = "TWINE_LOG_FILTER")]
    pub log_filter: String,

    #[config(nested)]
    pub git: GitConfig,
}

#[derive(Debug, Config)]
pub struct GitConfig {
    #[config(default = "/home/git", env = "TWINE_REPO_SCAN_PATH")]
    pub repo_scan_path: PathBuf,

    #[config(
        default = "/home/git/.twine-git-operations-metadata",
        env = "TWINE_METADATA_DB_PATH"
    )]
    pub metadata_db_path: PathBuf,

    #[config(default = 300, env = "TWINE_METADATA_REINDEX_INTERVAL_SECS")]
    pub metadata_reindex_interval_secs: u64,

    #[config(default = 128, env = "TWINE_README_CACHE_CAPACITY")]
    pub readme_cache_capacity: u64,

    #[config(default = 128, env = "TWINE_DIFF_CACHE_CAPACITY")]
    pub diff_cache_capacity: u64,

    #[config(default = 300, env = "TWINE_CACHE_TTL_SECS")]
    pub cache_ttl_secs: u64,
}

impl From<GitConfig> for twine_git_operations::GitConfig {
    fn from(value: GitConfig) -> Self {
        Self {
            repo_scan_path: value.repo_scan_path,
            metadata_db_path: value.metadata_db_path,
            metadata_reindex_interval: Duration::from_secs(value.metadata_reindex_interval_secs),
            readme_names: twine_git_operations::GitConfig::default().readme_names,
            readme_cache_capacity: value.readme_cache_capacity,
            diff_cache_capacity: value.diff_cache_capacity,
            cache_ttl: Duration::from_secs(value.cache_ttl_secs),
        }
    }
}
