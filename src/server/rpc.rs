use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

const PROTOCOL_VERSION: u32 = 1;

use crate::protocol::request::{CloseParams, CountParams, InfoParams, OpenParams, ReadParams, SearchParams};
use crate::protocol::response::{CountData, InfoData, LinesData, OpenedData, SearchResultsData};
use crate::reader::FileHandle;
use crate::security::validate_path;

// RwLock allows multiple concurrent reads (search, read, count on different files)
// while still serializing writes (open, close).
type FileStore = Arc<RwLock<HashMap<u64, FileHandle>>>;

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
            files: Arc::new(RwLock::new(HashMap::new())),
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
                    format!("request too large ({} bytes, max {})", line.len(), MAX_REQUEST_BYTES),
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
                tracing::warn!(client_version = v, server_version = PROTOCOL_VERSION, "protocol version mismatch");
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
        let path_for_open = path.clone();

        let handle =
            tokio::task::spawn_blocking(move || FileHandle::open(&path_for_open)).await??;

        let total_lines = handle.index.total_lines();
        let file_size = handle.index.file_size();
        let format = handle.format.as_str().to_string();

        let file_id = self.next_id.fetch_add(1, Ordering::Relaxed);

        self.files.write().await.insert(file_id, handle);

        tracing::info!(file_id, %path, total_lines, file_size, "file opened");

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
                .get(&p.file_id)
                .ok_or_else(|| anyhow::anyhow!("File not found: {}", p.file_id))?;
            let total = handle.index.total_lines();
            if p.offset >= total {
                anyhow::bail!("offset {} out of range (file has {} lines)", p.offset, total);
            }
            let byte_off = handle.index.line_offset(p.offset)
                .ok_or_else(|| anyhow::anyhow!("index error at offset {}", p.offset))?;
            let end = (p.offset + p.limit).min(total);
            (handle.path.clone(), byte_off, total, end, p.pretty.unwrap_or(false), handle.format)
        }; // ← lock released here, before any I/O

        let offset = p.offset;
        let lines = tokio::task::spawn_blocking(move || {
            crate::reader::stream::read_lines_direct(&path, byte_offset, offset, end, pretty, format)
        }).await??;

        Ok(serde_json::to_value(LinesData { lines, total_lines, offset: p.offset })?)
    }

    async fn cmd_search(&self, params: Option<Value>) -> Result<Value> {
        let p: SearchParams = from_params(params)?;
        let max = p.max_results.unwrap_or(100);
        let path = self.extract_path(p.file_id).await?;
        let query = p.query.clone();
        let use_regex = p.regex.unwrap_or(false);

        let (results, truncated) = tokio::task::spawn_blocking(move || {
            if use_regex {
                crate::reader::stream::stream_search_regex(&path, &query, max)
            } else {
                crate::reader::stream::stream_search(&path, &query, max)
            }
        }).await??;
        let total_found = results.len();

        Ok(serde_json::to_value(SearchResultsData { results, total_found, truncated })?)
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
        }).await??;

        Ok(serde_json::to_value(CountData { count })?)
    }

    /// Extract canonical path for a file_id while holding a read lock, then release.
    /// Used by search/count to avoid holding the lock during long I/O operations.
    async fn extract_path(&self, file_id: u64) -> Result<String> {
        let files = self.files.read().await;
        let handle = files
            .get(&file_id)
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", file_id))?;
        Ok(handle.path.clone())
    }

    async fn cmd_info(&self, params: Option<Value>) -> Result<Value> {
        let p: InfoParams = from_params(params)?;
        let files = self.files.read().await;
        let handle = files
            .get(&p.file_id)
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", p.file_id))?;

        Ok(serde_json::to_value(InfoData {
            file_id: p.file_id,
            path: handle.path.clone(),
            total_lines: handle.index.total_lines(),
            file_size: handle.index.file_size(),
            format: handle.format.as_str().to_string(),
        })?)
    }

    async fn cmd_close(&self, params: Option<Value>) -> Result<Value> {
        let p: CloseParams = from_params(params)?;
        self.files.write().await.remove(&p.file_id);
        tracing::info!(file_id = p.file_id, "file closed");
        Ok(Value::Null)
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
