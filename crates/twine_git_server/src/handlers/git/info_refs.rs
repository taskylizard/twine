use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use git2::Repository;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

use super::service::service_content_type;

#[derive(Debug, Deserialize, ToSchema)]
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

    let advertised_refs = read_advertised_refs(&state, &repo.owner, &repo.name);
    let advertisement = build_advertisement(&query.service, &advertised_refs);

    let mut response = Response::new(Body::from(advertisement));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        service_content_type(&query.service, "advertisement"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache"),
    );
    response
}

fn read_advertised_refs(state: &AppState, owner: &str, repo: &str) -> Vec<(String, String)> {
    let repo_path = match state.git.repo_path(owner, repo) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(
                owner,
                repo,
                ?error,
                "failed to resolve repository path for info/refs"
            );
            return Vec::new();
        }
    };

    let repository = match Repository::open_bare(&repo_path)
        .or_else(|_| Repository::open(&repo_path))
    {
        Ok(repository) => repository,
        Err(error) => {
            tracing::warn!(owner, repo, path = %repo_path.display(), ?error, "failed to open repository for info/refs advertisement");
            return Vec::new();
        }
    };

    let mut refs = Vec::new();
    let iterator = match repository.references() {
        Ok(iterator) => iterator,
        Err(error) => {
            tracing::warn!(
                owner,
                repo,
                ?error,
                "failed to iterate repository refs for info/refs"
            );
            return Vec::new();
        }
    };

    for reference in iterator.flatten() {
        let Some(name) = reference.name() else {
            continue;
        };
        let Some(target) = reference.target() else {
            continue;
        };

        refs.push((target.to_string(), name.to_owned()));
    }

    refs.sort_by(|left, right| left.1.cmp(&right.1));
    refs
}

fn capabilities_for_service(service: &str) -> &'static str {
    match service {
        "git-upload-pack" => {
            "multi_ack_detailed no-done side-band-64k thin-pack ofs-delta shallow deepen-since deepen-not include-tag symref=HEAD:refs/heads/main object-format=sha1 agent=twine/0.1"
        }
        "git-receive-pack" => {
            "report-status report-status-v2 delete-refs side-band-64k quiet atomic ofs-delta push-options object-format=sha1 agent=twine/0.1"
        }
        _ => "agent=twine/0.1",
    }
}

fn pkt_line(payload: &str) -> String {
    format!("{:04x}{payload}", payload.len() + 4)
}

fn build_advertisement(service: &str, refs: &[(String, String)]) -> String {
    let mut output = String::new();
    output.push_str(&pkt_line(&format!("# service={service}\n")));
    output.push_str("0000");

    let caps = capabilities_for_service(service);
    let mut iter = refs.iter();

    if let Some((oid, name)) = iter.next() {
        output.push_str(&pkt_line(&format!("{oid} {name}\0{caps}\n")));
    }

    for (oid, name) in iter {
        output.push_str(&pkt_line(&format!("{oid} {name}\n")));
    }

    output.push_str("0000");
    output
}

#[cfg(test)]
mod tests {
    use super::{build_advertisement, pkt_line};

    #[test]
    fn test_pkt_line_encodes_length_prefix() {
        let encoded = pkt_line("hello\n");
        assert_eq!(encoded, "000ahello\n");
    }

    #[test]
    fn test_advertisement_includes_service_and_refs() {
        let refs = vec![
            (
                "1111111111111111111111111111111111111111".to_owned(),
                "refs/heads/main".to_owned(),
            ),
            (
                "2222222222222222222222222222222222222222".to_owned(),
                "refs/tags/v1.0.0".to_owned(),
            ),
        ];

        let advertisement = build_advertisement("git-upload-pack", &refs);
        assert!(advertisement.starts_with("001e# service=git-upload-pack\n0000"));
        assert!(advertisement.contains("refs/heads/main"));
        assert!(advertisement.contains("refs/tags/v1.0.0"));
        assert!(advertisement.ends_with("0000"));
    }
}
