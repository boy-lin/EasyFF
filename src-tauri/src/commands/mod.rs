use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::command;
use tauri::ipc::JavaScriptChannelId;
use tauri::{AppHandle, Emitter, State};

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
pub async fn report_client_log(_log: ClientLogInput) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn export_logs_archive(_app: AppHandle) -> Result<String, String> { Err("command disabled".to_string()) }
#[command]
pub async fn get_detailed_media_info(_path: String) -> Result<MediaDetails, String> { Err("command disabled".to_string()) }
#[command]
pub async fn get_detailed_image_info(_path: String) -> Result<MediaDetails, String> { Err("command disabled".to_string()) }
#[command]
pub async fn get_detailed_media_info_batch(_paths: Vec<String>) -> Result<Vec<MediaDetails>, String> { Err("command disabled".to_string()) }
#[command]
pub async fn probe_media_info(_path: String) -> Result<MediaProbeResult, String> { Err("command disabled".to_string()) }
#[command]
pub async fn probe_media_info_batch(_paths: Vec<String>) -> Result<Vec<MediaProbeResult>, String> { Err("command disabled".to_string()) }
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
pub async fn run_self_check() -> Result<SelfCheckResult, String> { Err("command disabled".to_string()) }
#[command]
pub async fn get_device_id() -> Result<String, String> { Err("command disabled".to_string()) }
#[command]
pub async fn auth_exchange_code(_input: AuthExchangeCodeInput) -> Result<AuthTokenResponse, String> { Err("command disabled".to_string()) }
#[command]
pub async fn updater_guard_report_success() -> Result<UpdaterGuardStatus, String> { Err("command disabled".to_string()) }
#[command]
pub async fn updater_guard_report_failure(_reason: Option<String>) -> Result<UpdaterGuardStatus, String> { Err("command disabled".to_string()) }
#[command]
pub async fn updater_guard_get_status() -> Result<UpdaterGuardStatus, String> { Err("command disabled".to_string()) }
#[command]
pub async fn updater_guard_reset() -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn check_hardware_acceleration() -> Result<HardwareSupport, String> { Err("command disabled".to_string()) }
#[command]
pub async fn get_media_info(_path: String) -> Result<FileInfo, String> { Err("command disabled".to_string()) }
#[command]
pub async fn prepare_video_for_web_playback(_path: String) -> Result<WebPlaybackPrepareResult, String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_mse_stream_open(_app: AppHandle, _path: String, _chunk_channel: JavaScriptChannelId, _stream_state: State<'_, VideoMseStreamState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_mse_stream_close(_stream_state: State<'_, VideoMseStreamState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_open(_app: AppHandle, _path: String, _preview: Option<PreviewSize>, _frame_channel: Option<JavaScriptChannelId>, _player_state: State<'_, PlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_play(_player_state: State<'_, PlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_get_size(_player_state: State<'_, PlayerState>) -> Result<(u32, u32), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_pause(_player_state: State<'_, PlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_seek(_position: f64, _player_state: State<'_, PlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_get_position(_player_state: State<'_, PlayerState>) -> Result<f64, String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_get_duration(_player_state: State<'_, PlayerState>) -> Result<f64, String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_close(_player_state: State<'_, PlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn video_player_set_volume(_volume: f32, _player_state: State<'_, PlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_open(_app: AppHandle, _path: String, _audio_player_state: State<'_, AudioPlayerState>) -> Result<String, String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_play(_audio_player_state: State<'_, AudioPlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_pause(_audio_player_state: State<'_, AudioPlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_seek(_position: f64, _audio_player_state: State<'_, AudioPlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_stop(_audio_player_state: State<'_, AudioPlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_set_volume(_volume: f32, _audio_player_state: State<'_, AudioPlayerState>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_get_position(_audio_player_state: State<'_, AudioPlayerState>) -> Result<f64, String> { Err("command disabled".to_string()) }
#[command]
pub async fn audio_player_get_duration(_audio_player_state: State<'_, AudioPlayerState>) -> Result<f64, String> { Err("command disabled".to_string()) }
#[command]
pub async fn get_audio_file_info(_path: String) -> Result<serde_json::Value, String> { Err("command disabled".to_string()) }
#[command]
pub async fn convert_audio_file(_app: AppHandle, _args: AudioConversionArgs) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn convert_gif_file(_app: AppHandle, _args: GifConversionArgs) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn generate_media_thumbnail(_window: tauri::Window, _request_id: String, _path: String, _options: Option<serde_json::Value>) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn compress_video_file(_app: AppHandle, _args: VideoCompressionArgs) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn compress_audio_file(_app: AppHandle, _args: AudioCompressionArgs) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn compress_image_file(_app: AppHandle, _args: ImageCompressionArgs) -> Result<(), String> { Err("command disabled".to_string()) }
#[command]
pub async fn write_media_metadata(_args: WriteMetadataArgs) -> Result<(), String> { Err("command disabled".to_string()) }
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
    pub source: Option<String>,
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
        source: None,
        os: None,
        arch: None,
        keyword: None,
        limit: Some(20),
        offset: Some(0),
    });

    let arch = q.arch.or_else(|| Some(detect_host_arch()));

    crate::storage::ffmpeg_versions::list(
        q.source,
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
    crate::storage::ffmpeg_versions::list_installed()
        .await
        .map_err(|e| e.to_string())
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

    let app_handle = app.clone();
    let row_key_cloned = row_key.clone();
    let item_cloned = item.clone();
    let path = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
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
        Ok(target_path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("[JOIN:download_ffmpeg_version] {}", e))??;

    crate::storage::ffmpeg_versions::mark_downloaded(row_key.as_str(), path.as_str())
        .await
        .map_err(|e| e.to_string())?;

    Ok(path)
}

#[command]
pub async fn update_ffmpeg_download_state(row_key: String, state: String) -> Result<(), String> {
    crate::storage::ffmpeg_versions::set_download_state(row_key.as_str(), state.as_str())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn activate_ffmpeg_version(row_key: String) -> Result<(), String> {
    crate::storage::ffmpeg_versions::activate(row_key.as_str())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn delete_ffmpeg_version(row_key: String) -> Result<(), String> {
    crate::storage::ffmpeg_versions::remove_installation(row_key.as_str())
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_current_ffmpeg_version() -> Result<serde_json::Value, String> {
    let installed = crate::storage::ffmpeg_versions::list_installed()
        .await
        .map_err(|e| e.to_string())?;
    let active = installed.iter().find(|v| v.is_active).cloned();

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
            let output = Command::new(&bin).arg("-version").output();
            let Ok(out) = output else { continue };
            if !out.status.success() {
                continue;
            }
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = stdout.lines().next() {
                let v = first_line.trim();
                if !v.is_empty() {
                    return Some((v.to_string(), bin));
                }
            }
        }
        None
    })
    .await
    .map_err(|e| format!("[JOIN:get_current_ffmpeg_version] {}", e))?;

    if let Some((version, executable_path)) = probe {
        return Ok(serde_json::json!({
            "version": version,
            "displayVersion": version,
            "executablePath": executable_path,
        }));
    }

    let fallback_version = active_display.unwrap_or_else(|| "unknown".to_string());
    Ok(serde_json::json!({
        "version": fallback_version,
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
        input.description.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()),
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

