use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::debug;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::{AppState, RepoRole},
};

#[derive(Debug, Deserialize)]
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

    let mut repos = state.repos.write().await;
    let key = (req.owner, req.repo);
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

    debug!(repo = %repo.name, actor = actor, member = %req.member_id, "adding member");
    repo.members.insert(req.member_id, req.role);

    (StatusCode::OK, Json(repo.clone())).into_response()
}
