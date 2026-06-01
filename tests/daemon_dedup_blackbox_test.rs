use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::TempDir;

struct Daemon {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Daemon {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_glance"))
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn glance daemon");

        let stdin = child.stdin.take().expect("daemon stdin");
        let stdout = BufReader::new(child.stdout.take().expect("daemon stdout"));

        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn call_response(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "version": 1,
            "method": method,
            "params": params,
        });

        writeln!(self.stdin, "{req}").expect("write request");
        self.stdin.flush().expect("flush request");

        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read response");
        assert!(!line.is_empty(), "daemon closed stdout before responding");

        let response: Value = serde_json::from_str(&line).expect("valid JSON response");
        assert_eq!(response["id"], id);
        response
    }

    fn call_ok(&mut self, method: &str, params: Value) -> Value {
        let response = self.call_response(method, params);
        assert!(
            response.get("error").is_none(),
            "{method} returned error: {response}"
        );
        response["result"].clone()
    }

    fn call_err(&mut self, method: &str, params: Value) -> String {
        let response = self.call_response(method, params);
        response["error"]["message"]
            .as_str()
            .expect("error message")
            .to_string()
    }

    fn open(&mut self, path: &str) -> u64 {
        self.call_ok("open", json!({ "path": path }))["file_id"]
            .as_u64()
            .expect("file_id")
    }

    fn close(&mut self, file_id: u64) {
        self.call_ok("close", json!({ "file_id": file_id }));
    }

    fn read_one(&mut self, file_id: u64, offset: u64) -> String {
        self.call_ok(
            "read",
            json!({ "file_id": file_id, "offset": offset, "limit": 1 }),
        )["lines"][0]["content"]
            .as_str()
            .expect("line content")
            .to_string()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture(contents: &str) -> (TempDir, String) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("events.log");
    std::fs::write(&path, contents).expect("write fixture");
    (dir, path.to_string_lossy().into_owned())
}

#[test]
fn same_path_opens_return_unique_ids_but_close_independently() {
    let (_dir, path) = fixture("alpha\nbeta\ngamma\n");
    let mut daemon = Daemon::start();

    let first = daemon.open(&path);
    let second = daemon.open(&path);

    assert_ne!(first, second);
    assert_eq!(daemon.read_one(first, 0), "alpha");
    assert_eq!(daemon.read_one(second, 1), "beta");

    daemon.close(first);
    assert!(daemon
        .call_err("read", json!({ "file_id": first, "offset": 0, "limit": 1 }))
        .contains("File not found"));
    assert_eq!(daemon.read_one(second, 2), "gamma");

    daemon.close(second);
    assert!(daemon
        .call_err(
            "read",
            json!({ "file_id": second, "offset": 0, "limit": 1 })
        )
        .contains("File not found"));
}

#[test]
fn remaining_lease_still_supports_info_search_and_count() {
    let (_dir, path) = fixture("alpha\nbeta\nalpha beta\n");
    let mut daemon = Daemon::start();

    let first = daemon.open(&path);
    let second = daemon.open(&path);

    daemon.close(first);

    let info = daemon.call_ok("info", json!({ "file_id": second }));
    assert_eq!(info["file_id"], second);
    assert_eq!(info["total_lines"], 3);

    let search = daemon.call_ok(
        "search",
        json!({ "file_id": second, "query": "beta", "max_results": 10 }),
    );
    assert_eq!(search["results"].as_array().expect("results").len(), 2);

    let count = daemon.call_ok("count", json!({ "file_id": second, "query": "alpha" }));
    assert_eq!(count["count"], 2);
}

#[test]
fn close_is_idempotent_for_duplicate_open_lifecycle() {
    let (_dir, path) = fixture("one\ntwo\n");
    let mut daemon = Daemon::start();

    let first = daemon.open(&path);
    let second = daemon.open(&path);

    daemon.close(first);
    daemon.close(first);
    daemon.close(9_999_999);

    assert_eq!(daemon.read_one(second, 1), "two");
}

#[test]
fn opening_again_after_all_closes_returns_a_working_fresh_lease() {
    let (_dir, path) = fixture("before\nafter\n");
    let mut daemon = Daemon::start();

    let first = daemon.open(&path);
    daemon.close(first);
    assert!(daemon
        .call_err("info", json!({ "file_id": first }))
        .contains("File not found"));

    let second = daemon.open(&path);
    assert_ne!(first, second);
    assert_eq!(daemon.read_one(second, 1), "after");
}

#[test]
fn open_after_file_changes_while_old_lease_is_active_gets_fresh_index() {
    let (_dir, path) = fixture("old\n");
    let mut daemon = Daemon::start();

    let old_id = daemon.open(&path);
    let old_info = daemon.call_ok("info", json!({ "file_id": old_id }));
    assert_eq!(old_info["total_lines"], 1);

    std::fs::write(&path, "new\nsecond\n").expect("rewrite fixture");

    let new_id = daemon.open(&path);
    assert_ne!(old_id, new_id);

    let new_info = daemon.call_ok("info", json!({ "file_id": new_id }));
    assert_eq!(new_info["total_lines"], 2);
    assert_eq!(daemon.read_one(new_id, 1), "second");
}

#[cfg(unix)]
#[test]
fn canonical_path_variants_have_unique_ids_and_independent_close() {
    use std::os::unix::fs::symlink;

    let (dir, path) = fixture("real\nlink\n");
    let link = dir.path().join("alias.log");
    symlink(&path, &link).expect("create symlink");
    let link = link.to_string_lossy().into_owned();

    let mut daemon = Daemon::start();
    let real_id = daemon.open(&path);
    let link_id = daemon.open(&link);

    assert_ne!(real_id, link_id);
    assert_eq!(daemon.read_one(real_id, 0), "real");
    assert_eq!(daemon.read_one(link_id, 1), "link");

    daemon.close(real_id);
    assert!(daemon
        .call_err(
            "read",
            json!({ "file_id": real_id, "offset": 0, "limit": 1 })
        )
        .contains("File not found"));
    assert_eq!(daemon.read_one(link_id, 0), "real");
}
