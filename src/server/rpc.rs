use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::SystemTime;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

const PROTOCOL_VERSION: u32 = 1;

use crate::protocol::request::{
    CloseParams, CountParams, InfoParams, OpenParams, ReadParams, SearchParams,
};
use crate::protocol::response::{CountData, InfoData, LinesData, OpenedData, SearchResultsData};
use crate::reader::FileHandle;
use crate::security::validate_path;

// RwLock allows multiple concurrent reads (search, read, count on different files)
// while still serializing writes (open, close).
type FileStore = Arc<RwLock<FileStoreState>>;

#[derive(Default)]
struct FileStoreState {
    leases: HashMap<u64, Arc<SharedFile>>,
    by_path: HashMap<String, Weak<SharedFile>>,
}

struct SharedFile {
    handle: FileHandle,
    snapshot: FileSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

impl FileSnapshot {
    fn read(path: &str) -> Result<Self> {
        let meta = fs::metadata(path)?;
        Ok(Self::from_metadata(meta))
    }

    fn from_metadata(meta: fs::Metadata) -> Self {
        Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            #[cfg(unix)]
            dev: {
                use std::os::unix::fs::MetadataExt;
                meta.dev()
            },
            #[cfg(unix)]
            ino: {
                use std::os::unix::fs::MetadataExt;
                meta.ino()
            },
        }
    }
}

impl SharedFile {
    fn open(path: &str) -> Result<Self> {
        let handle = FileHandle::open(path)?;
        let snapshot = FileSnapshot::read(path)?;
        Ok(Self { handle, snapshot })
    }
}

impl FileStoreState {
    fn live_handle_for_path(&mut self, path: &str) -> Result<Option<Arc<SharedFile>>> {
        match self.by_path.get(path).and_then(Weak::upgrade) {
            Some(shared) if shared.snapshot == FileSnapshot::read(path)? => Ok(Some(shared)),
            Some(_) => {
                self.by_path.remove(path);
                Ok(None)
            }
            None => {
                self.by_path.remove(path);
                Ok(None)
            }
        }
    }

    fn insert_lease(&mut self, file_id: u64, path: String, shared: Arc<SharedFile>) {
        self.by_path.insert(path, Arc::downgrade(&shared));
        self.leases.insert(file_id, shared);
    }
}

pub struct RpcServer {
    files: FileStore,
    next_id: Arc<AtomicU64>,
}

impl Default for RpcServer {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcServer {
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(FileStoreState::default())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Read JSON-RPC requests from stdin, write responses to stdout.
    /// One request per line (newline-delimited JSON).
    pub async fn run(&self) -> Result<()> {
        // 4MB per request is already far beyond any realistic JSON-RPC call.
        const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break;
            }

            if line.len() > MAX_REQUEST_BYTES {
                tracing::warn!(bytes = line.len(), "request too large, discarding");
                let err = RpcResponse::error(
                    None,
                    -32700,
                    format!(
                        "request too large ({} bytes, max {})",
                        line.len(),
                        MAX_REQUEST_BYTES
                    ),
                );
                let mut out = serde_json::to_string(&err)?;
                out.push('\n');
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = self.handle(trimmed).await;
            let mut out = serde_json::to_string(&response)?;
            out.push('\n');
            stdout.write_all(out.as_bytes()).await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    async fn handle(&self, raw: &str) -> RpcResponse {
        let req: RpcRequest = match serde_json::from_str(raw) {
            Ok(r) => r,
            Err(e) => return RpcResponse::error(None, -32700, format!("Parse error: {e}")),
        };

        let id = req.id.clone();

        if let Some(v) = req.version {
            if v != PROTOCOL_VERSION {
                tracing::warn!(
                    client_version = v,
                    server_version = PROTOCOL_VERSION,
                    "protocol version mismatch"
                );
            }
        }

        let result = match req.method.as_str() {
            "open" => self.cmd_open(req.params).await,
            "read" => self.cmd_read(req.params).await,
            "search" => self.cmd_search(req.params).await,
            "count" => self.cmd_count(req.params).await,
            "info" => self.cmd_info(req.params).await,
            "close" => self.cmd_close(req.params).await,
            m => Err(anyhow::anyhow!("Unknown method: {m}")),
        };

        match result {
            Ok(data) => RpcResponse::ok(id, data),
            Err(e) => RpcResponse::error(id, -32000, e.to_string()),
        }
    }

    async fn cmd_open(&self, params: Option<Value>) -> Result<Value> {
        let p: OpenParams = from_params(params)?;
        let path = validate_path(&p.path)?;

        if let Some(handle) = {
            let mut files = self.files.write().await;
            files.live_handle_for_path(&path)?
        } {
            return self.register_open_handle(path, handle, false).await;
        }

        let path_for_open = path.clone();
        let built = tokio::task::spawn_blocking(move || SharedFile::open(&path_for_open)).await??;
        let built = Arc::new(built);

        self.register_open_handle(path, built, true).await
    }

    async fn register_open_handle(
        &self,
        path: String,
        candidate: Arc<SharedFile>,
        candidate_was_built: bool,
    ) -> Result<Value> {
        let mut files = self.files.write().await;
        let live = files.live_handle_for_path(&path)?;
        let dedup_hit = live.is_some();
        let built_but_discarded = candidate_was_built && dedup_hit;
        let shared = live.unwrap_or(candidate);
        let total_lines = shared.handle.index.total_lines();
        let file_size = shared.handle.index.file_size();
        let format = shared.handle.format.as_str().to_string();
        let file_id = self.next_id.fetch_add(1, Ordering::Relaxed);

        files.insert_lease(file_id, path.clone(), shared);
        let active_leases = files.leases.len();
        let active_paths = files.by_path.len();
        drop(files);

        tracing::info!(
            file_id,
            %path,
            total_lines,
            file_size,
            dedup_hit,
            built_but_discarded,
            active_leases,
            active_paths,
            "file opened"
        );

        Ok(serde_json::to_value(OpenedData {
            file_id,
            total_lines,
            file_size,
            format,
        })?)
    }

    async fn cmd_read(&self, params: Option<Value>) -> Result<Value> {
        let p: ReadParams = from_params(params)?;

        // Extract path + index data while holding read lock, then drop lock before I/O.
        let (path, byte_offset, total_lines, end, pretty, format) = {
            let files = self.files.read().await;
            let handle = files
                .leases
                .get(&p.file_id)
                .ok_or_else(|| anyhow::anyhow!("File not found: {}", p.file_id))?;
            let total = handle.handle.index.total_lines();
            if p.offset >= total {
                anyhow::bail!(
                    "offset {} out of range (file has {} lines)",
                    p.offset,
                    total
                );
            }
            let byte_off = handle
                .handle
                .index
                .line_offset(p.offset)
                .ok_or_else(|| anyhow::anyhow!("index error at offset {}", p.offset))?;
            let end = (p.offset + p.limit).min(total);
            (
                handle.handle.path.clone(),
                byte_off,
                total,
                end,
                p.pretty.unwrap_or(false),
                handle.handle.format,
            )
        }; // ← lock released here, before any I/O

        let offset = p.offset;
        let lines = tokio::task::spawn_blocking(move || {
            crate::reader::stream::read_lines_direct(
                &path,
                byte_offset,
                offset,
                end,
                pretty,
                format,
            )
        })
        .await??;

        Ok(serde_json::to_value(LinesData {
            lines,
            total_lines,
            offset: p.offset,
        })?)
    }

    async fn cmd_search(&self, params: Option<Value>) -> Result<Value> {
        let p: SearchParams = from_params(params)?;
        let max = p.max_results.unwrap_or(100);
        let before = p.before.unwrap_or(0);
        let after = p.after.unwrap_or(0);
        let use_regex = p.regex.unwrap_or(false);

        // When context is requested we need the shared handle's line index.
        // When no context, use the lighter standalone stream functions.
        let (results, truncated) = if before > 0 || after > 0 {
            let handle = self.extract_handle(p.file_id).await?;
            let query = p.query.clone();
            tokio::task::spawn_blocking(move || {
                handle
                    .handle
                    .search_with_context(&query, max, use_regex, before, after)
            })
            .await??
        } else {
            let path = self.extract_path(p.file_id).await?;
            let query = p.query.clone();
            tokio::task::spawn_blocking(move || {
                if use_regex {
                    crate::reader::stream::stream_search_regex(&path, &query, max)
                } else {
                    crate::reader::stream::stream_search(&path, &query, max)
                }
            })
            .await??
        };
        let total_found = results.len();

        Ok(serde_json::to_value(SearchResultsData {
            results,
            total_found,
            truncated,
        })?)
    }

    async fn cmd_count(&self, params: Option<Value>) -> Result<Value> {
        let p: CountParams = from_params(params)?;
        let path = self.extract_path(p.file_id).await?;
        let query = p.query.clone();
        let use_regex = p.regex.unwrap_or(false);

        let count = tokio::task::spawn_blocking(move || {
            if use_regex {
                crate::reader::stream::stream_count_regex(&path, &query)
            } else {
                crate::reader::stream::stream_count(&path, &query)
            }
        })
        .await??;

        Ok(serde_json::to_value(CountData { count })?)
    }

    /// Extract canonical path for a file_id while holding a read lock, then release.
    /// Used by search/count to avoid holding the lock during long I/O operations.
    async fn extract_path(&self, file_id: u64) -> Result<String> {
        Ok(self.extract_handle(file_id).await?.handle.path.clone())
    }

    async fn extract_handle(&self, file_id: u64) -> Result<Arc<SharedFile>> {
        let files = self.files.read().await;
        let handle = files
            .leases
            .get(&file_id)
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", file_id))?;
        Ok(Arc::clone(handle))
    }

    async fn cmd_info(&self, params: Option<Value>) -> Result<Value> {
        let p: InfoParams = from_params(params)?;
        let files = self.files.read().await;
        let handle = files
            .leases
            .get(&p.file_id)
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", p.file_id))?;

        Ok(serde_json::to_value(InfoData {
            file_id: p.file_id,
            path: handle.handle.path.clone(),
            total_lines: handle.handle.index.total_lines(),
            file_size: handle.handle.index.file_size(),
            format: handle.handle.format.as_str().to_string(),
        })?)
    }

    async fn cmd_close(&self, params: Option<Value>) -> Result<Value> {
        let p: CloseParams = from_params(params)?;
        let mut files = self.files.write().await;
        let found = if let Some(handle) = files.leases.remove(&p.file_id) {
            if Arc::strong_count(&handle) == 1 {
                files.by_path.remove(&handle.handle.path);
            }
            true
        } else {
            false
        };
        let active_leases = files.leases.len();
        let active_paths = files.by_path.len();
        drop(files);

        tracing::info!(
            file_id = p.file_id,
            found,
            active_leases,
            active_paths,
            "file closed"
        );
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    fn temp_file(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
        f
    }

    async fn open(server: &RpcServer, path: &str) -> u64 {
        let value = server
            .cmd_open(Some(json!({ "path": path })))
            .await
            .unwrap();
        value["file_id"].as_u64().unwrap()
    }

    #[tokio::test]
    async fn same_path_gets_unique_ids_sharing_one_handle() {
        let f = temp_file("one\ntwo\n");
        let server = RpcServer::new();
        let path = f.path().to_str().unwrap();

        let id1 = open(&server, path).await;
        let id2 = open(&server, path).await;

        assert_ne!(id1, id2);

        let files = server.files.read().await;
        let h1 = files.leases.get(&id1).unwrap();
        let h2 = files.leases.get(&id2).unwrap();

        assert!(Arc::ptr_eq(h1, h2));
        assert_eq!(files.leases.len(), 2);
        assert_eq!(files.by_path.len(), 1);
    }

    #[tokio::test]
    async fn same_path_after_file_change_gets_distinct_handle() {
        let f = temp_file("one\n");
        let server = RpcServer::new();
        let path = f.path().to_str().unwrap();

        let id1 = open(&server, path).await;
        std::fs::write(path, "one\ntwo\n").unwrap();
        let id2 = open(&server, path).await;

        assert_ne!(id1, id2);

        let files = server.files.read().await;
        let h1 = files.leases.get(&id1).unwrap();
        let h2 = files.leases.get(&id2).unwrap();

        assert!(!Arc::ptr_eq(h1, h2));
        assert_eq!(h1.handle.index.total_lines(), 1);
        assert_eq!(h2.handle.index.total_lines(), 2);
        assert_eq!(files.leases.len(), 2);
        assert_eq!(files.by_path.len(), 1);
    }

    #[tokio::test]
    async fn closing_one_lease_keeps_other_same_path_lease_alive() {
        let f = temp_file("one\ntwo\n");
        let server = RpcServer::new();
        let path = f.path().to_str().unwrap();

        let id1 = open(&server, path).await;
        let id2 = open(&server, path).await;

        server
            .cmd_close(Some(json!({ "file_id": id1 })))
            .await
            .unwrap();

        assert!(server
            .cmd_read(Some(json!({ "file_id": id1, "offset": 0, "limit": 1 })))
            .await
            .is_err());

        let value = server
            .cmd_read(Some(json!({ "file_id": id2, "offset": 1, "limit": 1 })))
            .await
            .unwrap();
        assert_eq!(value["lines"][0]["content"], "two");
    }

    #[tokio::test]
    async fn last_close_removes_path_entry() {
        let f = temp_file("one\n");
        let server = RpcServer::new();
        let path = f.path().to_str().unwrap();
        let canonical = validate_path(path).unwrap();

        let id1 = open(&server, path).await;
        server
            .cmd_close(Some(json!({ "file_id": id1 })))
            .await
            .unwrap();

        {
            let files = server.files.read().await;
            assert!(!files.by_path.contains_key(&canonical));
        }

        let id2 = open(&server, path).await;
        assert_ne!(id1, id2);

        let files = server.files.read().await;
        assert_eq!(files.leases.len(), 1);
        assert!(files.by_path.get(&canonical).unwrap().upgrade().is_some());
    }
}

fn from_params<T: serde::de::DeserializeOwned>(params: Option<Value>) -> Result<T> {
    Ok(serde_json::from_value(params.unwrap_or(Value::Null))?)
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
    pub version: Option<u32>,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcResponse {
    fn ok(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError { code, message }),
        }
    }
}
