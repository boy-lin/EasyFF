use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use tauri::command;
use tauri::ipc::JavaScriptChannelId;
use tauri::{AppHandle, Emitter, State};

fn detect_host_os() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
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

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PreviewSize {
    pub width: u32,
    pub height: u32,
}

pub type PlayerState = Mutex<Option<()>>;
pub type AudioPlayerState = Mutex<Option<()>>;
pub type VideoMseStreamState = Mutex<Option<VideoMseStreamSession>>;

#[derive(Debug, Clone, Default)]
pub struct VideoMseStreamSession;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub format: String,
    pub format_long_name: Option<String>,
    pub codec: String,
    pub codec_long_name: Option<String>,
    pub resolution: String,
    pub width: u64,
    pub height: u64,
    pub duration: f64,
    pub output_dir: String,
    pub bitrate: Option<String>,
    pub fps: Option<String>,
    pub avg_frame_rate: Option<String>,
    pub nb_frames: Option<u64>,
    pub pix_fmt: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_codec_long_name: Option<String>,
    pub audio_channels: Option<String>,
    pub audio_channel_layout: Option<String>,
    pub audio_sample_rate: Option<String>,
    pub audio_bitrate: Option<String>,
    pub audio_bits_per_sample: Option<String>,
    pub audio_sample_fmt: Option<String>,
    pub format_bitrate: Option<String>,
    pub format_tags: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct TranscodeArgs {
    pub input: String,
    pub output: String,
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub format: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SelfCheckResult {
    pub fs_permission: bool,
    pub fs_error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ModuleInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub ffprobe_path: Option<String>,
    pub version: Option<String>,
    pub source: Option<String>,
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HardwareSupport {
    pub h264_hardware: bool,
    pub hevc_hardware: bool,
    pub prores_hardware: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebPlaybackPrepareResult {
    pub play_path: String,
    pub prepared: bool,
    pub reason: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct AudioConversionArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    pub output_path: Option<String>,
    pub format: String,
    pub audio_tracks: Option<Vec<serde_json::Value>>,
    pub codec: Option<String>,
    pub bitrate: Option<f32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_depth: Option<u32>,
    pub quality: Option<u32>,
    pub use_hardware_acceleration: Option<bool>,
    pub use_ultra_fast_speed: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct DenoiseMediaArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    pub output_path: Option<String>,
    pub format: Option<String>,
    pub engine: Option<String>,
    pub filter: Option<serde_json::Value>,
    pub use_hardware_acceleration: Option<bool>,
    pub use_ultra_fast_speed: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct VideoConversionArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    pub output_path: Option<String>,
    pub format: Option<String>,
    pub video_encoder: Option<String>,
    pub video_bitrate: Option<u32>,
    pub min_bitrate: Option<u32>,
    pub max_bitrate: Option<u32>,
    pub rc_mode: Option<String>,
    pub crf: Option<u32>,
    pub resolution: Option<String>,
    pub aspect_ratio: Option<String>,
    pub scaling_mode: Option<String>,
    pub frame_rate: Option<String>,
    pub gop_size: Option<u32>,
    pub preset: Option<String>,
    pub profile: Option<String>,
    pub tune: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub bit_depth: Option<u32>,
    pub crop: Option<String>,
    pub audio_encoder: Option<String>,
    pub audio_bitrate: Option<u32>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
    pub audio_bit_depth: Option<u32>,
    pub audio_quality: Option<u32>,
    pub audio_tracks: Option<Vec<serde_json::Value>>,
    pub default_audio_params: Option<serde_json::Value>,
    pub use_hardware_acceleration: Option<bool>,
    pub use_ultra_fast_speed: Option<bool>,
    pub watermark: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct GifConversionArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    pub format: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub frame_rate: Option<f32>,
    #[serde(default)]
    pub quality: Option<u32>,
    #[serde(default)]
    pub preserve_transparency: Option<bool>,
    #[serde(default)]
    pub color_mode: Option<String>,
    #[serde(default)]
    pub dpi: Option<f64>,
    #[serde(default)]
    pub loop_count: Option<i32>,
    #[serde(default)]
    pub frame_delay: Option<u32>,
    #[serde(default)]
    pub colors: Option<u32>,
    #[serde(default)]
    pub preserve_extensions: Option<bool>,
    #[serde(default)]
    pub sharpen: Option<bool>,
    #[serde(default)]
    pub denoise: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct VideoCompressionArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    pub output_path: String,
    pub compression_ratio: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bitrate: Option<u32>,
    pub frame_rate: Option<f32>,
    pub codec: Option<String>,
    pub keyframe_interval: Option<u32>,
    pub color_depth: Option<u32>,
    pub aspect_ratio: Option<String>,
    pub remove_audio: Option<bool>,
    pub audio_tracks: Option<Vec<serde_json::Value>>,
    pub preset: Option<String>,
    pub use_hardware_acceleration: Option<bool>,
    pub use_ultra_fast_speed: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct AudioCompressionArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    pub output_path: String,
    #[serde(flatten)]
    pub encoding: HashMap<String, serde_json::Value>,
    pub format: Option<String>,
    pub remove_silence: Option<bool>,
    pub silence_threshold: Option<f32>,
    pub volume_gain: Option<f32>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ImageCompressionArgs {
    pub task_id: String,
    pub input_path: String,
    pub input_file_type: Option<String>,
    pub output_path: String,
    pub quality: Option<u32>,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub color_mode: Option<String>,
    pub strip_metadata: Option<bool>,
    pub keep_transparency: Option<bool>,
    pub dpi: Option<f64>,
    pub crop_whitespace: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct WriteMetadataArgs {
    pub input_path: String,
    pub output_path: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct UpdaterGuardStatus {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TaskHistoryItem {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MyFileItem {
    pub id: Option<String>,
}

#[command]
pub async fn report_client_log(_log: ClientLogInput) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn export_logs_archive(_app: AppHandle) -> Result<String, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn get_detailed_media_info(_path: String) -> Result<MediaDetails, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn get_detailed_image_info(_path: String) -> Result<MediaDetails, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn get_detailed_media_info_batch(
    _paths: Vec<String>,
) -> Result<Vec<MediaDetails>, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn probe_media_info(_path: String) -> Result<MediaProbeResult, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn probe_media_info_batch(_paths: Vec<String>) -> Result<Vec<MediaProbeResult>, String> {
    Err("command disabled".to_string())
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
pub async fn run_self_check() -> Result<SelfCheckResult, String> {
    Err("command disabled".to_string())
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
pub async fn updater_guard_report_success() -> Result<UpdaterGuardStatus, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn updater_guard_report_failure(
    _reason: Option<String>,
) -> Result<UpdaterGuardStatus, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn updater_guard_get_status() -> Result<UpdaterGuardStatus, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn updater_guard_reset() -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn check_hardware_acceleration() -> Result<HardwareSupport, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn get_media_info(_path: String) -> Result<FileInfo, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn prepare_video_for_web_playback(
    _path: String,
) -> Result<WebPlaybackPrepareResult, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_mse_stream_open(
    _app: AppHandle,
    _path: String,
    _chunk_channel: JavaScriptChannelId,
    _stream_state: State<'_, VideoMseStreamState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_mse_stream_close(
    _stream_state: State<'_, VideoMseStreamState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_open(
    _app: AppHandle,
    _path: String,
    _preview: Option<PreviewSize>,
    _frame_channel: Option<JavaScriptChannelId>,
    _player_state: State<'_, PlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_play(_player_state: State<'_, PlayerState>) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_get_size(
    _player_state: State<'_, PlayerState>,
) -> Result<(u32, u32), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_pause(_player_state: State<'_, PlayerState>) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_seek(
    _position: f64,
    _player_state: State<'_, PlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_get_position(
    _player_state: State<'_, PlayerState>,
) -> Result<f64, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_get_duration(
    _player_state: State<'_, PlayerState>,
) -> Result<f64, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_close(_player_state: State<'_, PlayerState>) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn video_player_set_volume(
    _volume: f32,
    _player_state: State<'_, PlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_open(
    _app: AppHandle,
    _path: String,
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<String, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_play(
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_pause(
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_seek(
    _position: f64,
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_stop(
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_set_volume(
    _volume: f32,
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_get_position(
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<f64, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn audio_player_get_duration(
    _audio_player_state: State<'_, AudioPlayerState>,
) -> Result<f64, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn get_audio_file_info(_path: String) -> Result<serde_json::Value, String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn convert_audio_file(_app: AppHandle, _args: AudioConversionArgs) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn convert_gif_file(_app: AppHandle, _args: GifConversionArgs) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn generate_media_thumbnail(
    _window: tauri::Window,
    _request_id: String,
    _path: String,
    _options: Option<serde_json::Value>,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn compress_video_file(
    _app: AppHandle,
    _args: VideoCompressionArgs,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn compress_audio_file(
    _app: AppHandle,
    _args: AudioCompressionArgs,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn compress_image_file(
    _app: AppHandle,
    _args: ImageCompressionArgs,
) -> Result<(), String> {
    Err("command disabled".to_string())
}
#[command]
pub async fn write_media_metadata(_args: WriteMetadataArgs) -> Result<(), String> {
    Err("command disabled".to_string())
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
    let output_arg = format!("-o{}", target_dir.to_string_lossy());
    for bin in ["7z", "7za"] {
        let output = Command::new(bin)
            .args([
                "x",
                "-y",
                output_arg.as_str(),
                archive_path.to_string_lossy().as_ref(),
            ])
            .output();
        let Ok(out) = output else { continue };
        if out.status.success() {
            return Ok(());
        }
    }
    Err("7z extraction failed. please install 7-Zip or choose a zip/tar.xz source".to_string())
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
