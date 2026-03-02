mod info_refs;
mod receive_pack;
mod service;
mod upload_pack;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{owner}/{repo}/info/refs", get(info_refs::handler))
        .route("/{owner}/{repo}/git-upload-pack", post(upload_pack::handler))
        .route(
            "/{owner}/{repo}/git-receive-pack",
            post(receive_pack::handler),
        )
}
