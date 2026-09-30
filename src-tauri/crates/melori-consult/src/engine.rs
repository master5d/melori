use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Returns whether a UI request may be forwarded to the local engine.
pub fn is_allowed_engine_path(method: &str, path: &str) -> bool {
    let method = method.trim().to_ascii_uppercase();
    let clean = path.split('?').next().unwrap_or(path);
    if !clean.starts_with('/') || clean.contains("..") || clean.contains('\\') {
        return false;
    }
    let template_item = clean.len() == "/api/templates/t-12345678".len()
        && clean.starts_with("/api/templates/t-")
        && clean[17..].chars().all(|ch| ch.is_ascii_hexdigit());
    let allowed = clean == "/health"
        || clean == "/api/purge-report"
        || (clean == "/api/psych-council/specialists" && method == "GET")
        || (clean == "/api/templates" && matches!(method.as_str(), "GET" | "POST"))
        || (template_item && matches!(method.as_str(), "GET" | "PUT" | "DELETE"))
        || clean == "/api/clients"
        || clean.starts_with("/api/clients/");
    allowed
        && (matches!(method.as_str(), "GET" | "POST" | "DELETE")
            || (method == "PUT" && (clean.starts_with("/api/clients/") || template_item)))
}

pub fn backoff_secs(attempt: u32) -> u64 {
    [1, 2, 4, 8, 16, 30]
        .get(attempt as usize)
        .copied()
        .unwrap_or(30)
}
pub fn pick_free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}
pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS random source unavailable");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub fn health_body_ok(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("ok").and_then(|ok| ok.as_bool()))
        == Some(true)
}
pub fn engine_dir() -> PathBuf {
    std::env::var("MELORI_ENGINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..\\..\\..\\engine"))
}

pub fn engine_env(
    port: u16,
    token: &str,
    base: &str,
    model: &str,
    corpus: &str,
    embed: &str,
) -> Vec<(String, String)> {
    let mut env = vec![
        ("MELORI_ENGINE_TOKEN".into(), token.into()),
        ("MELORI_ENGINE_PORT".into(), port.to_string()),
    ];
    for (key, value) in [
        ("MELORI_LLM_BASE_URL", base),
        ("MELORI_LLM_MODEL", model),
        ("MELORI_CORPUS_DIR", corpus),
        ("MELORI_EMBED_MODEL", embed),
    ] {
        let value = value.trim();
        if !value.is_empty() {
            env.push((key.into(), value.into()));
        }
    }
    env
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EngineStatus {
    pub state: String,
    pub url: Option<String>,
    pub disk_encryption: Option<String>,
}

struct Inner {
    python: String,
    args: Vec<String>,
    dir: PathBuf,
    llm_base_url: String,
    llm_model: String,
    corpus_dir: String,
    embed_model: String,
    token: String,
    url: Option<String>,
    status: EngineStatus,
    child: Option<Child>,
    spawn_count: u32,
    stopping: bool,
    running: bool,
    restart_requested: bool,
}
#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<Mutex<Inner>>,
}

impl Supervisor {
    pub fn new(
        python: impl Into<String>,
        dir: PathBuf,
        llm_base_url: impl Into<String>,
        llm_model: impl Into<String>,
        corpus_dir: impl Into<String>,
        embed_model: impl Into<String>,
    ) -> Self {
        let python = python.into();
        let python = if python.is_empty() {
            let candidate = if cfg!(windows) {
                dir.join(".venv\\Scripts\\python.exe")
            } else {
                dir.join(".venv/bin/python")
            };
            candidate.to_string_lossy().into_owned()
        } else {
            python
        };
        Self::with_token(
            python,
            dir,
            llm_base_url,
            llm_model,
            corpus_dir,
            embed_model,
            new_token(),
        )
    }
    fn with_token(
        python: impl Into<String>,
        dir: PathBuf,
        llm_base_url: impl Into<String>,
        llm_model: impl Into<String>,
        corpus_dir: impl Into<String>,
        embed_model: impl Into<String>,
        token: String,
    ) -> Self {
        Self::with_command_token(
            python,
            vec!["-m".into(), "melori_engine".into()],
            dir,
            llm_base_url,
            llm_model,
            corpus_dir,
            embed_model,
            token,
        )
    }
    fn with_command_token(
        python: impl Into<String>,
        args: Vec<String>,
        dir: PathBuf,
        llm_base_url: impl Into<String>,
        llm_model: impl Into<String>,
        corpus_dir: impl Into<String>,
        embed_model: impl Into<String>,
        token: String,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                python: python.into(),
                args,
                dir,
                llm_base_url: llm_base_url.into(),
                llm_model: llm_model.into(),
                corpus_dir: corpus_dir.into(),
                embed_model: embed_model.into(),
                token,
                url: None,
                status: EngineStatus {
                    state: "down".into(),
                    url: None,
                    disk_encryption: None,
                },
                child: None,
                spawn_count: 0,
                stopping: false,
                running: false,
                restart_requested: false,
            })),
        }
    }
    pub fn new_for_test(python: impl Into<String>) -> Self {
        Self::with_token(
            python,
            engine_dir(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            new_token(),
        )
    }
    pub fn with_command(
        python: impl Into<String>,
        args: Vec<String>,
        dir: PathBuf,
        llm_base_url: impl Into<String>,
        llm_model: impl Into<String>,
        corpus_dir: impl Into<String>,
        embed_model: impl Into<String>,
    ) -> Self {
        Self::with_command_token(
            python,
            args,
            dir,
            llm_base_url,
            llm_model,
            corpus_dir,
            embed_model,
            new_token(),
        )
    }
    pub fn url(&self) -> Option<String> {
        self.inner.lock().unwrap().url.clone()
    }
    pub fn token(&self) -> String {
        self.inner.lock().unwrap().token.clone()
    }
    pub fn spawn_count(&self) -> u32 {
        self.inner.lock().unwrap().spawn_count
    }
    pub fn status(&self) -> EngineStatus {
        self.inner.lock().unwrap().status.clone()
    }
    pub fn start(&self) {
        self.inner.lock().unwrap().running = true;
        let this = self.clone();
        thread::spawn(move || this.supervise());
    }

    pub fn set_llm_config(&self, base: String, model: String, corpus: String, embed: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.llm_base_url = base;
        inner.llm_model = model;
        inner.corpus_dir = corpus;
        inner.embed_model = embed;
    }

    pub fn restart(&self) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.running || inner.stopping {
            return;
        }
        inner.restart_requested = true;
        drop(inner);
        self.kill_child();
    }

    pub fn start_once_blocking(&self) {
        self.kill_child();
        let (port, token, python, dir, base, model, corpus, embed) = {
            let mut inner = self.inner.lock().unwrap();
            if inner.stopping {
                return;
            }
            let port = match pick_free_port() {
                Ok(port) => port,
                Err(_) => {
                    inner.status.state = "down".into();
                    return;
                }
            };
            let url = format!("http://127.0.0.1:{port}");
            inner.url = Some(url.clone());
            inner.status.url = Some(url);
            inner.status.state = "starting".into();
            (
                port,
                inner.token.clone(),
                inner.python.clone(),
                inner.dir.clone(),
                inner.llm_base_url.clone(),
                inner.llm_model.clone(),
                inner.corpus_dir.clone(),
                inner.embed_model.clone(),
            )
        };
        let child = {
            let inner = self.inner.lock().unwrap();
            spawn_child(
                &python,
                &inner.args,
                &dir,
                port,
                &token,
                &base,
                &model,
                &corpus,
                &embed,
            )
        };
        let child = match child {
            Ok(child) => child,
            Err(_) => {
                let mut inner = self.inner.lock().unwrap();
                inner.status.state = "down".into();
                inner.url = None;
                inner.status.url = None;
                return;
            }
        };
        let mut inner = self.inner.lock().unwrap();
        // the engine dies with the app however the app ends (crash, hard kill)
        if let Some(job) = engine_job() {
            if let Err(e) = job.assign(&child) {
                eprintln!("melori engine: could not tie the engine to the app's lifetime: {e}");
            }
        }
        inner.child = Some(child);
        inner.spawn_count = inner.spawn_count.saturating_add(1);
    }
    fn supervise(&self) {
        let mut attempt = 0;
        loop {
            if self.inner.lock().unwrap().stopping {
                break;
            }
            self.start_once_blocking();
            let deadline = Instant::now() + Duration::from_secs(15);
            let ready = loop {
                if self.inner.lock().unwrap().stopping {
                    return;
                }
                if self.health() {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                thread::sleep(Duration::from_millis(100));
            };
            if ready {
                self.inner.lock().unwrap().status.state = "ready".into();
                attempt = 0;
                self.wait_for_child_exit();
            } else {
                self.kill_child();
            }
            if self.inner.lock().unwrap().stopping {
                break;
            }
            if self.inner.lock().unwrap().restart_requested {
                self.inner.lock().unwrap().restart_requested = false;
                attempt = 0;
                continue;
            }
            thread::sleep(Duration::from_secs(backoff_secs(attempt)));
            attempt = attempt.saturating_add(1);
        }
    }
    fn wait_for_child_exit(&self) {
        loop {
            if self.inner.lock().unwrap().stopping {
                return;
            }
            let exited = {
                let mut inner = self.inner.lock().unwrap();
                let Some(child) = inner.child.as_mut() else {
                    return;
                };
                let result = child.try_wait();
                match result {
                    Ok(Some(_)) | Err(_) => {
                        let _ = inner.child.take();
                        inner.status.state = "down".into();
                        inner.url = None;
                        inner.status.url = None;
                        true
                    }
                    Ok(None) => false,
                }
            };
            if exited {
                return;
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
    fn health(&self) -> bool {
        let (url, token) = {
            let inner = self.inner.lock().unwrap();
            (inner.url.clone(), inner.token.clone())
        };
        let Some(url) = url else { return false };
        let Some(port) = url
            .rsplit(':')
            .next()
            .and_then(|part| part.parse::<u16>().ok())
        else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
        let request = format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Melori-Token: {token}\r\nConnection: close\r\n\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut response = String::new();
        if stream.read_to_string(&mut response).is_err() {
            return false;
        }
        let Some((_, body)) = response.split_once("\r\n\r\n") else {
            return false;
        };
        if !response.starts_with("HTTP/1.1 200") || !health_body_ok(body) {
            return false;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
            let mut inner = self.inner.lock().unwrap();
            inner.status.disk_encryption = value
                .get("disk_encryption")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        true
    }
    fn kill_child(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(mut child) = inner.child.take() {
            kill_tree(&child);
            let _ = child.kill();
            let _ = child.wait();
        }
        inner.status.state = "down".into();
        inner.url = None;
        inner.status.url = None;
    }
    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.stopping = true;
        inner.running = false;
        drop(inner);
        self.kill_child();
    }
}

/// On Windows a venv `python.exe` is a launcher: the engine runs as its child, and
/// `Child::kill` would leave that child serving. Take the whole tree down by our own PID.
#[cfg(windows)]
fn kill_tree(child: &Child) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    if let Err(e) = status {
        eprintln!(
            "melori engine: taskkill of process tree {} failed: {e}",
            child.id()
        );
    }
}

/// Elsewhere the venv interpreter is the engine process itself.
#[cfg(not(windows))]
fn kill_tree(_child: &Child) {}

/// One kill-on-close job for every engine this app process starts.
fn engine_job() -> Option<&'static crate::job::KillOnCloseJob> {
    static JOB: std::sync::OnceLock<Option<crate::job::KillOnCloseJob>> =
        std::sync::OnceLock::new();
    JOB.get_or_init(|| match crate::job::KillOnCloseJob::new() {
        Ok(job) => Some(job),
        Err(e) => {
            eprintln!("melori engine: kill-on-close job unavailable, a hard kill may orphan the engine: {e}");
            None
        }
    })
    .as_ref()
}

fn spawn_child(
    python: &str,
    args: &[String],
    dir: &PathBuf,
    port: u16,
    token: &str,
    base: &str,
    model: &str,
    corpus: &str,
    embed: &str,
) -> std::io::Result<Child> {
    let mut command = Command::new(python);
    command
        .args(args)
        .current_dir(dir)
        .env("MELORI_ENGINE_TOKEN", token)
        .env("MELORI_ENGINE_PORT", port.to_string())
        .env_remove("MELORI_LLM_BASE_URL")
        .env_remove("MELORI_LLM_MODEL")
        .env_remove("MELORI_CORPUS_DIR")
        .env_remove("MELORI_EMBED_MODEL")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in engine_env(port, token, base, model, corpus, embed) {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    fn process_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn fake_engine() -> (String, PathBuf) {
        (
            "python".into(),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fake_engine.py"),
        )
    }

    fn wait_for_state(manager: &Supervisor, state: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if manager.status().state == state {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for state {state}, got {:?}",
            manager.status()
        );
    }
    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(
            (0..8).map(backoff_secs).collect::<Vec<_>>(),
            vec![1, 2, 4, 8, 16, 30, 30, 30]
        );
    }
    #[test]
    fn free_port_is_bindable() {
        let p = pick_free_port().unwrap();
        assert!(p > 0);
        TcpListener::bind(("127.0.0.1", p)).unwrap();
    }
    #[test]
    fn token_is_32_hex() {
        let a = new_token();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, new_token());
    }
    #[test]
    fn health_ok_requires_exact_json_ok_true() {
        assert!(health_body_ok(r#"{"ok":true,"version":"0.1.0"}"#));
        assert!(!health_body_ok(r#"{"detail":"bad token"}"#));
        assert!(!health_body_ok("not json"));
    }

    #[test]
    fn engine_request_allowlist_rejects_path_outside_allowlist() {
        assert!(!is_allowed_engine_path("GET", "/internal/config"));
        assert!(!is_allowed_engine_path("POST", "/api/clients/../secret"));
    }

    #[test]
    fn engine_request_allowlist_accepts_client_route() {
        assert!(is_allowed_engine_path("GET", "/api/clients/anna"));
        assert!(is_allowed_engine_path("DELETE", "/api/clients/anna"));
        assert!(is_allowed_engine_path("PUT", "/api/clients/anna"));
    }

    #[test]
    fn engine_request_allowlist_accepts_template_routes_and_rejects_other_methods() {
        assert!(is_allowed_engine_path("GET", "/api/templates"));
        assert!(is_allowed_engine_path("POST", "/api/templates"));
        assert!(is_allowed_engine_path("PUT", "/api/templates/t-1a2b3c4d"));
        assert!(is_allowed_engine_path(
            "DELETE",
            "/api/templates/t-1a2b3c4d"
        ));
        assert!(!is_allowed_engine_path("PUT", "/api/psych-council/stream"));
        assert!(!is_allowed_engine_path("PATCH", "/api/templates"));
        assert!(!is_allowed_engine_path("GET", "/api/templates/../x"));
    }
    #[test]
    fn supervisor_reports_down_when_python_missing() {
        let manager = Supervisor::new_for_test("definitely-not-a-python-binary-xyz");
        manager.start_once_blocking();
        assert_eq!(manager.status().state, "down");
    }

    #[test]
    fn engine_env_skips_empty_settings() {
        let env = engine_env(1234, "token", "  ", "", "  ", "");
        assert_eq!(
            env,
            vec![
                ("MELORI_ENGINE_TOKEN".into(), "token".into()),
                ("MELORI_ENGINE_PORT".into(), "1234".into())
            ]
        );

        let env = engine_env(
            4321,
            "token",
            " http://llm ",
            " model ",
            " corpus ",
            " bge-m3 ",
        );
        assert_eq!(
            env,
            vec![
                ("MELORI_ENGINE_TOKEN".into(), "token".into()),
                ("MELORI_ENGINE_PORT".into(), "4321".into()),
                ("MELORI_LLM_BASE_URL".into(), "http://llm".into()),
                ("MELORI_LLM_MODEL".into(), "model".into()),
                ("MELORI_CORPUS_DIR".into(), "corpus".into()),
                ("MELORI_EMBED_MODEL".into(), "bge-m3".into()),
            ]
        );
    }

    #[test]
    fn engine_env_passes_embed_model() {
        let env = engine_env(4321, "token", "base", "model", "corpus", "bge-m3");
        assert!(env.contains(&("MELORI_EMBED_MODEL".into(), "bge-m3".into())));
        let env = engine_env(4321, "token", "base", "model", "corpus", "");
        assert!(!env.iter().any(|(key, _)| key == "MELORI_EMBED_MODEL"));
    }

    #[test]
    fn supervisor_keeps_exactly_one_engine() {
        let _guard = process_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::remove_var("FAKE_EXIT_AFTER");
        std::env::remove_var("FAKE_DELAY_START");
        let (python, script) = fake_engine();
        let manager = Supervisor::with_command(
            python,
            vec![script.to_string_lossy().into_owned()],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            "",
            "",
            "",
            "",
        );
        manager.start();
        wait_for_state(&manager, "ready", Duration::from_secs(15));
        thread::sleep(Duration::from_secs(4));
        assert_eq!(manager.spawn_count(), 1);
        assert_eq!(manager.status().state, "ready");
        manager.shutdown();
        wait_for_state(&manager, "down", Duration::from_secs(2));
    }

    #[test]
    fn supervisor_restarts_after_engine_exits() {
        let _guard = process_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("FAKE_EXIT_AFTER", "2");
        std::env::remove_var("FAKE_DELAY_START");
        let (python, script) = fake_engine();
        let manager = Supervisor::with_command(
            python,
            vec![script.to_string_lossy().into_owned()],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            "",
            "",
            "",
            "",
        );
        manager.start();
        wait_for_state(&manager, "ready", Duration::from_secs(15));
        let deadline = Instant::now() + Duration::from_secs(25);
        while manager.spawn_count() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        assert!(manager.spawn_count() >= 2);
        manager.shutdown();
        wait_for_state(&manager, "down", Duration::from_secs(2));
        std::env::remove_var("FAKE_EXIT_AFTER");
    }

    /// A venv `python.exe` on Windows is a launcher that runs the real interpreter as its
    /// child: killing only the launcher left the engine serving on its port after quit/restart.
    #[test]
    fn shutdown_stops_the_engine_behind_a_launcher_process() {
        let _guard = process_test_lock()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("FAKE_EXIT_AFTER");
        std::env::remove_var("FAKE_DELAY_START");
        let (python, script) = fake_engine();
        let launcher =
            "import subprocess, sys; sys.exit(subprocess.call([sys.executable, sys.argv[1]]))";
        let manager = Supervisor::with_command(
            python,
            vec![
                "-c".into(),
                launcher.into(),
                script.to_string_lossy().into_owned(),
            ],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            "base",
            "model",
            "corpus",
            "",
        );
        manager.start();
        wait_for_state(&manager, "ready", Duration::from_secs(15));
        let url = manager.url().expect("ready engine has a url");
        let addr = url.trim_start_matches("http://").to_string();
        manager.shutdown();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut still_serving = true;
        while Instant::now() < deadline {
            still_serving = TcpStream::connect(&addr).is_ok();
            if !still_serving {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
        assert!(
            !still_serving,
            "engine behind the launcher still serves on {addr} after shutdown"
        );
    }

    #[test]
    fn the_lens_list_is_readable_but_the_stream_stays_behind_the_council_command() {
        assert!(is_allowed_engine_path(
            "GET",
            "/api/psych-council/specialists"
        ));
        assert!(!is_allowed_engine_path(
            "POST",
            "/api/psych-council/specialists"
        ));
        assert!(!is_allowed_engine_path("POST", "/api/psych-council/stream"));
    }

    #[test]
    fn restart_uses_new_config() {
        let _guard = process_test_lock()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("FAKE_EXIT_AFTER");
        std::env::remove_var("FAKE_DELAY_START");
        let dump = std::env::temp_dir().join(format!("melori-fake-env-{}", new_token()));
        std::env::set_var("FAKE_ENV_DUMP", &dump);
        let (python, script) = fake_engine();
        let manager = Supervisor::with_command(
            python,
            vec![script.to_string_lossy().into_owned()],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            "base",
            "model-a",
            "corpus",
            "",
        );
        manager.start();
        wait_for_state(&manager, "ready", Duration::from_secs(15));
        manager.set_llm_config("base".into(), "model-b".into(), "corpus".into(), "".into());
        manager.restart();
        let deadline = Instant::now() + Duration::from_secs(20);
        while (manager.spawn_count() < 2 || manager.status().state != "ready")
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(100));
        }
        let contents = std::fs::read_to_string(&dump).unwrap();
        assert_eq!(contents.lines().last(), Some("model-b"));
        manager.shutdown();
        let _ = std::fs::remove_file(dump);
        std::env::remove_var("FAKE_ENV_DUMP");
    }

    #[test]
    fn status_is_starting_while_waiting_for_health() {
        let _guard = process_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::remove_var("FAKE_EXIT_AFTER");
        std::env::set_var("FAKE_DELAY_START", "3");
        let (python, script) = fake_engine();
        let manager = Supervisor::with_command(
            python,
            vec![script.to_string_lossy().into_owned()],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            "",
            "",
            "",
            "",
        );
        manager.start();
        thread::sleep(Duration::from_millis(700));
        assert_eq!(manager.status().state, "starting");
        manager.shutdown();
        wait_for_state(&manager, "down", Duration::from_secs(2));
        std::env::remove_var("FAKE_DELAY_START");
    }
}
