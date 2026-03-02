use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use serde::Deserialize;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

use super::service::service_content_type;

#[derive(Debug, Deserialize)]
pub(crate) struct InfoRefsQuery {
    service: String,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<InfoRefsQuery>,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let repos = state.repos.read().await;
    let key = (owner, repo);
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

    let allowed = match query.service.as_str() {
        "git-upload-pack" => role.can_read(),
        "git-receive-pack" => role.can_write(),
        _ => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "InvalidRequest",
                "service must be git-upload-pack or git-receive-pack",
            );
        }
    };

    if !allowed {
        return api_error(StatusCode::FORBIDDEN, "Forbidden", "service access denied");
    }

    let advertisement = format!("001e# service={}\n0000", query.service);

    let mut response = Response::new(Body::from(advertisement));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        service_content_type(&query.service, "advertisement"),
    );
    response
}
