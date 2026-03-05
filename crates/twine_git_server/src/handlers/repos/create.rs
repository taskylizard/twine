use std::collections::HashMap;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::info;
use utoipa::ToSchema;

use crate::{
    auth::actor_id,
    errors::api_error,
    state::{AppState, Repo, RepoRole},
};

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct CreateRepoRequest {
    owner: String,
    repo: String,
    description: Option<String>,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateRepoRequest>,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    if actor != req.owner {
        return api_error(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "actor can only create repositories for their own account",
        );
    }

    let key = (req.owner.clone(), req.repo.clone());

    {
        let repos = state.repos.read().await;
        if repos.contains_key(&key) {
            return api_error(
                StatusCode::CONFLICT,
                "AlreadyExists",
                "repository already exists",
            );
        }
    }

    let mut members = HashMap::new();
    members.insert(req.owner.clone(), RepoRole::Owner);

    let repo = Repo {
        owner: req.owner,
        name: req.repo,
        description: req.description,
        default_branch: "main".to_owned(),
        branches: vec!["main".to_owned()],
        tags: Vec::new(),
        members,
    };

    if let Err(error) = state.git.upsert_metadata_snapshot(
        &repo.owner,
        &repo.name,
        &repo.default_branch,
        repo.branches.clone(),
        repo.tags.clone(),
    ) {
        tracing::error!(owner = %repo.owner, repo = %repo.name, ?error, "failed to persist repo metadata snapshot");
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            "failed to persist repository metadata",
        );
    }

    let snapshot = {
        let mut repos = state.repos.write().await;
        repos.insert(key.clone(), repo.clone());
        repos.values().cloned().collect::<Vec<_>>()
    };

    if let Err(error) = state.persist_repo_catalog(snapshot).await {
        tracing::error!(owner = %repo.owner, repo = %repo.name, ?error, "failed to persist repository catalog");

        let mut repos = state.repos.write().await;
        repos.remove(&key);

        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "InternalError",
            "failed to persist repository catalog",
        );
    }

    info!(owner = %repo.owner, repo = %repo.name, "created repository");
    (StatusCode::CREATED, Json(repo)).into_response()
}
