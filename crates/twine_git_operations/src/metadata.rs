use std::time::{SystemTime, UNIX_EPOCH};

use eyre::{Context, Result};
use hashbrown::HashMap;
use itertools::Itertools;
use rocksdb::{DB, Options, WriteBatch};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoKey {
    pub owner: String,
    pub repo: String,
}

impl RepoKey {
    pub fn new(owner: &str, repo: &str) -> Self {
        Self {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        }
    }

    pub fn as_prefix(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefsSnapshot {
    pub default_branch: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    pub updated_at_unix_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexState {
    pub last_indexed_at_unix_secs: u64,
    pub head_oids: HashMap<String, String>,
}

pub struct MetadataStore {
    db: DB,
}

impl MetadataStore {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);

        let db = DB::open(&options, path)
            .with_context(|| format!("failed to open rocksdb at {}", path.display()))?;

        Ok(Self { db })
    }

    pub fn upsert_refs_snapshot(
        &self,
        repo: &RepoKey,
        default_branch: &str,
        branches: Vec<String>,
        tags: Vec<String>,
    ) -> Result<()> {
        let snapshot = RefsSnapshot {
            default_branch: default_branch.to_owned(),
            branches: branches.into_iter().sorted().collect(),
            tags: tags.into_iter().sorted().collect(),
            updated_at_unix_secs: now_unix_secs(),
        };

        let key = refs_snapshot_key(repo);
        let value = serde_json::to_vec(&snapshot).context("failed to serialize refs snapshot")?;
        self.db
            .put(key.as_bytes(), value)
            .context("failed to write refs snapshot")?;
        Ok(())
    }

    pub fn get_branches(&self, repo: &RepoKey) -> Result<Option<(Vec<String>, String)>> {
        let Some(snapshot) = self.get_refs_snapshot(repo)? else {
            return Ok(None);
        };

        Ok(Some((snapshot.branches, snapshot.default_branch)))
    }

    pub fn get_tags(&self, repo: &RepoKey) -> Result<Option<Vec<String>>> {
        let Some(snapshot) = self.get_refs_snapshot(repo)? else {
            return Ok(None);
        };

        Ok(Some(snapshot.tags))
    }

    pub fn get_index_state(&self, repo: &RepoKey) -> Result<Option<IndexState>> {
        let key = index_state_key(repo);
        let Some(raw) = self
            .db
            .get(key.as_bytes())
            .context("failed to read index state")?
        else {
            return Ok(None);
        };

        let state = serde_json::from_slice(&raw).context("failed to deserialize index state")?;
        Ok(Some(state))
    }

    pub fn upsert_index_state(&self, repo: &RepoKey, state: IndexState) -> Result<()> {
        let key = index_state_key(repo);
        let value = serde_json::to_vec(&state).context("failed to serialize index state")?;
        self.db
            .put(key.as_bytes(), value)
            .context("failed to write index state")?;
        Ok(())
    }

    pub fn write_index_bundle(
        &self,
        repo: &RepoKey,
        snapshot: RefsSnapshot,
        state: IndexState,
    ) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.put(
            refs_snapshot_key(repo).as_bytes(),
            serde_json::to_vec(&snapshot).context("serialize refs snapshot")?,
        );
        batch.put(
            index_state_key(repo).as_bytes(),
            serde_json::to_vec(&state).context("serialize index state")?,
        );
        self.db
            .write(batch)
            .context("failed to persist index bundle")?;
        Ok(())
    }

    fn get_refs_snapshot(&self, repo: &RepoKey) -> Result<Option<RefsSnapshot>> {
        let key = refs_snapshot_key(repo);
        let Some(raw) = self
            .db
            .get(key.as_bytes())
            .context("failed to read refs snapshot")?
        else {
            return Ok(None);
        };

        let snapshot =
            serde_json::from_slice(&raw).context("failed to deserialize refs snapshot")?;
        Ok(Some(snapshot))
    }
}

fn refs_snapshot_key(repo: &RepoKey) -> String {
    format!("repo/{}/refs", repo.as_prefix())
}

fn index_state_key(repo: &RepoKey) -> String {
    format!("repo/{}/index-state", repo.as_prefix())
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf};

    use super::{MetadataStore, RepoKey};

    fn test_db_path(test_name: &str) -> PathBuf {
        let base = env::temp_dir().join(format!("twine-{test_name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        base
    }

    #[test]
    fn test_refs_snapshot_round_trip() {
        let db_path = test_db_path("metadata-round-trip");
        let store = MetadataStore::open(&db_path).expect("db should open");
        let key = RepoKey::new("alice", "demo");

        store
            .upsert_refs_snapshot(
                &key,
                "main",
                vec!["main".to_owned(), "dev".to_owned()],
                vec!["v1.0.0".to_owned()],
            )
            .expect("snapshot should persist");

        let branches = store
            .get_branches(&key)
            .expect("branches should load")
            .expect("snapshot should exist");
        let tags = store
            .get_tags(&key)
            .expect("tags should load")
            .expect("snapshot should exist");

        assert_eq!(branches.1, "main");
        assert_eq!(branches.0, vec!["dev".to_owned(), "main".to_owned()]);
        assert_eq!(tags, vec!["v1.0.0".to_owned()]);

        drop(store);
        let _ = fs::remove_dir_all(db_path);
    }
}
