use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

pub(super) fn service_content_type(service: &str, suffix: &str) -> HeaderValue {
    let value = format!("application/x-{service}-{suffix}");
    HeaderValue::from_str(&value)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"))
}

pub(super) async fn service_post(
    state: AppState,
    headers: HeaderMap,
    owner: String,
    repo_name: String,
    service: &'static str,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let repos = state.repos.read().await;
    let key = (owner, repo_name);
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

    let allowed = if service == "git-upload-pack" {
        role.can_read()
    } else {
        role.can_write()
    };

    if !allowed {
        return api_error(StatusCode::FORBIDDEN, "Forbidden", "service access denied");
    }

    let mut response = Response::new(Body::from("0000"));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        service_content_type(service, "result"),
    );
    response
}
