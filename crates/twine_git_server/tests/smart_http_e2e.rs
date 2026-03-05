use std::{
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use tokio::{net::TcpListener, process::Command, task::JoinHandle};
use tower::ServiceExt;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!("twine-{prefix}-{}-{nonce}", std::process::id()))
}

async fn run_cmd_in_dir(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .expect("git command should run");

    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn create_bare_repo_with_main(scan_root: &Path, owner: &str, repo: &str) -> PathBuf {
    let owner_dir = scan_root.join(owner);
    fs::create_dir_all(&owner_dir).expect("owner dir should exist");

    let bare_repo_path = owner_dir.join(repo);
    let output = Command::new("git")
        .args(["init", "--bare"])
        .arg(&bare_repo_path)
        .output()
        .await
        .expect("git init --bare should run");
    assert!(
        output.status.success(),
        "git init --bare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let work_dir = unique_temp_dir("seed-work");
    fs::create_dir_all(&work_dir).expect("seed work dir should exist");

    run_cmd_in_dir(&work_dir, &["init"]).await;
    run_cmd_in_dir(&work_dir, &["config", "user.email", "twine@example.test"]).await;
    run_cmd_in_dir(&work_dir, &["config", "user.name", "Twine Test"]).await;

    fs::write(work_dir.join("README.md"), "# seed\n").expect("seed file should be written");
    run_cmd_in_dir(&work_dir, &["add", "README.md"]).await;
    run_cmd_in_dir(&work_dir, &["commit", "-m", "seed"]).await;
    run_cmd_in_dir(&work_dir, &["branch", "-M", "main"]).await;
    run_cmd_in_dir(
        &work_dir,
        &[
            "remote",
            "add",
            "origin",
            bare_repo_path.to_string_lossy().as_ref(),
        ],
    )
    .await;
    run_cmd_in_dir(&work_dir, &["push", "origin", "HEAD:refs/heads/main"]).await;

    let set_head_output = Command::new("git")
        .args(["symbolic-ref", "HEAD", "refs/heads/main"])
        .current_dir(&bare_repo_path)
        .output()
        .await
        .expect("git symbolic-ref should run");
    assert!(
        set_head_output.status.success(),
        "git symbolic-ref failed: {}",
        String::from_utf8_lossy(&set_head_output.stderr)
    );

    bare_repo_path
}

async fn push_followup_commit(remote_path: &Path) {
    let work_dir = unique_temp_dir("followup-work");
    fs::create_dir_all(&work_dir).expect("followup work dir should exist");

    let clone_output = Command::new("git")
        .arg("clone")
        .arg(remote_path)
        .arg(&work_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("git clone should run");
    assert!(
        clone_output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    run_cmd_in_dir(&work_dir, &["config", "user.email", "twine@example.test"]).await;
    run_cmd_in_dir(&work_dir, &["config", "user.name", "Twine Test"]).await;

    fs::write(work_dir.join("CHANGELOG.md"), "follow-up\n")
        .expect("follow-up file should be written");
    run_cmd_in_dir(&work_dir, &["add", "CHANGELOG.md"]).await;
    run_cmd_in_dir(&work_dir, &["commit", "-m", "follow-up"]).await;
    run_cmd_in_dir(&work_dir, &["push", "origin", "main:refs/heads/main"]).await;
}

async fn seed_repo_in_state(app: &Router) {
    let create = Request::builder()
        .method("POST")
        .uri("/api/repos")
        .header("x-actor-id", "alice")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"owner":"alice","repo":"demo"}"#))
        .expect("request should build");

    let response = app
        .clone()
        .oneshot(create)
        .await
        .expect("seed create request should complete");
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn seed_collaborator_in_state(app: &Router, member_id: &str) {
    let add_member = Request::builder()
        .method("POST")
        .uri("/api/repos/members")
        .header("x-actor-id", "alice")
        .header("content-type", "application/json")
        .body(Body::from(format!(
            r#"{{"owner":"alice","repo":"demo","member_id":"{member_id}","role":"collaborator"}}"#
        )))
        .expect("request should build");

    let response = app
        .clone()
        .oneshot(add_member)
        .await
        .expect("seed add_member request should complete");
    assert_eq!(response.status(), StatusCode::OK);
}

async fn spawn_server(app: Router) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("local addr should be available");

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    (addr, handle)
}

#[tokio::test]
async fn test_git_ls_remote_over_http_returns_main_ref() {
    let scan_root = unique_temp_dir("ls-remote-scan");
    fs::create_dir_all(&scan_root).expect("scan root should exist");

    let metadata_root = unique_temp_dir("ls-remote-metadata-root");
    fs::create_dir_all(&metadata_root).expect("metadata root should exist");
    let metadata_db_path = metadata_root.join("db");

    create_bare_repo_with_main(&scan_root, "alice", "demo").await;

    let config = twine_git_operations::GitConfig {
        repo_scan_path: scan_root.clone(),
        metadata_db_path,
        ..twine_git_operations::GitConfig::default()
    };
    let app = twine_git_server::router_with_git_config(config);
    seed_repo_in_state(&app).await;

    let (addr, server) = spawn_server(app).await;
    let url = format!("http://{addr}/alice/demo");

    let output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: alice",
            "ls-remote",
            &url,
        ])
        .output()
        .await
        .expect("git ls-remote should run");

    server.abort();

    assert!(
        output.status.success(),
        "git ls-remote failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refs/heads/main"), "stdout was: {stdout}");
}

#[tokio::test]
async fn test_owner_can_push_over_http() {
    let scan_root = unique_temp_dir("owner-push-scan");
    fs::create_dir_all(&scan_root).expect("scan root should exist");

    let metadata_root = unique_temp_dir("owner-push-metadata-root");
    fs::create_dir_all(&metadata_root).expect("metadata root should exist");
    let metadata_db_path = metadata_root.join("db");

    create_bare_repo_with_main(&scan_root, "alice", "demo").await;

    let config = twine_git_operations::GitConfig {
        repo_scan_path: scan_root,
        metadata_db_path,
        ..twine_git_operations::GitConfig::default()
    };
    let app = twine_git_server::router_with_git_config(config);
    seed_repo_in_state(&app).await;

    let (addr, server) = spawn_server(app).await;
    let url = format!("http://{addr}/alice/demo");

    let clone_dir = unique_temp_dir("owner-push-clone");
    let clone_output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: alice",
            "clone",
            &url,
            clone_dir.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .expect("git clone should run");
    assert!(
        clone_output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    run_cmd_in_dir(&clone_dir, &["config", "user.email", "twine@example.test"]).await;
    run_cmd_in_dir(&clone_dir, &["config", "user.name", "Twine Test"]).await;

    fs::write(clone_dir.join("OWNER_PUSH.md"), "owner push\n")
        .expect("owner push file should be written");
    run_cmd_in_dir(&clone_dir, &["add", "OWNER_PUSH.md"]).await;
    run_cmd_in_dir(&clone_dir, &["commit", "-m", "owner push"]).await;

    let push_output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: alice",
            "push",
            "origin",
            "+HEAD:refs/heads/main",
        ])
        .current_dir(&clone_dir)
        .output()
        .await
        .expect("git push should run");

    server.abort();

    assert!(
        push_output.status.success(),
        "git push failed: {}",
        String::from_utf8_lossy(&push_output.stderr)
    );
}

#[tokio::test]
async fn test_collaborator_cannot_push_over_http() {
    let scan_root = unique_temp_dir("collab-push-scan");
    fs::create_dir_all(&scan_root).expect("scan root should exist");

    let metadata_root = unique_temp_dir("collab-push-metadata-root");
    fs::create_dir_all(&metadata_root).expect("metadata root should exist");
    let metadata_db_path = metadata_root.join("db");

    create_bare_repo_with_main(&scan_root, "alice", "demo").await;

    let config = twine_git_operations::GitConfig {
        repo_scan_path: scan_root,
        metadata_db_path,
        ..twine_git_operations::GitConfig::default()
    };
    let app = twine_git_server::router_with_git_config(config);
    seed_repo_in_state(&app).await;
    seed_collaborator_in_state(&app, "bob").await;

    let (addr, server) = spawn_server(app).await;
    let url = format!("http://{addr}/alice/demo");

    let clone_dir = unique_temp_dir("collab-push-clone");
    let clone_output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: bob",
            "clone",
            &url,
            clone_dir.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .expect("git clone should run");
    assert!(
        clone_output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    run_cmd_in_dir(&clone_dir, &["config", "user.email", "twine@example.test"]).await;
    run_cmd_in_dir(&clone_dir, &["config", "user.name", "Twine Test"]).await;

    fs::write(clone_dir.join("COLLAB_PUSH.md"), "collab push\n")
        .expect("collaborator push file should be written");
    run_cmd_in_dir(&clone_dir, &["add", "COLLAB_PUSH.md"]).await;
    run_cmd_in_dir(&clone_dir, &["commit", "-m", "collab push"]).await;

    let push_output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: bob",
            "push",
            "origin",
            "+HEAD:refs/heads/main",
        ])
        .current_dir(&clone_dir)
        .output()
        .await
        .expect("git push should run");

    server.abort();

    assert!(
        !push_output.status.success(),
        "collaborator push unexpectedly succeeded"
    );
}

#[tokio::test]
async fn test_git_clone_then_fetch_over_http_succeeds() {
    let scan_root = unique_temp_dir("clone-fetch-scan");
    fs::create_dir_all(&scan_root).expect("scan root should exist");

    let metadata_root = unique_temp_dir("clone-fetch-metadata-root");
    fs::create_dir_all(&metadata_root).expect("metadata root should exist");
    let metadata_db_path = metadata_root.join("db");
    let bare_repo_path = create_bare_repo_with_main(&scan_root, "alice", "demo").await;

    let config = twine_git_operations::GitConfig {
        repo_scan_path: scan_root,
        metadata_db_path,
        ..twine_git_operations::GitConfig::default()
    };
    let app = twine_git_server::router_with_git_config(config);
    seed_repo_in_state(&app).await;

    let (addr, server) = spawn_server(app).await;
    let url = format!("http://{addr}/alice/demo");

    let clone_dir = unique_temp_dir("clone-dest");
    let clone_output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: alice",
            "clone",
            &url,
            clone_dir.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .expect("git clone should run");

    assert!(
        clone_output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    push_followup_commit(&bare_repo_path).await;

    let fetch_output = Command::new("git")
        .args([
            "-c",
            "http.extraHeader=x-actor-id: alice",
            "fetch",
            "origin",
        ])
        .current_dir(&clone_dir)
        .output()
        .await
        .expect("git fetch should run");

    server.abort();

    assert!(
        fetch_output.status.success(),
        "git fetch failed: {}",
        String::from_utf8_lossy(&fetch_output.stderr)
    );
}
