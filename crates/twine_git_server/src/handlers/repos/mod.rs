mod add_member;
mod create;
mod delete;
mod get;
mod list_branches;
mod list_tags;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/repos", post(create::handler))
        .route("/api/repos", delete(delete::handler))
        .route("/api/repos", get(get::handler))
        .route("/api/repos/members", post(add_member::handler))
        .route("/api/repos/branches", get(list_branches::handler))
        .route("/api/repos/tags", get(list_tags::handler))
}
