use std::{path::Path, sync::Arc};

use eyre::{Context, Result};
use git2::{DiffFormat, DiffOptions, ObjectType, Repository};

use crate::{cache::RenderCache, config::GitConfig, paths};

pub struct RepoReader {
    config: Arc<GitConfig>,
    cache: RenderCache,
}

impl RepoReader {
    pub fn new(config: Arc<GitConfig>, cache: RenderCache) -> Self {
        Self { config, cache }
    }

    pub async fn rendered_readme(
        &self,
        owner: &str,
        repo: &str,
        rev: &str,
    ) -> Result<Option<String>> {
        let cache_key = format!("{owner}:{repo}:{rev}");
        if let Some(cached) = self.cache.get_readme(&cache_key).await {
            return Ok(Some(cached));
        }

        let repo_path = paths::repo_path(&self.config.repo_scan_path, owner, repo)?;
        let _gix_repo = gix::open(&repo_path)
            .with_context(|| format!("failed to open repo with gix at {}", repo_path.display()))?;

        let repository = Repository::open_bare(&repo_path)
            .or_else(|_| Repository::open(&repo_path))
            .with_context(|| format!("failed to open repository at {}", repo_path.display()))?;

        let object = repository
            .revparse_single(rev)
            .with_context(|| format!("failed to resolve revision {rev}"))?;
        let commit = object
            .peel_to_commit()
            .context("revision is not a commit")?;
        let tree = commit.tree().context("failed to read commit tree")?;

        for candidate in &self.config.readme_names {
            if let Ok(entry) = tree.get_path(Path::new(candidate))
                && entry.kind() == Some(ObjectType::Blob)
            {
                let blob = repository
                    .find_blob(entry.id())
                    .context("failed to load readme blob")?;
                let rendered = String::from_utf8_lossy(blob.content()).to_string();
                self.cache
                    .insert_readme(cache_key.clone(), rendered.clone())
                    .await;
                return Ok(Some(rendered));
            }
        }

        Ok(None)
    }

    pub async fn diff_between(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
    ) -> Result<String> {
        let cache_key = format!("{owner}:{repo}:{base}..{head}");
        if let Some(cached) = self.cache.get_diff(&cache_key).await {
            return Ok(cached);
        }

        let repo_path = paths::repo_path(&self.config.repo_scan_path, owner, repo)?;
        let _gix_repo = gix::open(&repo_path)
            .with_context(|| format!("failed to open repo with gix at {}", repo_path.display()))?;

        let repository = Repository::open_bare(&repo_path)
            .or_else(|_| Repository::open(&repo_path))
            .with_context(|| format!("failed to open repository at {}", repo_path.display()))?;

        let base_obj = repository
            .revparse_single(base)
            .with_context(|| format!("failed to resolve base revision {base}"))?;
        let head_obj = repository
            .revparse_single(head)
            .with_context(|| format!("failed to resolve head revision {head}"))?;

        let base_tree = base_obj
            .peel_to_commit()
            .context("base revision is not a commit")?
            .tree()
            .context("failed to read base tree")?;
        let head_tree = head_obj
            .peel_to_commit()
            .context("head revision is not a commit")?
            .tree()
            .context("failed to read head tree")?;

        let mut options = DiffOptions::new();
        let diff = repository
            .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut options))
            .context("failed to generate tree diff")?;

        let mut output = Vec::new();
        diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
            output.extend_from_slice(line.content());
            true
        })
        .context("failed to render patch")?;

        let rendered = String::from_utf8_lossy(&output).to_string();
        self.cache
            .insert_diff(cache_key.clone(), rendered.clone())
            .await;

        Ok(rendered)
    }
}
