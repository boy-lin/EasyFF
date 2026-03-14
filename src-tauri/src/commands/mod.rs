use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::command;
use tauri::AppHandle;
use tauri::{Emitter, Manager};

fn detect_host_os() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
}

async fn run_blocking<T, F>(ctx: &'static str, job: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| format!("[JOIN:{}] {}", ctx, e))?
}

fn parse_ffmpeg_semver(version_line: &str) -> String {
    let mut parts = version_line.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "version" {
            return parts
                .next()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| version_line.trim().to_string());
        }
    }
    version_line.trim().to_string()
}

fn probe_ffmpeg_binary(bin: &str) -> Option<(String, String)> {
    let output = Command::new(bin).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?.trim();
    if first_line.is_empty() {
        return None;
    }
    Some((first_line.to_string(), bin.to_string()))
}

fn probe_global_ffmpeg() -> Option<(String, String)> {
    let mut candidates: Vec<&str> = Vec::new();
    if cfg!(target_os = "windows") {
        candidates.push("ffmpeg.exe");
    }
    candidates.push("ffmpeg");

    for bin in candidates {
        if let Some(result) = probe_ffmpeg_binary(bin) {
            return Some(result);
        }
    }
    None
}

async fn list_installed_ffmpeg_versions_with_system(
) -> Result<Vec<crate::storage::ffmpeg_versions::FfmpegVersionItem>, String> {
    let mut installed = crate::storage::ffmpeg_versions::list_installed()
        .await
        .map_err(|e| e.to_string())?;

    let has_active = installed.iter().any(|item| item.is_active);
    let has_system_row = installed
        .iter()
        .any(|item| item.row_key == "__system_ffmpeg__");

    let system_probe = tauri::async_runtime::spawn_blocking(probe_global_ffmpeg)
        .await
        .map_err(|e| format!("[JOIN:list_installed_ffmpeg_versions] {}", e))?;

    if !has_system_row {
        if let Some((display_version, executable_path)) = system_probe {
            installed.push(crate::storage::ffmpeg_versions::FfmpegVersionItem {
                row_key: "__system_ffmpeg__".to_string(),
                source: "System".to_string(),
                os: detect_host_os(),
                version: parse_ffmpeg_semver(display_version.as_str()),
                published_at: None,
                download_url: None,
                arch: Some(std::env::consts::ARCH.to_string()),
                local_path: Some(executable_path),
                updated_at: 0,
                download_state: "downloaded".to_string(),
                installed: true,
                is_active: !has_active,
            });
        }
    }

    Ok(installed)
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ClientLogInput {
    pub level: String,
    pub category: String,
    pub message: String,
    pub stack: Option<String>,
    pub url: Option<String>,
    pub meta: Option<serde_json::Value>,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MediaDetails {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MediaProbeResult {
    pub path: Option<String>,
}

pub type MediaTaskRequest = crate::task::queue::MediaTaskRequest;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SelfCheckResult {
    pub fs_permission: bool,
    pub fs_error: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthExchangeCodeInput {
    pub token_endpoint: String,
    pub client_id: String,
    pub code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: Option<String>,
    pub id_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct UpdaterGuardStatus {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TaskHistoryItem {
    pub id: Option<String>,
}


fn collect_files_recursive(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[command]
pub async fn report_client_log(log: ClientLogInput) -> Result<(), String> {
    let level = log.level.to_lowercase();
    let prefix = format!("[CLIENT:{}] {}", log.category, log.message);
    let detail = format!(
        "{} | url={} | ts={} | stack={} | meta={}",
        prefix,
        log.url.unwrap_or_default(),
        log.timestamp.unwrap_or_default(),
        log.stack.unwrap_or_default(),
        log.meta.map(|m| m.to_string()).unwrap_or_default()
    );
    match level.as_str() {
        "warn" => log::warn!("{}", detail),
        "info" => log::info!("{}", detail),
        _ => log::error!("{}", detail),
    }
    Ok(())
}

#[command]
pub async fn export_logs_archive(app: AppHandle) -> Result<String, String> {
    run_blocking("export_logs_archive", move || {
        let log_dir = app
            .path()
            .app_log_dir()
            .map_err(|e| format!("resolve app_log_dir failed: {}", e))?;
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("create app_log_dir failed: {}", e))?;

        let mut files = Vec::new();
        collect_files_recursive(&log_dir, &mut files)
            .map_err(|e| format!("collect logs failed: {}", e))?;
        if files.is_empty() {
            return Err("no log files found".to_string());
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let zip_path = std::env::temp_dir().join(format!("viko-logs-{}.zip", ts));
        let file = File::create(&zip_path)
            .map_err(|e| format!("create zip failed: {}", e))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for src in files {
            let rel = src
                .strip_prefix(&log_dir)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.replace('\\', "/"))
                .unwrap_or_else(|| {
                    src.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("log.txt")
                        .to_string()
                });
            zip.start_file(rel, options)
                .map_err(|e| format!("zip start_file failed: {}", e))?;
            let mut input = File::open(&src)
                .map_err(|e| format!("open log file failed ({}): {}", src.display(), e))?;
            std::io::copy(&mut input, &mut zip)
                .map_err(|e| format!("zip write failed ({}): {}", src.display(), e))?;
        }
        zip.finish()
            .map_err(|e| format!("zip finish failed: {}", e))?;

        Ok(zip_path.to_string_lossy().to_string())
    })
    .await
}

#[command]
pub async fn cli_task_submit(
    app: AppHandle,
    tasks: Vec<MediaTaskRequest>,
    _priority: Option<String>,
) -> Result<usize, String> {
    crate::task::queue::submit_tasks(app, tasks).await
}
#[command]
pub async fn media_task_has_running_by_type(task_type: Option<String>) -> Result<bool, String> {
    Ok(crate::task::queue::has_running(task_type).await)
}
#[command]
pub async fn media_task_clear_by_type(task_type: Option<String>) -> Result<usize, String> {
    crate::task::queue::clear_pending(task_type).await
}
#[command]
pub async fn media_task_clear_by_type_with_stop(
    task_type: Option<String>,
    stop_running: Option<bool>,
) -> Result<usize, String> {
    crate::task::queue::clear_pending_with_cancel(task_type, stop_running.unwrap_or(false)).await
}
#[command]
pub async fn media_task_cancel_task(id: String) -> Result<(), String> {
    crate::task::queue::cancel_task(id).await
}
#[command]
pub async fn get_device_id() -> Result<String, String> {
    machine_uid::get().map_err(|e| format!("failed to get device id: {}", e))
}
#[command]
pub async fn auth_exchange_code(input: AuthExchangeCodeInput) -> Result<AuthTokenResponse, String> {
    let token_endpoint = input.token_endpoint.trim().to_string();
    let client_id = input.client_id.trim().to_string();
    let code = input.code.trim().to_string();
    let code_verifier = input.code_verifier.trim().to_string();
    let redirect_uri = input.redirect_uri.trim().to_string();

    if token_endpoint.is_empty() {
        return Err("token_endpoint is required".to_string());
    }
    if client_id.is_empty() {
        return Err("client_id is required".to_string());
    }
    if code.is_empty() {
        return Err("code is required".to_string());
    }
    if code_verifier.is_empty() {
        return Err("code_verifier is required".to_string());
    }
    if redirect_uri.is_empty() {
        return Err("redirect_uri is required".to_string());
    }

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("failed to create http client: {}", e))?;

    let response = client
        .post(token_endpoint.as_str())
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "code": code,
            "code_verifier": code_verifier,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await
        .map_err(|e| format!("token exchange request failed: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            let err = v
                .get("error_description")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("error").and_then(|x| x.as_str()))
                .unwrap_or("token endpoint returned non-success status");
            return Err(format!("token exchange failed: {} ({})", err, status));
        }
        return Err(format!("token exchange failed: http {} {}", status, body));
    }

    let value = serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| format!("failed to parse token response json: {}; body={}", e, body))?;

    log::info!("auth_exchange_code token response shape: {}", value);

    if let Ok(token) = serde_json::from_value::<AuthTokenResponse>(value.clone()) {
        if !token.access_token.trim().is_empty() {
            return Ok(token);
        }
    }

    if let Some(data) = value.get("data") {
        let token = serde_json::from_value::<AuthTokenResponse>(data.clone()).map_err(|e| {
            format!(
                "failed to parse token response.data: {}; response={}",
                e, value
            )
        })?;
        if !token.access_token.trim().is_empty() {
            return Ok(token);
        }
    }

    Err(format!(
        "failed to parse token response: missing access_token; response={}",
        value
    ))
}


#[command]
pub async fn updater_guard_report_success() -> Result<crate::storage::updater_guard::UpdaterGuardStatus, String> {
    crate::storage::updater_guard::record_success()
        .await
        .map_err(|e| e.to_string())?;
    crate::storage::updater_guard::get_status()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn updater_guard_report_failure(
    reason: Option<String>,
) -> Result<crate::storage::updater_guard::UpdaterGuardStatus, String> {
    crate::storage::updater_guard::record_failure(reason)
        .await
        .map_err(|e| e.to_string())?;
    crate::storage::updater_guard::get_status()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn updater_guard_get_status() -> Result<crate::storage::updater_guard::UpdaterGuardStatus, String> {
    crate::storage::updater_guard::get_status()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn updater_guard_reset() -> Result<(), String> {
    crate::storage::updater_guard::reset_failures()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_task_history(
    limit: Option<u32>,
    offset: Option<u32>,
    task_type: Option<String>,
    keyword: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<Vec<crate::storage::task_history::TaskHistoryItem>, String> {
    crate::storage::task_history::get_history(
        limit.unwrap_or(50).clamp(1, 500) as usize,
        offset.unwrap_or(0) as usize,
        task_type,
        keyword,
        sort_by,
        sort_order,
    )
    .await
    .map_err(|e| e.to_string())
}
#[command]
pub async fn get_my_files(
    limit: Option<u32>,
    offset: Option<u32>,
    keyword: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    media_type: Option<String>,
) -> Result<Vec<crate::storage::task_history::MyFileItem>, String> {
    crate::storage::task_history::get_my_files(
        limit.unwrap_or(50).clamp(1, 500) as usize,
        offset.unwrap_or(0) as usize,
        keyword,
        sort_by,
        sort_order,
        media_type,
    )
    .await
    .map_err(|e| e.to_string())
}
#[command]
pub async fn delete_task_history(id: String) -> Result<(), String> {
    let clean = id.trim();
    if clean.is_empty() {
        return Err("id is required".to_string());
    }
    crate::storage::task_history::delete_history(clean)
        .await
        .map_err(|e| e.to_string())
}
#[command]
pub async fn clear_task_history(task_type: Option<String>) -> Result<(), String> {
    crate::storage::task_history::clear_history(task_type)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegVersionQuery {
    pub os: Option<String>,
    pub arch: Option<String>,
    pub keyword: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteCommandInput {
    pub title: String,
    pub description: Option<String>,
    pub command: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub include_deleted: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncCursorInput {
    pub cursor: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncAckInput {
    pub acks: Vec<crate::storage::favorite_commands::FavoriteCommandSyncAck>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncApplyRemoteInput {
    pub items: Vec<crate::storage::favorite_commands::FavoriteCommandItem>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegRefreshResult {
    pub os: String,
    pub source: String,
    pub fetched: usize,
    pub updated: usize,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegDownloadProgressEvent {
    pub row_key: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: f64,
    pub status: String,
}

static CANCELED_FFMPEG_DOWNLOADS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

fn sanitize_file_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "ffmpeg_download.bin".to_string()
    } else {
        out
    }
}

fn infer_download_name(item: &crate::storage::ffmpeg_versions::FfmpegVersionItem) -> String {
    if let Some(url) = item.download_url.as_ref() {
        if let Some(last) = url.split('/').last() {
            let base = last.split('?').next().unwrap_or(last);
            if !base.is_empty() {
                return sanitize_file_name(base);
            }
        }
    }
    sanitize_file_name(
        format!(
            "ffmpeg-{}-{}-{}.bin",
            item.source,
            item.version,
            item.arch.clone().unwrap_or_else(|| "unknown".to_string())
        )
        .as_str(),
    )
}

fn sanitize_dir_name(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "ffmpeg".to_string()
    } else {
        out
    }
}

fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(enclosed) = entry.enclosed_name().map(|v| v.to_path_buf()) else {
            continue;
        };
        let outpath = target_dir.join(enclosed);
        if entry.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_tar_xz(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(target_dir).map_err(|e| e.to_string())
}

fn extract_gz(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let binary_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let output_path = target_dir.join(binary_name);
    let mut output = File::create(&output_path).map_err(|e| e.to_string())?;
    std::io::copy(&mut decoder, &mut output).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(&output_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&output_path, perms).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn extract_7z_with_system(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    sevenz_rust::decompress_file(archive_path, target_dir).map_err(|e| {
        format!(
            "7z extraction failed: {}. try a zip/tar.xz source",
            e
        )
    })
}

fn find_ffmpeg_binary(root: &Path) -> Option<PathBuf> {
    let target = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if name.eq_ignore_ascii_case(target) {
                return Some(path);
            }
        }
    }
    None
}

fn resolve_ffmpeg_binary_from_download(
    archive_path: &Path,
    base_dir: &Path,
    row_key: &str,
) -> Result<PathBuf, String> {
    let file_name = archive_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if cfg!(target_os = "windows")
        && archive_path
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| v.eq_ignore_ascii_case("exe"))
            .unwrap_or(false)
    {
        return Ok(archive_path.to_path_buf());
    }

    let mut extract_dir = base_dir.join("extracted");
    extract_dir.push(sanitize_dir_name(row_key));
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;

    if file_name.ends_with(".zip") {
        extract_zip(archive_path, &extract_dir)?;
    } else if file_name.ends_with(".tar.xz") || file_name.ends_with(".txz") {
        extract_tar_xz(archive_path, &extract_dir)?;
    } else if file_name.ends_with(".gz") {
        extract_gz(archive_path, &extract_dir)?;
    } else if file_name.ends_with(".7z") {
        extract_7z_with_system(archive_path, &extract_dir)?;
    } else {
        return Err(format!("unsupported archive format: {}", file_name));
    }

    find_ffmpeg_binary(&extract_dir)
        .ok_or_else(|| "ffmpeg executable not found after extraction".to_string())
}

#[command]
pub async fn refresh_ffmpeg_versions(
    source: Option<String>,
    os: Option<String>,
    _arch: Option<String>,
) -> Result<Vec<FfmpegRefreshResult>, String> {
    let updated = crate::storage::ffmpeg_versions::seed_static_data()
        .await
        .map_err(|e| e.to_string())?;
    Ok(vec![FfmpegRefreshResult {
        os: os.unwrap_or_else(|| "all".to_string()),
        source: source.unwrap_or_else(|| "all".to_string()),
        fetched: updated,
        updated,
        message: "Seeded ffmpeg_versions from static sources".to_string(),
    }])
}

#[command]
pub async fn list_ffmpeg_versions(
    query: Option<FfmpegVersionQuery>,
) -> Result<crate::storage::ffmpeg_versions::FfmpegVersionListResult, String> {
    fn detect_host_arch() -> String {
        match std::env::consts::ARCH.to_ascii_lowercase().as_str() {
            "x86_64" | "amd64" => "x86_64".to_string(),
            "aarch64" | "arm64" => "arm64".to_string(),
            "x86" | "i386" | "i686" => "x86".to_string(),
            other => other.to_string(),
        }
    }

    let q = query.unwrap_or(FfmpegVersionQuery {
        os: None,
        arch: None,
        keyword: None,
        limit: Some(20),
        offset: Some(0),
    });

    let arch = q.arch.or_else(|| Some(detect_host_arch()));
    crate::storage::ffmpeg_versions::list(
        None,
        q.os,
        arch,
        q.keyword,
        q.limit.unwrap_or(20).clamp(1, 200) as usize,
        q.offset.unwrap_or(0) as usize,
    )
    .await
    .map_err(|e| e.to_string())
}

#[command]
pub async fn list_installed_ffmpeg_versions(
) -> Result<Vec<crate::storage::ffmpeg_versions::FfmpegVersionItem>, String> {
    list_installed_ffmpeg_versions_with_system().await
}

#[command]
pub async fn download_ffmpeg_version(app: AppHandle, row_key: String) -> Result<String, String> {
    let item = crate::storage::ffmpeg_versions::get_by_row_key(row_key.as_str())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "ffmpeg version not found".to_string())?;
    let url = item
        .download_url
        .clone()
        .ok_or_else(|| "download_url is empty".to_string())?;

    crate::storage::ffmpeg_versions::set_download_state(row_key.as_str(), "downloading")
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(mut canceled) = CANCELED_FFMPEG_DOWNLOADS.lock() {
        canceled.remove(row_key.as_str());
    }

    let app_handle = app.clone();
    let row_key_cloned = row_key.clone();
    let item_cloned = item.clone();
    let download_result =
        tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
            let mut target_dir: PathBuf =
                dirs::data_local_dir().unwrap_or_else(|| std::env::temp_dir());
            target_dir.push("easyff");
            target_dir.push("ffmpeg");
            fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

            let filename = infer_download_name(&item_cloned);
            let target_path = target_dir.join(filename);

            let client = reqwest::blocking::Client::new();
            let mut response = client.get(url.as_str()).send().map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err(format!("download failed with status {}", response.status()));
            }

            let total = response.content_length();
            let mut file = File::create(&target_path).map_err(|e| e.to_string())?;
            let mut downloaded = 0_u64;
            let mut buf = [0_u8; 32 * 1024];

            let _ = app_handle.emit(
                "ffmpeg-download-progress",
                FfmpegDownloadProgressEvent {
                    row_key: row_key_cloned.clone(),
                    downloaded: 0,
                    total,
                    percent: 0.0,
                    status: "downloading".to_string(),
                },
            );

            loop {
                let is_canceled = CANCELED_FFMPEG_DOWNLOADS
                    .lock()
                    .map(|set| set.contains(row_key_cloned.as_str()))
                    .unwrap_or(false);
                if is_canceled {
                    let _ = fs::remove_file(&target_path);
                    return Err("download canceled".to_string());
                }
                let n = response.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                downloaded += n as u64;
                let percent = total
                    .map(|v| (downloaded as f64 * 100.0 / v as f64).min(100.0))
                    .unwrap_or(0.0);
                let _ = app_handle.emit(
                    "ffmpeg-download-progress",
                    FfmpegDownloadProgressEvent {
                        row_key: row_key_cloned.clone(),
                        downloaded,
                        total,
                        percent,
                        status: "downloading".to_string(),
                    },
                );
            }

            let _ = app_handle.emit(
                "ffmpeg-download-progress",
                FfmpegDownloadProgressEvent {
                    row_key: row_key_cloned.clone(),
                    downloaded,
                    total,
                    percent: 100.0,
                    status: "completed".to_string(),
                },
            );
            let executable = resolve_ffmpeg_binary_from_download(
                target_path.as_path(),
                target_dir.as_path(),
                row_key_cloned.as_str(),
            )?;
            Ok(executable.to_string_lossy().to_string())
        })
        .await
        .map_err(|e| format!("[JOIN:download_ffmpeg_version] {}", e))?;

    let path = match download_result {
        Ok(path) => path,
        Err(err) => {
            let state = if err.to_ascii_lowercase().contains("cancel") {
                "not_downloaded"
            } else {
                "failed"
            };
            let _ =
                crate::storage::ffmpeg_versions::set_download_state(row_key.as_str(), state).await;
            return Err(err);
        }
    };

    crate::storage::ffmpeg_versions::mark_downloaded(row_key.as_str(), path.as_str())
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(mut canceled) = CANCELED_FFMPEG_DOWNLOADS.lock() {
        canceled.remove(row_key.as_str());
    }

    Ok(path)
}

#[command]
pub async fn cancel_ffmpeg_download(row_key: String) -> Result<(), String> {
    let key = row_key.trim();
    if key.is_empty() {
        return Err("row_key is required".to_string());
    }
    if let Ok(mut canceled) = CANCELED_FFMPEG_DOWNLOADS.lock() {
        canceled.insert(key.to_string());
    }
    crate::storage::ffmpeg_versions::set_download_state(key, "not_downloaded")
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn update_ffmpeg_download_state(row_key: String, state: String) -> Result<(), String> {
    crate::storage::ffmpeg_versions::set_download_state(row_key.as_str(), state.as_str())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn activate_ffmpeg_version(row_key: String) -> Result<(), String> {
    let key = row_key.trim().to_string();
    if key.is_empty() {
        return Err("row_key is required".to_string());
    }
    if key == "__system_ffmpeg__" {
        return crate::storage::ffmpeg_versions::deactivate_all()
            .await
            .map_err(|e| e.to_string());
    }

    let item = crate::storage::ffmpeg_versions::get_by_row_key(key.as_str())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "ffmpeg version not found".to_string())?;

    let local_path = item
        .local_path
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "ffmpeg binary path is empty, please download again".to_string())?;

    let binary_path = PathBuf::from(local_path.clone());
    if !binary_path.exists() || !binary_path.is_file() {
        let _ = crate::storage::ffmpeg_versions::set_download_state(key.as_str(), "failed").await;
        return Err(format!(
            "ffmpeg binary not found: {}. please re-download this version",
            local_path
        ));
    }

    crate::storage::ffmpeg_versions::activate(key.as_str())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_ffmpeg_version(row_key: String) -> Result<(), String> {
    if row_key.trim() == "__system_ffmpeg__" {
        return Err("system ffmpeg cannot be deleted from EasyFF".to_string());
    }
    crate::storage::ffmpeg_versions::remove_installation(row_key.as_str())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_current_ffmpeg_version() -> Result<serde_json::Value, String> {
    let installed = list_installed_ffmpeg_versions_with_system().await?;
    let active = installed.iter().find(|v| v.is_active).cloned();
    let table_version = active
        .as_ref()
        .map(|v| v.version.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let active_path = active
        .as_ref()
        .and_then(|v| v.local_path.as_ref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let active_display = active
        .as_ref()
        .map(|v| format!("{} ({})", v.version, v.source));

    let probe = tauri::async_runtime::spawn_blocking(move || -> Option<(String, String)> {
        let mut candidates: Vec<String> = Vec::new();
        if let Some(path) = active_path.clone() {
            candidates.push(path);
        }
        if cfg!(target_os = "windows") {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let local = exe_dir.join("ffmpeg.exe");
                    if local.exists() {
                        let p = local.to_string_lossy().to_string();
                        if !candidates.iter().any(|v| v == &p) {
                            candidates.push(p);
                        }
                    }
                }
            }
            candidates.push("ffmpeg.exe".to_string());
            candidates.push("ffmpeg".to_string());
        } else {
            candidates.push("ffmpeg".to_string());
        }

        for bin in candidates {
            if let Some((version, _)) = probe_ffmpeg_binary(bin.as_str()) {
                return Some((version, bin));
            }
        }
        None
    })
    .await
    .map_err(|e| format!("[JOIN:get_current_ffmpeg_version] {}", e))?;

    if let Some((version, executable_path)) = probe {
        return Ok(serde_json::json!({
            "version": table_version,
            "displayVersion": version,
            "executablePath": executable_path,
        }));
    }

    let fallback_version = active_display.unwrap_or_else(|| "unknown".to_string());
    Ok(serde_json::json!({
        "version": table_version,
        "displayVersion": fallback_version,
        "executablePath": active.and_then(|v| v.local_path),
    }))
}

#[command]
pub async fn list_favorite_commands(
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<crate::storage::favorite_commands::FavoriteCommandItem>, String> {
    crate::storage::favorite_commands::list(
        limit.unwrap_or(100).clamp(1, 200) as usize,
        offset.unwrap_or(0) as usize,
    )
    .await
    .map_err(|e| e.to_string())
}

#[command]
pub async fn create_favorite_command(
    input: FavoriteCommandInput,
) -> Result<crate::storage::favorite_commands::FavoriteCommandItem, String> {
    let title = input.title.trim().to_string();
    let command = input.command.trim().to_string();
    if title.is_empty() {
        return Err("title is required".to_string());
    }
    if command.is_empty() {
        return Err("command is required".to_string());
    }

    crate::storage::favorite_commands::create(
        title,
        input
            .description
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        command,
    )
    .await
    .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_favorite_command(id: String) -> Result<(), String> {
    crate::storage::favorite_commands::delete(id.trim())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn list_favorite_commands_for_sync(
    query: Option<FavoriteSyncQuery>,
) -> Result<Vec<crate::storage::favorite_commands::FavoriteCommandItem>, String> {
    let q = query.unwrap_or(FavoriteSyncQuery {
        limit: Some(200),
        offset: Some(0),
        include_deleted: Some(false),
    });
    crate::storage::favorite_commands::list_for_sync(
        q.limit.unwrap_or(200).clamp(1, 1000) as usize,
        q.offset.unwrap_or(0) as usize,
        q.include_deleted.unwrap_or(false),
    )
    .await
    .map_err(|e| e.to_string())
}

#[command]
pub async fn list_pending_favorite_command_sync(
    limit: Option<u32>,
) -> Result<Vec<crate::storage::favorite_commands::FavoriteCommandItem>, String> {
    crate::storage::favorite_commands::list_pending_sync(
        limit.unwrap_or(200).clamp(1, 1000) as usize
    )
    .await
    .map_err(|e| e.to_string())
}

#[command]
pub async fn apply_remote_favorite_command_changes(
    input: FavoriteSyncApplyRemoteInput,
) -> Result<usize, String> {
    crate::storage::favorite_commands::upsert_from_remote_batch(input.items)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn mark_favorite_commands_synced(input: FavoriteSyncAckInput) -> Result<usize, String> {
    crate::storage::favorite_commands::mark_synced(input.acks)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_favorite_command_sync_cursor() -> Result<i64, String> {
    crate::storage::favorite_commands::get_sync_cursor()
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn set_favorite_command_sync_cursor(
    input: FavoriteSyncCursorInput,
) -> Result<(), String> {
    crate::storage::favorite_commands::set_sync_cursor(input.cursor)
        .await
        .map_err(|e| e.to_string())
}


