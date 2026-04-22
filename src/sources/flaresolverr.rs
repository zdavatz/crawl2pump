//! FlareSolverr client — a Docker-hosted proxy that drives a stealth-patched
//! Chromium specifically tuned to beat Cloudflare's Turnstile / managed
//! challenges.
//!
//! Each call returns the fully rendered HTML post-challenge, which we parse
//! exactly like direct-browser output. If the container isn't running,
//! `ensure_running()` shells out to `docker` to start it (pulling the image
//! on first run). Opt out via `--no-auto-docker` on the CLI.
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const CONTAINER_NAME: &str = "flaresolverr";
pub const IMAGE: &str = "ghcr.io/flaresolverr/flaresolverr:latest";

/// Pinned standalone release — upstream only publishes Linux x64 and
/// Windows x64 binaries, so macOS users must go through Docker.
pub const STANDALONE_VERSION: &str = "v3.4.6";

pub struct FlareSolverrClient {
    endpoint: String,
    http: Client,
}

impl FlareSolverrClient {
    pub fn new(endpoint: impl Into<String>) -> Result<Self> {
        let http = Client::builder()
            // FlareSolverr itself may take 30–60s to solve a tough challenge;
            // give generous headroom before timing out.
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            endpoint: endpoint.into(),
            http,
        })
    }

    /// Best-effort reachability ping. Returns Ok(()) if FlareSolverr answered,
    /// Err with an actionable hint otherwise.
    pub async fn ping(&self) -> Result<()> {
        let req = FsRequest {
            cmd: "sessions.list",
            url: None,
            max_timeout: None,
        };
        let resp = self
            .http
            .post(&self.endpoint)
            .json(&req)
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach FlareSolverr at {}\n\
                     \t  → start it with: docker run -d --name flaresolverr -p 8191:8191 \
                     ghcr.io/flaresolverr/flaresolverr:latest",
                    self.endpoint
                )
            })?;
        if !resp.status().is_success() {
            anyhow::bail!("FlareSolverr replied HTTP {}", resp.status());
        }
        Ok(())
    }

    pub async fn get(&self, url: &str) -> Result<String> {
        let req = FsRequest {
            cmd: "request.get",
            url: Some(url),
            max_timeout: Some(60_000),
        };
        let resp = self
            .http
            .post(&self.endpoint)
            .json(&req)
            .send()
            .await
            .with_context(|| format!("POST {} (is flaresolverr up?)", self.endpoint))?;
        if !resp.status().is_success() {
            anyhow::bail!("flaresolverr HTTP {}", resp.status());
        }
        let body: FsResponse = resp
            .json()
            .await
            .context("parse flaresolverr response")?;
        if body.status != "ok" {
            anyhow::bail!("flaresolverr: {}", body.message);
        }
        let solution = body
            .solution
            .ok_or_else(|| anyhow!("flaresolverr returned no solution"))?;
        Ok(solution.response)
    }
}

/// Best-effort bring-up. Order of attempts:
/// 1. `ping` — already running → done.
/// 2. Docker (any OS, if installed + daemon up).
/// 3. Standalone binary auto-download (Linux x64 / Windows x64 only).
/// 4. Error with OS-appropriate install hint.
pub async fn ensure_running(client: &FlareSolverrClient) -> Result<()> {
    if client.ping().await.is_ok() {
        return Ok(());
    }

    if docker_available() && docker_daemon_up() {
        start_via_docker()?;
        return poll_until_ready(client, Duration::from_secs(120)).await;
    }

    if let Some(platform) = standalone_platform() {
        start_via_standalone(platform).await?;
        return poll_until_ready(client, Duration::from_secs(60)).await;
    }

    Err(anyhow!(
        "FlareSolverr unreachable at {} and no auto-start path available on {}\n\
         \t  → install Docker Desktop (`brew install --cask docker`) or run \
         FlareSolverr manually",
        client.endpoint,
        std::env::consts::OS
    ))
}

async fn poll_until_ready(client: &FlareSolverrClient, budget: Duration) -> Result<()> {
    let deadline = Instant::now() + budget;
    let mut last_err: Option<anyhow::Error> = None;
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(2)).await;
        match client.ping().await {
            Ok(()) => {
                eprintln!("flaresolverr: ready at {}", client.endpoint);
                return Ok(());
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(anyhow!(
        "FlareSolverr started but {} didn't become ready within {:?}: {:?}",
        client.endpoint,
        budget,
        last_err
    ))
}

// --- Docker path ------------------------------------------------------------

fn start_via_docker() -> Result<()> {
    let have_container = docker_inspect_exists(CONTAINER_NAME);
    if have_container {
        eprintln!("flaresolverr: docker container exists, starting");
        run_docker(&["start", CONTAINER_NAME])?;
    } else {
        eprintln!(
            "flaresolverr: pulling image + creating container \
             (first time can take a few minutes — ~300 MB)"
        );
        run_docker(&[
            "run",
            "-d",
            "--name",
            CONTAINER_NAME,
            "-p",
            "8191:8191",
            "--restart=unless-stopped",
            IMAGE,
        ])?;
    }
    Ok(())
}

// --- Standalone binary path -------------------------------------------------

/// Returns the upstream release asset name for the current OS/arch — or
/// `None` on unsupported platforms (including macOS).
fn standalone_platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux_x64"),
        ("windows", "x86_64") => Some("windows_x64"),
        _ => None,
    }
}

async fn start_via_standalone(platform: &str) -> Result<()> {
    let base = std::env::current_dir()?.join(".flaresolverr");
    std::fs::create_dir_all(&base)?;

    let exec_name = if cfg!(windows) {
        "flaresolverr.exe"
    } else {
        "flaresolverr"
    };
    // Upstream archive extracts into a `flaresolverr/` subdir.
    let binary = base.join("flaresolverr").join(exec_name);

    if !binary.exists() {
        let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
        let url = format!(
            "https://github.com/FlareSolverr/FlareSolverr/releases/download/{}/\
             flaresolverr_{}.{}",
            STANDALONE_VERSION, platform, ext
        );
        eprintln!("flaresolverr: downloading standalone binary from {url}");
        download_and_extract(&url, &base).await?;
        if !binary.exists() {
            return Err(anyhow!(
                "downloaded archive did not contain {}",
                binary.display()
            ));
        }
    } else {
        eprintln!("flaresolverr: using cached binary at {}", binary.display());
    }

    eprintln!("flaresolverr: spawning {}", binary.display());
    spawn_detached(&binary)?;
    Ok(())
}

async fn download_and_extract(url: &str, dest: &std::path::Path) -> Result<()> {
    let http = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;
    let bytes = http
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?
        .bytes()
        .await?;
    let archive = dest.join(if cfg!(windows) {
        "_download.zip"
    } else {
        "_download.tar.gz"
    });
    std::fs::write(&archive, &bytes)?;
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(dest)
        .status()
        .context("`tar` not found — install tar (bundled on Linux; Windows 10+ ships tar.exe)")?;
    if !status.success() {
        return Err(anyhow!("tar extraction failed for {}", archive.display()));
    }
    std::fs::remove_file(&archive).ok();
    Ok(())
}

fn spawn_detached(binary: &std::path::Path) -> Result<()> {
    let mut cmd = Command::new(binary);
    cmd.stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());

    // Break away from the parent process group so the child survives after
    // crawl2pump exits — mirrors `nohup ... &` / `start /b ...`.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Safe: setsid has no preconditions; we just become a new session leader.
        unsafe {
            cmd.pre_exec(|| {
                libc_setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()
        .with_context(|| format!("spawn {}", binary.display()))?;
    Ok(())
}

#[cfg(unix)]
fn libc_setsid() {
    // Direct syscall to avoid pulling in the `libc` crate just for this.
    // `setsid()` returns the new session id or -1; we ignore the return —
    // if it fails the child just shares our process group, which is fine.
    extern "C" {
        fn setsid() -> i32;
    }
    unsafe {
        setsid();
    }
}

// --- Docker helpers ---------------------------------------------------------

fn docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn docker_daemon_up() -> bool {
    Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn docker_inspect_exists(name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_docker(args: &[&str]) -> Result<()> {
    let out = Command::new("docker")
        .args(args)
        .output()
        .with_context(|| format!("docker {}", args.join(" ")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("docker {} failed: {}", args.join(" "), stderr.trim()));
    }
    Ok(())
}

#[derive(Serialize)]
struct FsRequest<'a> {
    cmd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(rename = "maxTimeout", skip_serializing_if = "Option::is_none")]
    max_timeout: Option<u64>,
}

#[derive(Deserialize)]
struct FsResponse {
    status: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    solution: Option<FsSolution>,
}

#[derive(Deserialize)]
struct FsSolution {
    #[allow(dead_code)]
    #[serde(default)]
    url: String,
    #[allow(dead_code)]
    #[serde(default)]
    status: i32,
    response: String,
}
