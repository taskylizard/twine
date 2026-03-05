pub(crate) mod add_member;
pub(crate) mod create;
pub(crate) mod delete;
pub(crate) mod get;
pub(crate) mod list_branches;
pub(crate) mod list_tags;

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
