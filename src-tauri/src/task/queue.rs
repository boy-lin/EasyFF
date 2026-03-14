use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
fn apply_no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn apply_no_window(_cmd: &mut Command) {}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct MediaTaskRequest {
    pub task_id: String,
    pub task_type: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub input_path: Option<String>,
    pub output_dir: Option<String>,
}

#[derive(Serialize, Clone)]
struct MediaTaskEventPayload {
    task_id: String,
    task_type: String,
    file_type: String,
    event_type: String,
    progress: Option<u32>,
    output_path: Option<String>,
    output_size: Option<u64>,
    error_message: Option<String>,
}

#[derive(Serialize, Clone)]
struct MediaTaskLogPayload {
    task_id: String,
    task_type: String,
    stream: String,
    line: String,
    ts: i64,
}

static RUNNING_TASKS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn guess_file_type(input_path: Option<&str>) -> String {
    let Some(input) = input_path else {
        return "video".to_string();
    };
    let ext = Path::new(input)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "mp3" | "aac" | "wav" | "flac" | "m4a" | "ogg") {
        "audio".to_string()
    } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif") {
        if ext == "gif" {
            "gif".to_string()
        } else {
            "image".to_string()
        }
    } else {
        "video".to_string()
    }
}

async fn emit_event(
    app: &AppHandle,
    task: &MediaTaskRequest,
    event_type: &str,
    progress: Option<u32>,
    output_path: Option<String>,
    output_size: Option<u64>,
    error_message: Option<String>,
) {
    let payload = MediaTaskEventPayload {
        task_id: task.task_id.clone(),
        task_type: task.task_type.clone(),
        file_type: guess_file_type(task.input_path.as_deref()),
        event_type: event_type.to_string(),
        progress,
        output_path,
        output_size,
        error_message,
    };
    let _ = app.emit("media_task_event", payload);
}

async fn emit_log(app: &AppHandle, task: &MediaTaskRequest, stream: &str, line: String) {
    if line.trim().is_empty() {
        return;
    }
    let payload = MediaTaskLogPayload {
        task_id: task.task_id.clone(),
        task_type: task.task_type.clone(),
        stream: stream.to_string(),
        line,
        ts: crate::shared::get_millis(),
    };
    let _ = app.emit("media_task_log", payload);
}

fn guess_output_path(args: &[String]) -> Option<String> {
    args.last().cloned().filter(|v| !v.trim().is_empty())
}

fn quote_cmd_arg(arg: &str) -> String {
    if arg.contains(' ') || arg.contains('\t') || arg.contains('"') {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

async fn persist_history(task: &MediaTaskRequest, status: &str, error_message: Option<String>) {
    let now = crate::shared::get_millis();
    let output_path = guess_output_path(task.args.as_slice());

    let history = crate::storage::task_history::TaskHistoryItem {
        id: task.task_id.clone(),
        task_type: task.task_type.clone(),
        status: status.to_string(),
        input_path: task.input_path.clone().unwrap_or_default(),
        output_path: output_path.clone(),
        created_at: now,
        finished_at: now,
        error_message,
        task_data: serde_json::to_string(task).unwrap_or_default(),
        command_line: Some(format!("{} {}", task.command, task.args.join(" "))),
    };

    let _ = crate::storage::task_history::add_history(&history).await;
}

async fn run_task(app: AppHandle, task: MediaTaskRequest) {
    {
        let mut running = RUNNING_TASKS.lock().await;
        running.insert(task.task_id.clone(), task.task_type.clone());
    }

    let _ = crate::storage::media_queue::remove_by_task_id(task.task_id.as_str()).await;
    emit_event(&app, &task, "progress", Some(5), None, None, None).await;

    let mut cmd = Command::new(task.command.as_str());
    apply_no_window(&mut cmd);
    cmd.args(task.args.as_slice());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Prevent interactive ffmpeg prompts (e.g. overwrite confirmation) from hanging the task.
    cmd.stdin(Stdio::null());
    if let Some(dir) = task.output_dir.as_ref().filter(|v| !v.trim().is_empty()) {
        cmd.current_dir(dir);
    }

    let cmd_line = std::iter::once(task.command.as_str())
        .chain(task.args.iter().map(|v| v.as_str()))
        .map(quote_cmd_arg)
        .collect::<Vec<_>>()
        .join(" ");
    println!("[media_task][run_task] id={} cmd={}", task.task_id, cmd_line);
    let cwd = task
        .output_dir
        .as_ref()
        .map(|v| v.as_str())
        .unwrap_or("");
    emit_log(
        &app,
        &task,
        "stdout",
        format!("[runner] executable={} cwd={} cmd={}", task.command, cwd, cmd_line),
    )
    .await;

    let result = cmd.spawn();
    match result {
        Ok(mut child) => {
            let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

            let stdout_task = child.stdout.take().map(|stdout| {
                let app_handle = app.clone();
                let task_clone = task.clone();
                tauri::async_runtime::spawn(async move {
                    let mut lines = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        emit_log(&app_handle, &task_clone, "stdout", line).await;
                    }
                })
            });

            let stderr_task = child.stderr.take().map(|stderr| {
                let app_handle = app.clone();
                let task_clone = task.clone();
                let stderr_lines = stderr_lines.clone();
                tauri::async_runtime::spawn(async move {
                    let mut lines = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        emit_log(&app_handle, &task_clone, "stderr", line.clone()).await;
                        let mut buf = stderr_lines.lock().await;
                        buf.push(line);
                        if buf.len() > 80 {
                            let drain = buf.len() - 80;
                            buf.drain(0..drain);
                        }
                    }
                })
            });

            let status_result = child.wait().await;
            if let Some(handle) = stdout_task {
                let _ = handle.await;
            }
            if let Some(handle) = stderr_task {
                let _ = handle.await;
            }

            match status_result {
                Ok(status) if status.success() => {
                    let output_path = guess_output_path(task.args.as_slice());
                    let output_size = output_path
                        .as_ref()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len() as u64);
                    emit_event(
                        &app,
                        &task,
                        "complete",
                        Some(100),
                        output_path,
                        output_size,
                        None,
                    )
                    .await;
                    persist_history(&task, "finished", None).await;
                }
                Ok(status) => {
                    let err = {
                        let lines = stderr_lines.lock().await;
                        if lines.is_empty() {
                            format!("process exited with status {}", status)
                        } else {
                            lines
                                .iter()
                                .rev()
                                .take(8)
                                .cloned()
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    };
                    emit_event(&app, &task, "error", Some(100), None, None, Some(err.clone())).await;
                    persist_history(&task, "error", Some(err)).await;
                }
                Err(e) => {
                    let err = e.to_string();
                    emit_event(&app, &task, "error", Some(100), None, None, Some(err.clone())).await;
                    persist_history(&task, "error", Some(err)).await;
                }
            }
        }
        Err(e) => {
            let err = e.to_string();
            emit_event(&app, &task, "error", Some(100), None, None, Some(err.clone())).await;
            persist_history(&task, "error", Some(err)).await;
        }
    }

    let mut running = RUNNING_TASKS.lock().await;
    running.remove(&task.task_id);
}

pub async fn submit_tasks(app: AppHandle, tasks: Vec<MediaTaskRequest>) -> Result<usize, String> {
    for task in tasks.iter() {
        crate::storage::media_queue::enqueue(task)
            .await
            .map_err(|e| e.to_string())?;
    }

    for task in tasks.clone() {
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            run_task(app_handle, task).await;
        });
    }

    Ok(tasks.len())
}

pub async fn has_running(task_type: Option<String>) -> bool {
    let running = RUNNING_TASKS.lock().await;
    if let Some(task_type) = task_type.filter(|v| !v.trim().is_empty()) {
        running.values().any(|t| t == task_type.as_str())
    } else {
        !running.is_empty()
    }
}

pub async fn clear_pending(task_type: Option<String>) -> Result<usize, String> {
    if let Some(task_type) = task_type.filter(|v| !v.trim().is_empty()) {
        crate::storage::media_queue::clear_by_type(task_type.as_str())
            .await
            .map_err(|e| e.to_string())
    } else {
        let count = crate::storage::media_queue::count().await.map_err(|e| e.to_string())?;
        crate::storage::media_queue::clear().await.map_err(|e| e.to_string())?;
        Ok(count)
    }
}

pub async fn clear_pending_with_cancel(
    task_type: Option<String>,
    _stop_running: bool,
) -> Result<usize, String> {
    clear_pending(task_type).await
}

pub async fn cancel_task(task_id: String) -> Result<(), String> {
    let removed = crate::storage::media_queue::remove_by_task_id(task_id.as_str())
        .await
        .map_err(|e| e.to_string())?;
    if removed > 0 {
        return Ok(());
    }

    let running = RUNNING_TASKS.lock().await;
    if running.contains_key(&task_id) {
        return Err("task is running, cancel for running process is not supported yet".to_string());
    }

    Ok(())
}
