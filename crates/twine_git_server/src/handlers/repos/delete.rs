use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct DeleteRepoQuery {
    owner: String,
    repo: String,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DeleteRepoQuery>,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let key = (query.owner, query.repo);

    let (removed_repo, repos_snapshot) = {
        let mut repos = state.repos.write().await;
        let Some(repo) = repos.get(&key) else {
            return api_error(StatusCode::NOT_FOUND, "NotFound", "repository not found");
        };

        let Some(role) = lookup_role(repo, actor) else {
            return api_error(
                StatusCode::FORBIDDEN,
                "Forbidden",
                "actor has no role in this repository",
            );
        };

        if !role.can_admin() {
            return api_error(
                StatusCode::FORBIDDEN,
                "Forbidden",
                "only owners may delete repositories",
            );
        }

        let removed_repo = repos
            .remove(&key)
            .expect("repo should exist when deleting after pre-check");
        let repos_snapshot = repos.values().cloned().collect::<Vec<_>>();
        (removed_repo, repos_snapshot)
    };

    if let Err(error) = state.persist_repo_catalog(repos_snapshot).await {
        tracing::error!(owner = %removed_repo.owner, repo = %removed_repo.name, ?error, "failed to persist repository catalog after delete");

        let mut repos = state.repos.write().await;
        repos.insert(key, removed_repo);

        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            "failed to persist repository catalog",
        );
    }

    StatusCode::NO_CONTENT.into_response()
}
