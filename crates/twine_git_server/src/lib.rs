mod auth;
mod docs;
mod errors;
mod handlers;
mod state;

use axum::Router;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

pub use state::{Repo, RepoRole};

pub fn router() -> Router {
    router_with_state(state::AppState::default())
}

pub fn router_with_git_config(config: twine_git_operations::GitConfig) -> Router {
    router_with_state(state::AppState::from_git_config(config))
}

fn router_with_state(state: state::AppState) -> Router {
    Router::new()
        .merge(handlers::repos::router())
        .merge(handlers::git::router())
        .merge(Scalar::with_url("/scalar", docs::ApiDoc::openapi()))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::router;

    #[tokio::test]
    async fn test_create_repo_requires_actor_header() {
        let app = router();
        let request = Request::builder()
            .method("POST")
            .uri("/api/repos")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"owner":"alice","repo":"demo"}"#))
            .expect("request should build");

        let response = app.oneshot(request).await.expect("request should succeed");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_collaborator_cannot_push() {
        let app = router();

        let create = Request::builder()
            .method("POST")
            .uri("/api/repos")
            .header("x-actor-id", "alice")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"owner":"alice","repo":"demo"}"#))
            .expect("request should build");
        let create_res = app
            .clone()
            .oneshot(create)
            .await
            .expect("create should complete");
        assert_eq!(create_res.status(), StatusCode::CREATED);

        let add_member = Request::builder()
            .method("POST")
            .uri("/api/repos/members")
            .header("x-actor-id", "alice")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"owner":"alice","repo":"demo","member_id":"bob","role":"collaborator"}"#,
            ))
            .expect("request should build");
        let add_member_res = app
            .clone()
            .oneshot(add_member)
            .await
            .expect("add member should complete");
        assert_eq!(add_member_res.status(), StatusCode::OK);

        let pull = Request::builder()
            .method("GET")
            .uri("/alice/demo/info/refs?service=git-upload-pack")
            .header("x-actor-id", "bob")
            .body(Body::empty())
            .expect("request should build");
        let pull_res = app
            .clone()
            .oneshot(pull)
            .await
            .expect("pull should complete");
        assert_eq!(pull_res.status(), StatusCode::OK);

        let push = Request::builder()
            .method("POST")
            .uri("/alice/demo/git-receive-pack")
            .header("x-actor-id", "bob")
            .body(Body::empty())
            .expect("request should build");
        let push_res = app.oneshot(push).await.expect("push should complete");
        assert_eq!(push_res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_owner_can_list_branches() {
        let app = router();

        let create = Request::builder()
            .method("POST")
            .uri("/api/repos")
            .header("x-actor-id", "alice")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"owner":"alice","repo":"demo"}"#))
            .expect("request should build");
        let create_res = app
            .clone()
            .oneshot(create)
            .await
            .expect("create should complete");
        assert_eq!(create_res.status(), StatusCode::CREATED);

        let list = Request::builder()
            .method("GET")
            .uri("/api/repos/branches?owner=alice&repo=demo")
            .header("x-actor-id", "alice")
            .body(Body::empty())
            .expect("request should build");
        let list_res = app
            .oneshot(list)
            .await
            .expect("list branches should complete");
        assert_eq!(list_res.status(), StatusCode::OK);

        let body = to_bytes(list_res.into_body(), 1024)
            .await
            .expect("response body should be readable");
        let json = String::from_utf8(body.to_vec()).expect("response body should be utf8");
        assert!(json.contains("main"));
    }
}
