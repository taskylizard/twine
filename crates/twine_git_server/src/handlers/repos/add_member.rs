use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::debug;
use utoipa::ToSchema;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::{AppState, RepoRole},
};

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct AddMemberRequest {
    owner: String,
    repo: String,
    member_id: String,
    role: RepoRole,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AddMemberRequest>,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let key = (req.owner, req.repo);
    let member_id = req.member_id;
    let member_role = req.role;

    let (repo_snapshot, previous_member_role, repos_snapshot) = {
        let mut repos = state.repos.write().await;
        let Some(repo) = repos.get_mut(&key) else {
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
                "only owners may manage members",
            );
        }

        debug!(repo = %repo.name, actor = actor, member = %member_id, "adding member");
        let previous_member_role = repo.members.insert(member_id.clone(), member_role);
        let repo_snapshot = repo.clone();
        let repos_snapshot = repos.values().cloned().collect::<Vec<_>>();

        (repo_snapshot, previous_member_role, repos_snapshot)
    };

    if let Err(error) = state.persist_repo_catalog(repos_snapshot).await {
        tracing::error!(owner = %repo_snapshot.owner, repo = %repo_snapshot.name, ?error, "failed to persist repository catalog after membership update");

        let mut repos = state.repos.write().await;
        if let Some(repo) = repos.get_mut(&key) {
            if let Some(previous_role) = previous_member_role {
                repo.members.insert(member_id, previous_role);
            } else {
                repo.members.remove(&member_id);
            }
        }

        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            "failed to persist repository catalog",
        );
    }

    (StatusCode::OK, Json(repo_snapshot)).into_response()
}
