use std::{env, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct GitConfig {
    pub repo_scan_path: PathBuf,
    pub metadata_db_path: PathBuf,
    pub metadata_reindex_interval: Duration,
    pub readme_names: Vec<String>,
    pub readme_cache_capacity: u64,
    pub diff_cache_capacity: u64,
    pub cache_ttl: Duration,
}

impl Default for GitConfig {
    fn default() -> Self {
        let repo_scan_path = env::var("TWINE_REPO_SCAN_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/git"));

        let metadata_db_path = env::var("TWINE_METADATA_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_scan_path.join(".twine-git-operations-metadata"));

        let metadata_reindex_interval = env::var("TWINE_METADATA_REINDEX_INTERVAL_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(300));

        Self {
            repo_scan_path,
            metadata_db_path,
            metadata_reindex_interval,
            readme_names: vec![
                "README.md",
                "readme.md",
                "README",
                "readme",
                "README.markdown",
                "readme.markdown",
                "README.txt",
                "readme.txt",
                "README.rst",
                "readme.rst",
                "README.org",
                "readme.org",
                "README.asciidoc",
                "readme.asciidoc",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            readme_cache_capacity: 128,
            diff_cache_capacity: 128,
            cache_ttl: Duration::from_secs(300),
        }
    }
}
