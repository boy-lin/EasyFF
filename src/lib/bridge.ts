import { emit, listen, UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Channel, invoke } from "@tauri-apps/api/core";
import { FileType } from "@/types/tasks";
import { MediaTaskType } from "@/types/tasks";
import { handleDirectoryToFiles } from "./file";
import { MediaTaskEvent } from "./mediaTaskEvent";

export type DownloadProgress = {
  stage: string;
  downloaded: number;
  total?: number | null;
};

export type BridgeEvents = {
  "single-instance": {
    args?: string[];
    cwd?: string;
  };
  media_task_event: MediaTaskEvent;
  media_task_log: {
    task_id: string;
    task_type: string;
    stream: "stdout" | "stderr" | string;
    line: string;
    ts: number;
  };
  "ffmpeg-download-progress": {
    rowKey: string;
    downloaded: number;
    total?: number | null;
    percent: number;
    status: "downloading" | "completed";
  };
};

type KnownEvent = keyof BridgeEvents;
type EventPayload<K extends string> = K extends KnownEvent
  ? BridgeEvents[K]
  : unknown;

export interface HardwareSupport {
  h264_hardware: boolean;
  hevc_hardware: boolean;
  prores_hardware: boolean;
}

export interface SelfCheckResult {
  ffmpeg_installed?: boolean;
  ffprobe_installed?: boolean;
  ffmpeg_path?: string | null;
  ffmpeg_version?: string | null;
  ffprobe_path?: string | null;
  ffprobe_version?: string | null;
  fs_permission: boolean;
  fs_error?: string | null;
}

export interface WriteMetadataArgs {
  input_path: string;
  output_path: string;
  metadata: Record<string, string>;
}

export interface ClientLogInput {
  level: "error" | "warn" | "info";
  category: string;
  message: string;
  stack?: string;
  url?: string;
  meta?: Record<string, unknown>;
  timestamp?: number;
}

export interface AuthExchangeCodeInput {
  tokenEndpoint: string;
  clientId: string;
  code: string;
  codeVerifier: string;
  redirectUri: string;
}

export interface UpdaterGuardStatus {
  shouldForceUpdate: boolean;
  effectiveFailCount: number;
  lastSuccessAtMs?: number | null;
}

export interface FfmpegVersionItem {
  rowKey: string;
  source: string;
  os: string;
  version: string;
  publishedAt?: string | null;
  downloadUrl?: string | null;
  arch?: string | null;
  localPath?: string | null;
  updatedAt: number;
  downloadState: "not_downloaded" | "downloading" | "downloaded" | "failed";
  installed: boolean;
  isActive: boolean;
}

export interface FfmpegVersionListResult {
  list: FfmpegVersionItem[];
  total: number;
  hasMore: boolean;
  nextOffset: number;
}

export interface FfmpegDownloadProgress {
  rowKey: string;
  downloaded: number;
  total?: number | null;
  percent: number;
  status: "downloading" | "completed";
}

export interface CurrentFfmpegRuntimeInfo {
  version: string;
  displayVersion: string;
  executablePath?: string | null;
}

export interface FavoriteCommandItem {
  id: string;
  title: string;
  description?: string | null;
  command: string;
  createdAt: number;
  updatedAt: number;
  deletedAt?: number | null;
  syncState?: "synced" | "pending_upsert" | "pending_delete" | "conflict";
  serverVersion?: number | null;
  updatedByDeviceId?: string | null;
}

export interface CreateFavoriteCommandInput {
  title: string;
  description?: string;
  command: string;
}

export interface FavoriteSyncAck {
  id: string;
  serverVersion?: number | null;
  updatedAt?: number | null;
  deletedAt?: number | null;
  updatedByDeviceId?: string | null;
}

export interface BridgeInvokeError extends Error {
  code?: string;
  context?: string;
  originalMessage?: string;
}

export type MediaTaskPriority = "high" | "normal" | "low";

export type ThumbnailPayload = {
  thumbnailPath?: string;
  dataUrl?: string;
  width: number;
  height: number;
  sourceWidth?: number;
  sourceHeight?: number;
};

export interface VideoPlayerOpenInput {
  path: string;
  preview?: {
    width: number;
    height: number;
  };
}

export interface VideoPlayerSize {
  width: number;
  height: number;
}

export interface WebPlaybackPrepareResult {
  playPath: string;
  prepared: boolean;
  reason: string;
}

class Bridge {
  private static instance: Bridge | null = null;
  private disposers: UnlistenFn[] = [];
  private fallbackTarget = new EventTarget();
  private tauriReady = true;
  private tauriEventUnlisteners = new Map<string, UnlistenFn>();
  private tauriEventHandlers = new Map<
    string,
    Set<(payload: unknown) => void>
  >();
  videoFrameChannel: Channel<unknown> | null = null;
  private constructor() {
    if (Bridge.instance) {
      return Bridge.instance;
    }
    Bridge.instance = this;
  }

  static getInstance(): Bridge {
    if (Bridge.instance === null) {
      Bridge.instance = new Bridge();
    }
    return Bridge.instance;
  }

  isTauri() {
    return this.tauriReady;
  }

  isTauriEvn() {
    return typeof window !== "undefined" && "__TAURI__" in window;
  }

  async on<K extends string>(
    event: K,
    handler: (payload: EventPayload<K>) => void,
  ): Promise<() => void> {
    if (this.tauriReady) {
      const eventKey = String(event);
      const typedHandler = handler as (payload: unknown) => void;
      let handlers = this.tauriEventHandlers.get(eventKey);
      if (!handlers) {
        handlers = new Set<(payload: unknown) => void>();
        this.tauriEventHandlers.set(eventKey, handlers);
      }
      handlers.add(typedHandler);

      if (!this.tauriEventUnlisteners.has(eventKey)) {
        const unlisten = await listen<unknown>(eventKey, ({ payload }) => {
          const listeners = this.tauriEventHandlers.get(eventKey);
          if (!listeners || listeners.size === 0) return;
          listeners.forEach((listener) => listener(payload));
        });
        this.tauriEventUnlisteners.set(eventKey, unlisten);
        this.disposers.push(unlisten);
      }

      return () => {
        const listeners = this.tauriEventHandlers.get(eventKey);
        if (!listeners) return;
        listeners.delete(typedHandler);
        if (listeners.size > 0) return;

        this.tauriEventHandlers.delete(eventKey);
        const unlisten = this.tauriEventUnlisteners.get(eventKey);
        if (unlisten) {
          unlisten();
          this.tauriEventUnlisteners.delete(eventKey);
          this.disposers = this.disposers.filter((fn) => fn !== unlisten);
        }
      };
    }

    const wrapped = (evt: Event) => {
      const detail = (evt as CustomEvent<EventPayload<K>>).detail;
      handler(detail);
    };
    this.fallbackTarget.addEventListener(event, wrapped);
    return () =>
      this.fallbackTarget.removeEventListener(event, wrapped as EventListener);
  }

  async emit<K extends string>(event: K, payload: EventPayload<K>) {
    if (this.tauriReady) {
      await emit(event, payload);
      return;
    }
    this.fallbackTarget.dispatchEvent(
      new CustomEvent<EventPayload<K>>(event, { detail: payload }),
    );
  }

  createEventWaiter<K extends string>(
    event: K,
    options?: {
      timeoutMs?: number;
      filter?: (payload: EventPayload<K>) => boolean;
      signal?: AbortSignal;
    },
  ): { promise: Promise<EventPayload<K>>; cancel: () => void } {
    const timeoutMs = options?.timeoutMs ?? 15000;
    let cancel: () => void = () => {};
    const promise = new Promise<EventPayload<K>>((resolve, reject) => {
      let settled = false;
      let timeoutId: number | null = null;
      let unlisten: (() => void) | null = null;

      const finalize = (err?: Error, payload?: EventPayload<K>) => {
        if (settled) return;
        settled = true;
        if (timeoutId !== null) window.clearTimeout(timeoutId);
        if (unlisten) unlisten();
        if (options?.signal) {
          options.signal.removeEventListener("abort", onAbort);
        }
        if (err) reject(err);
        else if (payload) resolve(payload);
      };

      const onAbort = () => {
        finalize(new Error(`Event "${String(event)}" aborted`));
      };

      this.on(event, (payload) => {
        if (options?.filter && !options.filter(payload)) return;
        finalize(undefined, payload);
      })
        .then((dispose) => {
          unlisten = dispose;
          timeoutId = window.setTimeout(() => {
            finalize(new Error(`Event "${String(event)}" timeout`));
          }, timeoutMs);
          if (options?.signal) {
            if (options.signal.aborted) {
              finalize(new Error(`Event "${String(event)}" aborted`));
              return;
            }
            options.signal.addEventListener("abort", onAbort, { once: true });
          }
        })
        .catch((err) => finalize(err));

      cancel = () => finalize(new Error(`Event "${String(event)}" cancelled`));
    });
    return { promise, cancel };
  }

  once<K extends string>(
    event: K,
    options?: {
      timeoutMs?: number;
      filter?: (payload: EventPayload<K>) => boolean;
      signal?: AbortSignal;
    },
  ): Promise<EventPayload<K>> {
    return this.createEventWaiter(event, options).promise;
  }

  async invoke<T = unknown>(
    cmd: string,
    args?: Record<string, unknown>,
  ): Promise<T> {
    if (!this.tauriReady) {
      console.warn(`[bridge] invoke "${cmd}" skipped: not running in Tauri`);
      return Promise.reject(new Error("Tauri runtime unavailable"));
    }
    try {
      return await invoke<T>(cmd, args);
    } catch (error) {
      throw this.parseInvokeError(error, cmd);
    }
  }

  private parseInvokeError(error: unknown, cmd: string): BridgeInvokeError {
    const rawMessage =
      (error as { message?: string } | null | undefined)?.message ||
      String(error ?? "Unknown invoke error");
    const matched = rawMessage.match(/^\[([A-Z_]+)(?::([^\]]+))?\]\s*(.*)$/);
    const parsedCode = matched?.[1];
    const parsedContext = matched?.[2];
    const parsedMessage = matched?.[3]?.trim();

    const err = new Error(
      parsedMessage?.length ? parsedMessage : rawMessage,
    ) as BridgeInvokeError;
    err.name = "BridgeInvokeError";
    err.code = parsedCode || "INVOKE_ERROR";
    err.context = parsedContext || cmd;
    err.originalMessage = rawMessage;
    return err;
  }

  async reportClientLog(log: ClientLogInput): Promise<void> {
    await this.invoke("report_client_log", { log });
  }

  async exportLogsArchive(): Promise<string> {
    return this.invoke<string>("export_logs_archive");
  }

  async authExchangeCode(input: AuthExchangeCodeInput): Promise<{
    access_token: string;
    refresh_token?: string | null;
    expires_in?: number | null;
    token_type?: string | null;
    id_token?: string | null;
  }> {
    return this.invoke("auth_exchange_code", { input });
  }

  async updaterGuardGetStatus(): Promise<UpdaterGuardStatus> {
    return this.invoke<UpdaterGuardStatus>("updater_guard_get_status");
  }

  async updaterGuardReportSuccess(): Promise<UpdaterGuardStatus> {
    return this.invoke<UpdaterGuardStatus>("updater_guard_report_success");
  }

  async updaterGuardReportFailure(
    reason?: string,
  ): Promise<UpdaterGuardStatus> {
    return this.invoke<UpdaterGuardStatus>("updater_guard_report_failure", {
      reason,
    });
  }

  async updaterGuardReset(): Promise<void> {
    await this.invoke("updater_guard_reset");
  }

  async submitMediaTasks(
    tasks: unknown[],
    priority: MediaTaskPriority = "normal",
  ): Promise<void> {
    await this.invoke("cli_task_submit", { tasks, priority });
  }

  async hasRunningMediaTasksByType(taskType?: MediaTaskType): Promise<boolean> {
    if (taskType) {
      return this.invoke<boolean>("media_task_has_running_by_type", {
        taskType,
      });
    }
    return this.invoke<boolean>("media_task_has_running_by_type");
  }

  async clearMediaTaskQueueByType(
    stopRunning: boolean = false,
    taskType?: MediaTaskType,
  ): Promise<void> {
    const args: Record<string, unknown> = { stopRunning };
    if (taskType) args.taskType = taskType;
    await this.invoke("media_task_clear_by_type_with_stop", args);
  }

  async cancelMediaTaskById(id: string): Promise<void> {
    await this.invoke("media_task_cancel_task", { id });
  }

  async revealItemInDirFallback(path: string): Promise<void> {
    await this.invoke("plugin:opener|reveal_item_in_dir", { paths: [path] });
  }

  async prepareVideoForWebPlayback(
    path: string,
  ): Promise<WebPlaybackPrepareResult> {
    return this.invoke<WebPlaybackPrepareResult>(
      "prepare_video_for_web_playback",
      { path },
    );
  }

  async getDeviceId(): Promise<string> {
    return this.invoke<string>("get_device_id");
  }

  async getTaskHistory(
    limit: number = 50,
    offset: number = 0,
    taskType?: string,
    keyword?: string,
    sortBy?: "created_at" | "output_name",
    sortOrder?: "asc" | "desc",
  ): Promise<TaskHistoryItem[]> {
    return this.invoke<TaskHistoryItem[]>("get_task_history", {
      limit,
      offset,
      taskType,
      keyword,
      sortBy,
      sortOrder,
    });
  }

  async deleteTaskHistory(id: string): Promise<void> {
    return this.invoke("delete_task_history", { id });
  }

  async clearTaskHistory(taskType?: string): Promise<void> {
    return this.invoke("clear_task_history", { taskType });
  }

  async listFfmpegVersions(query?: {
    os?: string;
    arch?: string;
    keyword?: string;
    limit?: number;
    offset?: number;
  }): Promise<FfmpegVersionListResult> {
    const detectWebHostOS = (): "windows" | "linux" | "macos" => {
      if (typeof navigator === "undefined") return "windows";
      const ua = navigator.userAgent.toLowerCase();
      const platform = (navigator.platform || "").toLowerCase();
      if (platform.includes("mac") || ua.includes("mac os")) return "macos";
      if (platform.includes("linux") || ua.includes("linux")) return "linux";
      return "windows";
    };

    const finalQuery = {
      ...query,
      os: query?.os ?? detectWebHostOS(),
    };

    return this.invoke<FfmpegVersionListResult>("list_ffmpeg_versions", {
      query: finalQuery,
    });
  }

  async listInstalledFfmpegVersions(): Promise<FfmpegVersionItem[]> {
    return this.invoke<FfmpegVersionItem[]>("list_installed_ffmpeg_versions");
  }

  async updateFfmpegDownloadState(
    rowKey: string,
    state: "not_downloaded" | "downloading" | "downloaded" | "failed",
  ): Promise<void> {
    await this.invoke("update_ffmpeg_download_state", { rowKey, state });
  }

  async activateFfmpegVersion(rowKey: string): Promise<void> {
    await this.invoke("activate_ffmpeg_version", { rowKey });
  }

  async deleteFfmpegVersion(rowKey: string): Promise<void> {
    await this.invoke("delete_ffmpeg_version", { rowKey });
  }

  async getCurrentFfmpegRuntimeInfo(): Promise<CurrentFfmpegRuntimeInfo> {
    const raw = await this.invoke<unknown>("get_current_ffmpeg_version");
    if (typeof raw === "string") {
      return {
        version: raw,
        displayVersion: raw,
        executablePath: null,
      };
    }
    const obj = (raw || {}) as Record<string, unknown>;
    const version = String(obj.version ?? "unknown");
    const displayVersion = String(obj.displayVersion ?? version);
    const executablePath =
      typeof obj.executablePath === "string" && obj.executablePath.trim()
        ? obj.executablePath
        : null;
    return {
      version,
      displayVersion,
      executablePath,
    };
  }

  async getCurrentFfmpegVersion(): Promise<string> {
    const info = await this.getCurrentFfmpegRuntimeInfo();
    return info.displayVersion || info.version || "unknown";
  }

  async downloadFfmpegVersion(
    rowKey: string,
    onProgress?: (progress: FfmpegDownloadProgress) => void,
  ): Promise<string> {
    let dispose: (() => void) | null = null;
    if (onProgress) {
      dispose = await this.on("ffmpeg-download-progress", (payload) => {
        if (payload.rowKey !== rowKey) return;
        onProgress(payload);
      });
    }
    try {
      return await this.invoke<string>("download_ffmpeg_version", { rowKey });
    } finally {
      if (dispose) dispose();
    }
  }

  async cancelFfmpegDownload(rowKey: string): Promise<void> {
    await this.invoke("cancel_ffmpeg_download", { rowKey });
  }

  async listFavoriteCommands(
    limit: number = 100,
    offset: number = 0,
  ): Promise<FavoriteCommandItem[]> {
    return this.invoke<FavoriteCommandItem[]>("list_favorite_commands", {
      limit,
      offset,
    });
  }

  async createFavoriteCommand(
    input: CreateFavoriteCommandInput,
  ): Promise<FavoriteCommandItem> {
    return this.invoke<FavoriteCommandItem>("create_favorite_command", {
      input,
    });
  }

  async deleteFavoriteCommand(id: string): Promise<void> {
    await this.invoke("delete_favorite_command", { id });
  }

  async listFavoriteCommandsForSync(query?: {
    limit?: number;
    offset?: number;
    includeDeleted?: boolean;
  }): Promise<FavoriteCommandItem[]> {
    return this.invoke<FavoriteCommandItem[]>(
      "list_favorite_commands_for_sync",
      {
        query,
      },
    );
  }

  async listPendingFavoriteCommandSync(
    limit: number = 200,
  ): Promise<FavoriteCommandItem[]> {
    return this.invoke<FavoriteCommandItem[]>(
      "list_pending_favorite_command_sync",
      {
        limit,
      },
    );
  }

  async applyRemoteFavoriteCommandChanges(
    items: FavoriteCommandItem[],
  ): Promise<number> {
    return this.invoke<number>("apply_remote_favorite_command_changes", {
      input: { items },
    });
  }

  async markFavoriteCommandsSynced(acks: FavoriteSyncAck[]): Promise<number> {
    return this.invoke<number>("mark_favorite_commands_synced", {
      input: { acks },
    });
  }

  async getFavoriteCommandSyncCursor(): Promise<number> {
    return this.invoke<number>("get_favorite_command_sync_cursor");
  }

  async setFavoriteCommandSyncCursor(cursor: number): Promise<void> {
    await this.invoke("set_favorite_command_sync_cursor", {
      input: { cursor },
    });
  }

  async getMyFiles(
    limit: number = 10,
    offset: number = 0,
    keyword?: string,
    sortBy?: "date" | "name",
    sortOrder?: "asc" | "desc",
    mediaType?: FileType,
  ): Promise<MyFileItem[]> {
    return this.invoke<MyFileItem[]>("get_my_files", {
      limit,
      offset,
      keyword,
      sortBy,
      sortOrder,
      mediaType,
    });
  }

  async getMyFilesPage(
    limit: number = 10,
    offset: number = 0,
    keyword?: string,
    sortBy?: "date" | "name",
    sortOrder?: "asc" | "desc",
    mediaType?: FileType,
  ): Promise<{ list: MyFileItem[]; hasMore: boolean }> {
    const pageSize = Math.max(1, limit);
    const rows = await this.getMyFiles(
      pageSize + 1,
      offset,
      keyword,
      sortBy,
      sortOrder,
      mediaType,
    );
    return {
      list: rows.slice(0, pageSize),
      hasMore: rows.length > pageSize,
    };
  }

  clear() {
    this.tauriEventHandlers.clear();
    this.tauriEventUnlisteners.clear();
    this.disposers.forEach((dispose) => dispose());
    this.disposers = [];
  }

  async getDirectoryToFiles(paths: string[], extensions: string[]) {
    try {
      if (!paths.length) return [];
      // 处理文件夹：如果是文件夹，读取文件夹下的所有支持文件（只递归一层）
      const finalPaths: string[] = await handleDirectoryToFiles({
        paths,
        depth: 1,
        supportedExtensions: extensions,
      });
      if (!finalPaths.length) return [];
      return finalPaths;
    } catch (err) {
      console.error("Error selecting files:", err);
      return [];
    }
  }

  async addFilesOrFolders(opts: {
    name: string;
    multiple: boolean;
    extensions: string[];
    directory?: boolean;
  }) {
    const {
      name = "",
      multiple = false,
      extensions = [],
      directory = false,
    } = opts;
    const selected = await open({
      multiple,
      filters: [
        {
          name,
          extensions,
        },
      ],
      directory,
    });
    if (!selected) return [];
    const paths: string[] = Array.isArray(selected) ? selected : [selected];
    if (directory) {
      return await this.getDirectoryToFiles(paths, extensions);
    }
    return paths;
  }
}

export interface TaskHistoryItem {
  id: string;
  task_type: MediaTaskType;
  status: "idle" | "processing" | "finished" | "error" | "cancelled";
  input_path: string;
  output_path?: string;
  created_at: number;
  finished_at: number;
  error_message?: string;
  // Deprecated: backend no longer returns these fields in history payload.
  task_data?: string;
}

export interface MyFileItem extends TaskHistoryItem {
  is_favorite?: boolean;
}

export const bridge = Bridge.getInstance();
