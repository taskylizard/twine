# Twine

Twine is a general-purpose Git collaboration server built in Rust.

## Development setup

This guide gets you to a working local server where you can run:

- repo API calls (`/api/repos`, `/api/repos/members`, ...)
- Smart HTTP Git operations (`ls-remote`, `clone`, `fetch`, `push`)

## Prerequisites

- Rust toolchain (stable)
- `git` CLI installed
- Linux/macOS shell

## 1) Create a local config

From repo root:

```bash
cp twine.toml.example twine.toml
```

Edit `twine.toml`:

```toml
bind_addr = "127.0.0.1:3000"
log_filter = "info"

[git]
repo_scan_path = "/tmp/twine-dev/git"
metadata_db_path = "/tmp/twine-dev/metadata/db"
metadata_reindex_interval_secs = 300
readme_cache_capacity = 128
diff_cache_capacity = 128
cache_ttl_secs = 300
```

Create directories:

```bash
mkdir -p /tmp/twine-dev/git
mkdir -p /tmp/twine-dev/metadata
```

## 2) Create and seed a bare repo

```bash
mkdir -p /tmp/twine-dev/git/alice
```

```bash
git init --bare /tmp/twine-dev/git/alice/demo
```

Seed with a `main` branch:

```bash
mkdir -p /tmp/twine-seed
```

```bash
git init /tmp/twine-seed
```

```bash
git -C /tmp/twine-seed config user.email twine@example.test
```

```bash
git -C /tmp/twine-seed config user.name "Twine Test"
```

```bash
echo "# demo" > /tmp/twine-seed/README.md
```

```bash
git -C /tmp/twine-seed add README.md
```

```bash
git -C /tmp/twine-seed commit -m "seed"
```

```bash
git -C /tmp/twine-seed branch -M main
```

```bash
git -C /tmp/twine-seed remote add origin /tmp/twine-dev/git/alice/demo
```

```bash
git -C /tmp/twine-seed push origin HEAD:refs/heads/main
```

```bash
git -C /tmp/twine-dev/git/alice/demo symbolic-ref HEAD refs/heads/main
```

## 3) Start Twine

From repo root:

```bash
cargo run
```

Server will listen on `bind_addr` from `twine.toml`.

## 4) Register repo in Twine API state

Twine currently requires the repo to exist on disk **and** in API state.

```bash
curl -i \
  -H 'x-actor-id: alice' \
  -H 'content-type: application/json' \
  -d '{"owner":"alice","repo":"demo"}' \
  http://127.0.0.1:3000/api/repos
```

## 5) Try smart HTTP Git

### ls-remote

```bash
git -c "http.extraHeader=x-actor-id: alice" ls-remote http://127.0.0.1:3000/alice/demo
```

### clone

```bash
git -c "http.extraHeader=x-actor-id: alice" clone http://127.0.0.1:3000/alice/demo /tmp/twine-clone
```

### fetch

```bash
git -C /tmp/twine-clone -c "http.extraHeader=x-actor-id: alice" fetch origin
```

### push (owner)

```bash
echo "owner push" > /tmp/twine-clone/OWNER_PUSH.md
```

```bash
git -C /tmp/twine-clone add OWNER_PUSH.md
```

```bash
git -C /tmp/twine-clone commit -m "owner push"
```

```bash
git -C /tmp/twine-clone -c "http.extraHeader=x-actor-id: alice" push origin +HEAD:refs/heads/main
```

## 6) Verify collaborator cannot push

Add collaborator:

```bash
curl -i \
  -H 'x-actor-id: alice' \
  -H 'content-type: application/json' \
  -d '{"owner":"alice","repo":"demo","member_id":"bob","role":"collaborator"}' \
  http://127.0.0.1:3000/api/repos/members
```

Clone as collaborator:

```bash
git -c "http.extraHeader=x-actor-id: bob" clone http://127.0.0.1:3000/alice/demo /tmp/twine-bob
```

Attempt push (expected to fail):

```bash
echo "blocked" > /tmp/twine-bob/BLOCKED.md
```

```bash
git -C /tmp/twine-bob add BLOCKED.md
```

```bash
git -C /tmp/twine-bob commit -m "blocked push"
```

```bash
git -C /tmp/twine-bob -c "http.extraHeader=x-actor-id: bob" push origin +HEAD:refs/heads/main
```

You should see a permission failure.

## Useful checks

```bash
cargo test --workspace
```

```bash
cargo clippy --all-targets --all-features -- -D warnings
```
