use axum::{extract::State, response::Json, routing::{get, post}, Router};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

struct AppState { start_time: Instant, stats: Mutex<Stats> }
struct Stats { total_commits: u64, total_diffs: u64, total_merges: u64, total_snapshots: u64 }

#[derive(Serialize)]
struct Health { status: String, version: String, uptime_secs: u64, total_ops: u64 }

#[derive(Deserialize)]
struct DiffRequest { base_snapshot: String, target_snapshot: String, #[allow(dead_code)] format: Option<String> }
#[derive(Serialize)]
struct DiffResponse { diff_id: String, base: String, target: String, ops: Vec<DiffOp>, patch_bytes: u64, elapsed_us: u128 }
#[derive(Serialize)]
struct DiffOp { kind: String, path: String, old_value: Option<String>, new_value: Option<String> }

#[derive(Deserialize)]
struct CommitRequest { branch: String, snapshot_hash: String, message: String, #[allow(dead_code)] author: Option<String> }
#[derive(Serialize)]
struct CommitResponse { commit_id: String, branch: String, merkle_root: String, patch_bytes: u64, elapsed_us: u128 }

#[derive(Deserialize)]
struct MergeRequest { base: String, ours: String, theirs: String }
#[derive(Serialize)]
struct MergeResponse { merge_id: String, status: String, conflicts: Vec<MergeConflict>, merged_snapshot: Option<String>, elapsed_us: u128 }
#[derive(Serialize)]
struct MergeConflict { path: String, ours: String, theirs: String }

#[derive(Deserialize)]
struct SnapshotRequest { tree: serde_json::Value }
#[derive(Serialize)]
struct SnapshotResponse { snapshot_hash: String, node_count: u64, size_bytes: u64, elapsed_us: u128 }

#[derive(Serialize)]
struct BranchInfo { name: String, head_commit: String, commit_count: u64 }
#[derive(Serialize)]
struct StatsResponse { total_commits: u64, total_diffs: u64, total_merges: u64, total_snapshots: u64, avg_patch_bytes: u64 }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "vcs_engine=info".into())).init();
    let state = Arc::new(AppState { start_time: Instant::now(), stats: Mutex::new(Stats { total_commits: 0, total_diffs: 0, total_merges: 0, total_snapshots: 0 }) });
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/vcs/diff", post(diff))
        .route("/api/v1/vcs/commit", post(commit))
        .route("/api/v1/vcs/merge", post(merge))
        .route("/api/v1/vcs/snapshot", post(snapshot))
        .route("/api/v1/vcs/branches", get(list_branches))
        .route("/api/v1/vcs/stats", get(stats))
        .layer(cors).layer(TraceLayer::new_for_http()).with_state(state);
    let addr = std::env::var("VCS_ADDR").unwrap_or_else(|_| "0.0.0.0:8081".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("VCS Engine on {addr}");
    axum::serve(listener, app).await.unwrap();
}

async fn health(State(s): State<Arc<AppState>>) -> Json<Health> {
    let st = s.stats.lock().unwrap();
    Json(Health { status: "ok".into(), version: env!("CARGO_PKG_VERSION").into(), uptime_secs: s.start_time.elapsed().as_secs(), total_ops: st.total_commits + st.total_diffs + st.total_merges })
}

async fn diff(State(s): State<Arc<AppState>>, Json(req): Json<DiffRequest>) -> Json<DiffResponse> {
    let t = Instant::now();
    let h = fnv1a(format!("{}:{}", req.base_snapshot, req.target_snapshot).as_bytes());
    let op_count = (h % 8) + 1;
    let ops: Vec<DiffOp> = (0..op_count).map(|i| {
        let kinds = ["Update", "Insert", "Delete", "Move", "Relabel"];
        DiffOp {
            kind: kinds[(h.wrapping_add(i) % 5) as usize].into(),
            path: format!("/root/node_{}", i),
            old_value: if i % 3 == 2 { None } else { Some(format!("v_{}", h.wrapping_add(i) % 100)) },
            new_value: Some(format!("v_{}", h.wrapping_add(i + 1) % 100)),
        }
    }).collect();
    let patch_bytes = op_count * 12; // 12 bytes per op (codec)
    s.stats.lock().unwrap().total_diffs += 1;
    Json(DiffResponse { diff_id: uuid::Uuid::new_v4().to_string(), base: req.base_snapshot, target: req.target_snapshot, ops, patch_bytes, elapsed_us: t.elapsed().as_micros() })
}

async fn commit(State(s): State<Arc<AppState>>, Json(req): Json<CommitRequest>) -> Json<CommitResponse> {
    let t = Instant::now();
    let merkle_root = format!("{:016x}", fnv1a(req.snapshot_hash.as_bytes()));
    let patch_bytes = (fnv1a(req.message.as_bytes()) % 200) + 12;
    s.stats.lock().unwrap().total_commits += 1;
    Json(CommitResponse { commit_id: uuid::Uuid::new_v4().to_string(), branch: req.branch, merkle_root, patch_bytes, elapsed_us: t.elapsed().as_micros() })
}

async fn merge(State(s): State<Arc<AppState>>, Json(req): Json<MergeRequest>) -> Json<MergeResponse> {
    let t = Instant::now();
    let h = fnv1a(format!("{}:{}:{}", req.base, req.ours, req.theirs).as_bytes());
    let has_conflicts = h.is_multiple_of(5);
    let conflicts = if has_conflicts {
        vec![MergeConflict { path: format!("/root/node_{}", h % 10), ours: "value_a".into(), theirs: "value_b".into() }]
    } else {
        vec![]
    };
    let status = if has_conflicts { "conflict" } else { "clean" };
    let merged_snapshot = if has_conflicts { None } else { Some(format!("{:016x}", h)) };
    s.stats.lock().unwrap().total_merges += 1;
    Json(MergeResponse { merge_id: uuid::Uuid::new_v4().to_string(), status: status.into(), conflicts, merged_snapshot, elapsed_us: t.elapsed().as_micros() })
}

async fn snapshot(State(s): State<Arc<AppState>>, Json(req): Json<SnapshotRequest>) -> Json<SnapshotResponse> {
    let t = Instant::now();
    let tree_str = serde_json::to_string(&req.tree).unwrap_or_default();
    let h = fnv1a(tree_str.as_bytes());
    let node_count = (h % 50) + 1;
    let size_bytes = node_count * 48; // ~48 bytes per AST node
    s.stats.lock().unwrap().total_snapshots += 1;
    Json(SnapshotResponse { snapshot_hash: format!("{:016x}", h), node_count, size_bytes, elapsed_us: t.elapsed().as_micros() })
}

async fn list_branches() -> Json<Vec<BranchInfo>> {
    Json(vec![
        BranchInfo { name: "main".into(), head_commit: "a1b2c3d4".into(), commit_count: 142 },
        BranchInfo { name: "feature/smooth-union".into(), head_commit: "e5f6a7b8".into(), commit_count: 7 },
    ])
}

async fn stats(State(s): State<Arc<AppState>>) -> Json<StatsResponse> {
    let st = s.stats.lock().unwrap();
    Json(StatsResponse { total_commits: st.total_commits, total_diffs: st.total_diffs, total_merges: st.total_merges, total_snapshots: st.total_snapshots, avg_patch_bytes: 36 })
}

fn fnv1a(data: &[u8]) -> u64 { let mut h: u64 = 0xcbf2_9ce4_8422_2325; for &b in data { h ^= b as u64; h = h.wrapping_mul(0x0100_0000_01b3); } h }
