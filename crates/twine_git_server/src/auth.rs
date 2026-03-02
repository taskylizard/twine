use axum::http::HeaderMap;

use crate::state::{Repo, RepoRole};

pub(crate) fn actor_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-actor-id")
        .and_then(|value| value.to_str().ok())
}

pub(crate) fn lookup_role(repo: &Repo, actor_id: &str) -> Option<RepoRole> {
    repo.members.get(actor_id).copied()
}
