# ALICE-VCS-SaaS

AST-level version control service. Diffs the logic tree, not the text. Content-addressed Merkle DAG snapshots, 12-byte binary patches, structural 3-way merge with conflict detection.

## Architecture

```
Client
  │
  ▼
┌─────────────────────────────────────────┐
│          ALICE-VCS-SaaS API             │
│         (Rust / Axum, port 8081)        │
└────────────┬──────────────┬────────────┘
             │              │
    ┌─────────────┐  ┌──────────────────┐
    │  AST Diff   │  │  Snapshot Store  │
    │  Engine     │  │  (Merkle DAG)    │
    └──────┬──────┘  └────────┬─────────┘
           │                  │
  ┌────────▼──────────────────▼─────────┐
  │        Content-Addressed Store      │
  │  (FNV-1a hash, 12-byte patches)    │
  └────────────────────────────────────┘
           │                  │
  ┌────────▼────────┐  ┌─────▼─────────┐
  │  3-Way Merge    │  │  Commit Model │
  │  (Conflict det.)│  │  (Branch/Tag) │
  └─────────────────┘  └───────────────┘
```

## Features

| Feature | Details |
|---------|---------|
| AST Semantic Diff | Tree-level diff (Insert, Delete, Update, Move, Relabel) |
| 12-Byte Patches | Binary codec: 4-12 bytes per edit op vs 50 KB text diffs |
| Merkle DAG Snapshots | Content-addressed, deduplicated, FNV-1a hashed |
| 3-Way Merge | Structural merge with automatic conflict detection |
| Branch/Commit Model | Git-compatible branching with lightweight commits |

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/vcs/diff` | AST diff between two snapshots |
| POST | `/api/v1/vcs/commit` | Create a commit on a branch |
| POST | `/api/v1/vcs/merge` | 3-way merge with conflict detection |
| POST | `/api/v1/vcs/snapshot` | Store an AST tree snapshot (returns Merkle hash) |
| GET | `/api/v1/vcs/branches` | List branches with head commit |
| GET | `/api/v1/vcs/stats` | Commit/diff/merge throughput stats |

## Quick Start

```bash
# Clone and start the backend
git clone https://github.com/ext-sakamoro/ALICE-VCS-SaaS.git
cd ALICE-VCS-SaaS
cargo run --release

# In a second terminal, start the frontend
cd frontend
npm install
npm run dev
# Open http://localhost:3000
```

### Example: Create a Snapshot

```bash
curl -X POST http://localhost:8081/api/v1/vcs/snapshot \
  -H "Content-Type: application/json" \
  -d '{
    "tree": {
      "kind": "Root",
      "children": [
        {"kind": "Primitive", "name": "sphere", "value": 1.0},
        {"kind": "Primitive", "name": "box", "value": [0.5, 0.5, 0.5]}
      ]
    }
  }'
```

### Example: Diff Two Snapshots

```bash
curl -X POST http://localhost:8081/api/v1/vcs/diff \
  -H "Content-Type: application/json" \
  -d '{
    "base_snapshot": "a1b2c3d4e5f6a7b8",
    "target_snapshot": "c3d4e5f6a7b8a1b2"
  }'
```

### Example: 3-Way Merge

```bash
curl -X POST http://localhost:8081/api/v1/vcs/merge \
  -H "Content-Type: application/json" \
  -d '{
    "base": "a1b2c3d4e5f6a7b8",
    "ours": "b2c3d4e5f6a7b8a1",
    "theirs": "c3d4e5f6a7b8a1b2"
  }'
```

## License

`LicenseRef-Proprietary`. The text is [LICENSE-Proprietary](LICENSE-Proprietary).

Enquiries: https://extoria.co.jp/
