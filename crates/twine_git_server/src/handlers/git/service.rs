use std::process::Stdio;

use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use tokio::{io::AsyncWriteExt, process::Command};

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

const MAX_SERVICE_BODY_BYTES: usize = 8 * 1024 * 1024;

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
    body: Body,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let request_body = match to_bytes(body, MAX_SERVICE_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                owner,
                repo = repo_name,
                service,
                ?error,
                "failed to read git service request body"
            );
            return api_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PayloadTooLarge",
                "request body is too large",
            );
        }
    };

    {
        let repos = state.repos.read().await;
        let key = (owner.clone(), repo_name.clone());
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
    }

    if service == "git-receive-pack"
        && let Err(message) = enforce_push_policy(&owner, &repo_name, actor, &request_body)
    {
        return api_error(StatusCode::FORBIDDEN, "PolicyDenied", message);
    }

    let response_body =
        match execute_git_service(&state, &owner, &repo_name, service, &request_body).await {
            Ok(output) => output,
            Err(error) => {
                tracing::warn!(
                    owner,
                    repo = repo_name,
                    service,
                    ?error,
                    "git service execution failed"
                );
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "TransportError",
                    format!("failed to execute {service}"),
                );
            }
        };

    let mut response = Response::new(Body::from(response_body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        service_content_type(service, "result"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn execute_git_service(
    state: &AppState,
    owner: &str,
    repo_name: &str,
    service: &str,
    request_body: &[u8],
) -> eyre::Result<Vec<u8>> {
    let repo_path = state.git.repo_path(owner, repo_name)?;
    if !repo_path.exists() {
        eyre::bail!("repository path does not exist: {}", repo_path.display());
    }

    let subcommand = match service {
        "git-upload-pack" => "upload-pack",
        "git-receive-pack" => "receive-pack",
        _ => eyre::bail!("unsupported git service: {service}"),
    };

    let mut child = Command::new("git")
        .arg(subcommand)
        .arg("--stateless-rpc")
        .arg(repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(request_body).await?;
        stdin.shutdown().await?;
    }

    let output = child.wait_with_output().await?;
    if !output.status.success() {
        eyre::bail!(
            "git {service} exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.stdout)
}

fn enforce_push_policy(
    owner: &str,
    repo_name: &str,
    actor: &str,
    request_body: &[u8],
) -> Result<(), &'static str> {
    tracing::debug!(
        owner,
        repo = repo_name,
        actor,
        bytes = request_body.len(),
        "evaluating push policy hook"
    );

    Ok(())
}
