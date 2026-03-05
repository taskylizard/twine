use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    components(
        schemas(
            crate::state::Repo,
            crate::state::RepoRole,
            crate::errors::ErrorEnvelope,
            crate::handlers::repos::create::CreateRepoRequest,
            crate::handlers::repos::get::GetRepoQuery,
            crate::handlers::repos::delete::DeleteRepoQuery,
            crate::handlers::repos::add_member::AddMemberRequest,
            crate::handlers::repos::list_branches::ListBranchesQuery,
            crate::handlers::repos::list_branches::ListBranchesResponse,
            crate::handlers::repos::list_tags::ListTagsQuery,
            crate::handlers::repos::list_tags::ListTagsResponse,
            crate::handlers::git::info_refs::InfoRefsQuery
        )
    ),
    tags(
        (name = "repos", description = "Repository and membership endpoints"),
        (name = "git", description = "Git smart HTTP endpoints")
    )
)]
pub(crate) struct ApiDoc;
