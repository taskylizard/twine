use std::path::{Path, PathBuf};

use eyre::{Result, bail};

pub fn repo_path(scan_path: &Path, owner: &str, repo: &str) -> Result<PathBuf> {
    validate_segment(owner, "owner")?;
    validate_segment(repo, "repo")?;
    Ok(scan_path.join(owner).join(repo))
}

fn validate_segment(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{field} cannot be empty");
    }

    if value.contains("..") || value.contains('/') || value.contains('\\') {
        bail!("{field} contains path traversal characters");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::repo_path;

    #[test]
    fn test_repo_path_uses_expected_layout() {
        let path = repo_path(Path::new("/home/git"), "alice", "myrepo")
            .expect("path should resolve");
        assert_eq!(path.to_string_lossy(), "/home/git/alice/myrepo");
    }

    #[test]
    fn test_repo_path_rejects_traversal() {
        let error = repo_path(Path::new("/home/git"), "alice", "../repo")
            .expect_err("traversal should fail");
        assert!(
            error
                .to_string()
                .contains("repo contains path traversal characters")
        );
    }
}
