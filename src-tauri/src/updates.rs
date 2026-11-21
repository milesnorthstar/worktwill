//! 更新只管理程序制品；业务数据及迁移继续归 Store 所有。
use reqwest::{header, Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use worktwill_core::model::{AppError, Result, UpdatePhase, UpdateResume, UpdateStatus};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const STALL: Duration = Duration::from_secs(30);

#[derive(Serialize, Deserialize)]
struct Preferences {
    automatic_download: bool,
}

struct Inner {
    status: UpdateStatus,
    candidate: Option<Update>,
}

pub struct Updates {
    inner: Mutex<Inner>,
    operation: tokio::sync::Mutex<()>,
    directory: PathBuf,
    configured: bool,
}

fn failure(code: &str, zh: &str, en: &str) -> AppError {
    AppError::new(code, zh, en)
}

fn storage_error(_: impl std::fmt::Display) -> AppError {
    failure(
        "update_storage",
        "更新缓存无法读写，请检查磁盘空间及权限后重试",
        "Cannot access the update cache. Check disk space and permissions, then retry.",
    )
}

fn updater_error(error: impl std::fmt::Display) -> AppError {
    eprintln!("更新检查或校验失败: {error}");
    failure(
        "update_failed",
        "更新检查或签名校验失败，当前版本仍可使用，请重试",
        "Update checking or signature verification failed. Keep using this version and try again.",
    )
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(storage_error)?;
    serde_json::to_writer(&mut file, value).map_err(storage_error)?;
    file.as_file().sync_all().map_err(storage_error)?;
    file.persist(path).map_err(storage_error)?;
    Ok(())
}

fn package_path(directory: &Path, update: &Update) -> PathBuf {
    // 缓存按版本和签名隔离；清单里的 URL/文件名不能成为本地路径。
    let key = Sha256::digest(format!("{}\n{}", update.version, update.signature).as_bytes());
    directory.join(format!("{key:x}.package"))
}

fn read_package(path: &Path) -> Result<Vec<u8>> {
    if fs::metadata(path).map_err(storage_error)?.len() > MAX_BYTES {
        return Err(storage_error("oversized package"));
    }
    fs::read(path).map_err(storage_error)
}

impl Updates {
    pub fn new(directory: PathBuf, version: String, configured: bool) -> Self {
        let preferences = match fs::read(directory.join("preferences.json")) {
            Ok(bytes) => serde_json::from_slice::<Preferences>(&bytes).map_err(storage_error),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Preferences {
                automatic_download: true,
            }),
            Err(error) => Err(storage_error(error)),
        };
        let automatic_download = preferences.as_ref().is_ok_and(|p| p.automatic_download);
        let error = preferences.err();
        Self {
            inner: Mutex::new(Inner {
                status: UpdateStatus {
                    revision: 0,
                    phase: if error.is_some() {
                        UpdatePhase::Error
                    } else if configured {
                        UpdatePhase::Idle
                    } else {
                        UpdatePhase::Unconfigured
                    },
                    current_version: version,
                    version: None,
                    notes: None,
                    downloaded: 0.,
                    total: None,
                    automatic_download,
                    error,
                },
                candidate: None,
            }),
            operation: tokio::sync::Mutex::new(()),
            directory,
            configured,
        }
    }

    pub fn status(&self) -> UpdateStatus {
        self.inner.lock().unwrap().status.clone()
    }

    fn change(&self, app: &tauri::AppHandle, update: impl FnOnce(&mut Inner)) {
        let status = {
            let mut inner = self.inner.lock().unwrap();
            update(&mut inner);
            inner.status.revision += 1;
            inner.status.clone()
        };
        if let Err(error) = app.emit("update-status", status) {
            eprintln!("更新状态通知失败: {error}");
        }
    }

    pub fn set_automatic(&self, app: &tauri::AppHandle, enabled: bool) -> Result<UpdateStatus> {
        fs::create_dir_all(&self.directory).map_err(storage_error)?;
        write_json(
            &self.directory.join("preferences.json"),
            &Preferences {
                automatic_download: enabled,
            },
        )?;
        self.change(app, |inner| inner.status.automatic_download = enabled);
        Ok(self.status())
    }

    async fn check(&self, app: &tauri::AppHandle, explicit_download: bool) -> Result<()> {
        if !self.configured {
            return Err(failure(
                "update_unconfigured",
                "此版本尚未连接更新服务",
                "This build is not connected to an update service yet.",
            ));
        }
        fs::create_dir_all(&self.directory).map_err(storage_error)?;
        // 已校验的缓存优先恢复；断网检查不能抹掉已准备好的更新。
        if self.inner.lock().unwrap().candidate.is_none() {
            if let Err(error) = self.restore(app) {
                // 坏缓存先明确报错并隔离清单，下一次重试才能从可信线上清单恢复。
                let path = self.directory.join("ready.json");
                if path.exists() {
                    fs::remove_file(path).map_err(storage_error)?;
                }
                return Err(error);
            }
        }
        let ready = self.status().phase == UpdatePhase::Ready;
        if !ready {
            self.change(app, |inner| {
                inner.status.phase = UpdatePhase::Checking;
                inner.status.error = None;
            });
        }
        let updater = app
            .updater_builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(updater_error)?;
        let found = updater.check().await.map_err(updater_error)?;
        if let Some(update) = found {
            let cached_version = self
                .inner
                .lock()
                .unwrap()
                .candidate
                .as_ref()
                .map(|old| old.version.clone());
            // CDN旧清单不能把已下载的新版本降回去；仍可安装先前校验的制品。
            if ready
                && cached_version.as_deref().is_some_and(|cached| {
                    semver::Version::parse(cached)
                        .ok()
                        .zip(semver::Version::parse(&update.version).ok())
                        .is_some_and(|(old, new)| old > new)
                })
            {
                return Ok(());
            }
            let same = self
                .inner
                .lock()
                .unwrap()
                .candidate
                .as_ref()
                .is_some_and(|old| {
                    old.version == update.version && old.signature == update.signature
                });
            if ready && same {
                return Ok(());
            }
            self.change(app, |inner| {
                inner.status.phase = UpdatePhase::Available;
                inner.status.version = Some(update.version.clone());
                inner.status.notes = update.body.clone();
                inner.status.error = None;
                inner.status.downloaded = 0.;
                inner.status.total = None;
                inner.candidate = Some(update);
            });
            if explicit_download || self.status().automatic_download {
                self.download(app).await?;
            }
        } else if !ready {
            self.change(app, |inner| {
                inner.status.phase = UpdatePhase::UpToDate;
                inner.status.version = None;
                inner.status.notes = None;
                inner.candidate = None;
                inner.status.error = None;
            });
        }
        Ok(())
    }

    fn restore(&self, app: &tauri::AppHandle) -> Result<()> {
        let path = self.directory.join("ready.json");
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(storage_error(error)),
        };
        let raw = serde_json::from_slice(&bytes).map_err(storage_error)?;
        let updater = app.updater_builder().build().map_err(updater_error)?;
        if let Some(update) = updater.restore_update(raw).map_err(updater_error)? {
            let bytes = read_package(&package_path(&self.directory, &update))?;
            // 官方恢复接口要求版本绑定签名，不能借缓存元数据把旧包冒充新版本。
            update.verify(&bytes).map_err(updater_error)?;
            self.change(app, |inner| {
                inner.status.phase = UpdatePhase::Ready;
                inner.status.version = Some(update.version.clone());
                inner.status.notes = update.body.clone();
                inner.status.downloaded = bytes.len() as f64;
                inner.status.total = Some(bytes.len() as f64);
                inner.candidate = Some(update);
            });
        } else {
            fs::remove_file(path).map_err(storage_error)?;
        }
        Ok(())
    }

    async fn download(&self, app: &tauri::AppHandle) -> Result<()> {
        let update = self
            .inner
            .lock()
            .unwrap()
            .candidate
            .clone()
            .ok_or_else(|| updater_error("no candidate"))?;
        let path = package_path(&self.directory, &update);
        self.change(app, |inner| {
            inner.status.phase = UpdatePhase::Downloading;
            inner.status.error = None;
        });
        let client = Client::builder()
            .https_only(true)
            .connect_timeout(STALL)
            .build()
            .map_err(updater_error)?;
        let mut urls = vec![update.download_url.clone()];
        if let Some(mirrors) = update
            .raw_json
            .get("mirrors")
            .and_then(|value| value.as_array())
        {
            for mirror in mirrors {
                let url = mirror
                    .as_str()
                    .and_then(|value| Url::parse(value).ok())
                    .ok_or_else(|| updater_error("invalid mirror"))?;
                if url.scheme() != "https" {
                    return Err(updater_error("HTTPS mirror required"));
                }
                urls.push(url);
            }
        }
        let mut last = Instant::now() - Duration::from_secs(1);
        download_with_retry(&client, &urls, &path, |done, total| {
            if last.elapsed() >= Duration::from_millis(100) || total == Some(done) {
                last = Instant::now();
                self.change(app, |inner| {
                    inner.status.downloaded = done as f64;
                    inner.status.total = total.map(|value| value as f64);
                });
            }
        })
        .await?;
        let bytes = read_package(&path)?;
        if let Err(error) = update.verify(&bytes) {
            fs::remove_file(&path).map_err(storage_error)?;
            return Err(updater_error(error));
        }
        write_json(&self.directory.join("ready.json"), &update.raw_json)?;
        // 只清理本服务的旧制品，避免每次升级永久累积一个安装包。
        for entry in fs::read_dir(&self.directory).map_err(storage_error)? {
            let entry = entry.map_err(storage_error)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry.path() != path
                && name.strip_suffix(".package").is_some_and(|key| {
                    key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            {
                if let Err(error) = fs::remove_file(entry.path()) {
                    eprintln!("清理旧更新缓存失败: {error}");
                }
            }
        }
        self.change(app, |inner| {
            inner.status.phase = UpdatePhase::Ready;
            inner.status.downloaded = bytes.len() as f64;
            inner.status.total = Some(bytes.len() as f64);
        });
        Ok(())
    }

    pub fn start(app: tauri::AppHandle) {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            loop {
                let service = app.state::<Arc<Self>>().inner().clone();
                if service.configured {
                    service.run(&app, false).await;
                }
                tokio::time::sleep(CHECK_EVERY).await;
            }
        });
    }

    pub async fn run(&self, app: &tauri::AppHandle, explicit_download: bool) -> UpdateStatus {
        let Ok(_operation) = self.operation.try_lock() else {
            return self.status();
        };
        if let Err(error) = self.check(app, explicit_download).await {
            self.change(app, |inner| {
                if inner.status.phase != UpdatePhase::Ready {
                    inner.status.phase = if self.configured {
                        UpdatePhase::Error
                    } else {
                        UpdatePhase::Unconfigured
                    };
                }
                inner.status.error = Some(error);
            });
        }
        self.status()
    }

    pub fn take_resume(&self) -> Result<Option<UpdateResume>> {
        let path = self.directory.join("resume.json");
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(storage_error(error)),
        };
        let resume = serde_json::from_slice(&bytes).map_err(storage_error)?;
        fs::remove_file(path).map_err(storage_error)?;
        Ok(Some(resume))
    }

    pub async fn install(&self, app: &tauri::AppHandle, resume: UpdateResume) -> Result<()> {
        let _operation = self.operation.try_lock().map_err(|_| {
            failure(
                "update_busy",
                "更新正在处理中，请稍候",
                "An update operation is already running.",
            )
        })?;
        let update = {
            let inner = self.inner.lock().unwrap();
            if inner.status.phase != UpdatePhase::Ready {
                return Err(updater_error("update not ready"));
            }
            inner
                .candidate
                .clone()
                .ok_or_else(|| updater_error("update missing"))?
        };
        let bytes = read_package(&package_path(&self.directory, &update))?;
        if ![
            "home",
            "favorites",
            "workspaces",
            "workspace",
            "activity",
            "settings",
        ]
        .contains(&resume.view.as_str())
        {
            return Err(updater_error("invalid resume view"));
        }
        write_json(&self.directory.join("resume.json"), &resume)?;
        self.change(app, |inner| inner.status.phase = UpdatePhase::Installing);
        // 调用前由页面等待最新便签保存；Windows安装器会直接退出当前进程。
        let result = tauri::async_runtime::spawn_blocking(move || update.install(bytes)).await;
        match result {
            Ok(Ok(())) => {
                app.restart();
            }
            other => {
                if let Err(error) = fs::remove_file(self.directory.join("resume.json")) {
                    eprintln!("清理更新返回位置失败: {error}");
                }
                let error = updater_error(format!("{other:?}"));
                self.change(app, |inner| {
                    inner.status.phase = UpdatePhase::Ready;
                    inner.status.error = Some(error.clone());
                });
                Err(error)
            }
        }
    }
}

async fn download_with_retry(
    client: &Client,
    urls: &[Url],
    path: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<()> {
    let mut last = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(attempt)).await;
        }
        let url = &urls[(attempt as usize).min(urls.len() - 1)];
        match transfer(client, url, path, &mut progress).await {
            Ok(()) => return Ok(()),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap())
}

async fn transfer(
    client: &Client,
    url: &Url,
    path: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<()> {
    let have = match fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(storage_error(error)),
    };
    if have > MAX_BYTES {
        fs::remove_file(path).map_err(storage_error)?;
        return Err(storage_error("oversized partial file"));
    }
    let mut request = client
        .get(url.clone())
        .header(header::ACCEPT_ENCODING, "identity");
    if have > 0 {
        request = request.header(header::RANGE, format!("bytes={have}-"));
    }
    let mut response = tokio::time::timeout(STALL, request.send())
        .await
        .map_err(updater_error)?
        .map_err(updater_error)?;
    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        fs::remove_file(path).map_err(storage_error)?;
        return Err(updater_error("range rejected; restarting download"));
    }
    let append = response.status() == StatusCode::PARTIAL_CONTENT && have > 0;
    if append {
        let range = response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !range.starts_with(&format!("bytes {have}-")) {
            return Err(updater_error("invalid Content-Range"));
        }
    } else if response.status() != StatusCode::OK {
        return Err(updater_error(response.status()));
    }
    let mut done = if append { have } else { 0 };
    let total = response
        .content_length()
        .and_then(|length| length.checked_add(done));
    if total.is_some_and(|value| value > MAX_BYTES) {
        return Err(storage_error("oversized remote package"));
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!append)
        .append(append)
        .open(path)
        .map_err(storage_error)?;
    progress(done, total);
    while let Some(chunk) = tokio::time::timeout(STALL, response.chunk())
        .await
        .map_err(updater_error)?
        .map_err(updater_error)?
    {
        done += chunk.len() as u64;
        if done > MAX_BYTES {
            return Err(storage_error("oversized stream"));
        }
        file.write_all(&chunk).map_err(storage_error)?;
        progress(done, total);
    }
    if total.is_some_and(|total| done != total) {
        return Err(updater_error("incomplete body"));
    }
    file.sync_all().map_err(storage_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener};

    #[tokio::test]
    async fn resumes_partial_download_and_restarts_when_server_ignores_range() {
        let root = std::env::var_os("WORKTWILL_TEST_ROOT").unwrap();
        let dir = tempfile::tempdir_in(root).unwrap();
        for resume in [true, false] {
            let path = dir.path().join(format!("{resume}.package"));
            fs::write(&path, b"hello").unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let length = stream.read(&mut request).unwrap();
                assert!(String::from_utf8_lossy(&request[..length])
                    .to_lowercase()
                    .contains("range: bytes=5-"));
                let response = if resume {
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: 6\r\nContent-Range: bytes 5-10/11\r\nConnection: close\r\n\r\n world"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nhello world"
                };
                stream.write_all(response.as_bytes()).unwrap();
            });
            transfer(&Client::new(), &url, &path, |_, _| {})
                .await
                .unwrap();
            server.join().unwrap();
            assert_eq!(fs::read(path).unwrap(), b"hello world");
        }
    }

    #[test]
    fn invalid_preferences_are_visible_and_disable_background_downloads() {
        let dir = tempfile::tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap()).unwrap();
        fs::write(dir.path().join("preferences.json"), b"broken").unwrap();
        let service = Updates::new(dir.path().into(), "1.3.1".into(), true);
        assert_eq!(service.status().phase, UpdatePhase::Error);
        assert!(!service.status().automatic_download);
        assert!(service.status().error.is_some());
    }

    #[test]
    fn cached_update_requires_valid_artifact_and_version_bound_signature() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        let mut context = mock_context(noop_assets());
        context
            .config_mut()
            .plugins
            .0
            .insert("updater".into(), serde_json::json!({"pubkey":""}));
        let app = mock_builder()
            .plugin(
                tauri_plugin_updater::Builder::new()
                    .pubkey(include_str!("../tests/fixtures/updates/public.key").trim())
                    .build(),
            )
            .build(context)
            .unwrap();
        let updater = app
            .updater_builder()
            .target("fixture")
            .endpoints(vec![Url::parse("https://example.com/updates.json").unwrap()])
            .unwrap()
            .executable_path("/tmp/WorkTwill.app/Contents/MacOS/worktwill")
            .build()
            .unwrap();
        let manifest = serde_json::json!({"version":"2.0.0","platforms":{"fixture":{"url":"https://example.com/package","signature":include_str!("../tests/fixtures/updates/payload.sig").trim()}}});
        let update = updater.restore_update(manifest.clone()).unwrap().unwrap();
        assert!(update
            .verify(include_bytes!("../tests/fixtures/updates/payload"))
            .is_ok());
        assert!(update.verify(b"tampered payload").is_err());
        let mut forged = manifest;
        forged["version"] = serde_json::json!("3.0.0");
        let update = updater.restore_update(forged).unwrap().unwrap();
        assert!(update
            .verify(include_bytes!("../tests/fixtures/updates/payload"))
            .is_err());
    }

    #[tokio::test]
    async fn incorrect_range_never_appends_to_partial_file() {
        let dir = tempfile::tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap()).unwrap();
        let path = dir.path().join("partial");
        fs::write(&path, b"hello").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            stream.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 2-4/5\r\nConnection: close\r\n\r\nbad").unwrap();
        });
        assert!(transfer(&Client::new(), &url, &path, |_, _| {})
            .await
            .is_err());
        server.join().unwrap();
        assert_eq!(fs::read(path).unwrap(), b"hello");
    }
}
