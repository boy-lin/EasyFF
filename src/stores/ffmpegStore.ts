import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { bridge, type FfmpegVersionItem } from "@/lib/bridge";

export type FfmpegRemoteVersion = {
  rowKey: string;
  source: string;
  version: string;
  releaseDate: string;
  osRaw: string;
  archRaw: string;
  arch: string;
  state: string;
  downloadState: FfmpegVersionItem["downloadState"];
};

export type FfmpegEnsureStage =
  | "idle"
  | "checking"
  | "downloading"
  | "activating"
  | "ready"
  | "error";

type DownloadOptions = {
  silentError?: boolean;
  autoActivate?: boolean;
  ensureFlow?: boolean;
};

type SourceMessagePayload = {
  key: string;
  values?: Record<string, string | number>;
};

type FfmpegStore = {
  remoteVersions: FfmpegRemoteVersion[];
  installedVersions: FfmpegVersionItem[];
  sourceMessage: SourceMessagePayload | null;
  runtimeVersion: string;
  runtimeExecutablePath: string | null;
  busyRowKey: string | null;
  cancelingRowKey: string | null;
  downloadProgress: Record<string, number>;
  ensureStage: FfmpegEnsureStage;
  ensureMessage: string;
  ensureTargetRowKey: string | null;
  lastSyncedAt: number | null;
  init: () => Promise<void>;
  refreshRuntime: () => Promise<void>;
  loadInstalled: () => Promise<void>;
  loadRemote: () => Promise<void>;
  refreshAll: () => Promise<void>;
  downloadVersion: (rowKey: string, options?: DownloadOptions) => Promise<void>;
  cancelDownload: (rowKey: string) => Promise<void>;
  activateVersion: (
    rowKey: string,
    options?: { quietEnsure?: boolean },
  ) => Promise<void>;
  deleteVersion: (rowKey: string) => Promise<void>;
  ensureReadyForHome: () => Promise<void>;
};

const formatArchLabel = (os?: string | null, arch?: string | null): string => {
  const osValue = (os || "").toLowerCase();
  const archValue = (arch || "").toLowerCase();
  if (osValue === "macos" && archValue === "arm64") return "Apple Silicon";
  if (osValue === "macos" && archValue === "x86_64") return "Intel";
  if (archValue === "x86_64") return "x64";
  if (archValue === "arm64") return "ARM64";
  return arch || "-";
};

const mapDownloadState = (row: FfmpegVersionItem): string => {
  if (row.isActive) return "Active";
  if (row.downloadState === "downloaded") return "Downloaded";
  if (row.downloadState === "downloading") return "Downloading";
  if (row.downloadState === "failed") return "Failed";
  return "Not downloaded";
};

const mapDbRowToView = (row: FfmpegVersionItem): FfmpegRemoteVersion => ({
  rowKey: row.rowKey,
  source: row.source || "-",
  version: row.version,
  releaseDate: row.publishedAt?.slice(0, 10) || "-",
  osRaw: row.os || "",
  archRaw: row.arch || "",
  arch: formatArchLabel(row.os, row.arch),
  state: mapDownloadState(row),
  downloadState: row.downloadState,
});

const mergeInstalledWithPending = (
  rows: FfmpegVersionItem[],
  prev: FfmpegVersionItem[],
  progress: Record<string, number>,
): FfmpegVersionItem[] => {
  const rowKeySet = new Set(rows.map((item) => item.rowKey));
  const pendingFromPrev = prev.filter(
    (item) =>
      progress[item.rowKey] !== undefined &&
      !rowKeySet.has(item.rowKey),
  );
  return [...rows, ...pendingFromPrev];
};

export const useFfmpegStore = create<FfmpegStore>()(
  persist(
    (set, get) => ({
      remoteVersions: [],
      installedVersions: [],
      sourceMessage: null,
      runtimeVersion: "unknown",
      runtimeExecutablePath: null,
      busyRowKey: null,
      cancelingRowKey: null,
      downloadProgress: {},
      ensureStage: "idle",
      ensureMessage: "",
      ensureTargetRowKey: null,
      lastSyncedAt: null,

      init: async () => {
        try {
          if (!get().sourceMessage) {
            set({ sourceMessage: { key: "source.loading" } });
          }
          await get().refreshAll();
          await get().refreshRuntime();
        } catch (error) {
          console.error("init ffmpeg store failed:", error);
          set({ sourceMessage: { key: "source.query_failed" } });
        }
      },

      refreshRuntime: async () => {
        try {
          const runtime = await bridge.getCurrentFfmpegRuntimeInfo();
          set({
            runtimeVersion: runtime.version || "unknown",
            runtimeExecutablePath: runtime.executablePath ?? null,
          });
        } catch {
          set({ runtimeVersion: "unknown", runtimeExecutablePath: null });
        }
      },

      loadInstalled: async () => {
        const rows = await bridge.listInstalledFfmpegVersions();
        set((state) => ({
          installedVersions: mergeInstalledWithPending(
            rows,
            state.installedVersions,
            state.downloadProgress,
          ),
        }));
      },

      loadRemote: async () => {
        const page = await bridge.listFfmpegVersions({ limit: 50, offset: 0 });
        set({
          remoteVersions: page.list.map(mapDbRowToView),
          sourceMessage: {
            key: "source.loaded_count",
            values: { count: page.list.length },
          },
        });
      },

      refreshAll: async () => {
        await Promise.all([get().loadRemote(), get().loadInstalled()]);
        set({ lastSyncedAt: Date.now() });
      },

      downloadVersion: async (rowKey: string, options?: DownloadOptions) => {
        if (get().downloadProgress[rowKey] !== undefined) return;

        const remote =
          get().remoteVersions.find((item) => item.rowKey === rowKey) ||
          (await bridge
            .listFfmpegVersions({ limit: 50, offset: 0 })
            .then((page) =>
              page.list
                .map(mapDbRowToView)
                .find((item) => item.rowKey === rowKey),
            ));

        if (!remote) {
          if (!options?.silentError) {
            throw new Error("target version not found");
          }
          return;
        }

        set((state) => {
          const optimistic: FfmpegVersionItem = {
            rowKey: remote.rowKey,
            source: remote.source,
            os: remote.osRaw,
            version: remote.version,
            publishedAt: remote.releaseDate === "-" ? null : remote.releaseDate,
            downloadUrl: null,
            arch: remote.archRaw || null,
            localPath: null,
            updatedAt: Date.now(),
            downloadState: "downloading",
            installed: false,
            isActive: false,
          };
          const idx = state.installedVersions.findIndex(
            (item) => item.rowKey === rowKey,
          );
          return {
            installedVersions:
              idx === -1
                ? [...state.installedVersions, optimistic]
                : state.installedVersions.map((item) =>
                    item.rowKey === rowKey
                      ? { ...item, downloadState: "downloading" }
                      : item,
                  ),
            downloadProgress: { ...state.downloadProgress, [rowKey]: 0 },
            ensureStage: options?.ensureFlow
              ? "downloading"
              : state.ensureStage,
            ensureMessage: options?.ensureFlow
              ? `downloading FFmpeg ${remote.version}`
              : state.ensureMessage,
            ensureTargetRowKey: options?.ensureFlow
              ? rowKey
              : state.ensureTargetRowKey,
          };
        });

        try {
          await bridge.downloadFfmpegVersion(rowKey, (payload) => {
            const percent = Number.isFinite(payload.percent)
              ? Math.max(0, Math.min(100, Math.round(payload.percent)))
              : 0;
            set((state) => ({
              downloadProgress: {
                ...state.downloadProgress,
                [rowKey]: percent,
              },
              ensureMessage:
                options?.ensureFlow && state.ensureTargetRowKey === rowKey
                  ? `\u6b63\u5728\u4e0b\u8f7d FFmpeg ${percent}%`
                  : state.ensureMessage,
            }));
          });

          if (options?.autoActivate) {
            set((state) => ({
              ensureStage: options?.ensureFlow
                ? "activating"
                : state.ensureStage,
              ensureMessage: options?.ensureFlow
                ? "download completed, activating..."
                : state.ensureMessage,
            }));
            await bridge.activateFfmpegVersion(rowKey);
          }

          await Promise.all([get().refreshAll(), get().refreshRuntime()]);
          set((state) => ({
            ensureStage: options?.ensureFlow ? "ready" : state.ensureStage,
            ensureMessage: options?.ensureFlow
              ? "FFmpeg is ready"
              : state.ensureMessage,
            ensureTargetRowKey: options?.ensureFlow
              ? null
              : state.ensureTargetRowKey,
          }));
        } catch (error) {
          const reason =
            error instanceof Error
              ? error.message
              : String(error ?? "unknown error");
          const canceled = reason.toLowerCase().includes("cancel");
          set((state) => {
            const nextProgress = { ...state.downloadProgress };
            delete nextProgress[rowKey];
            return {
              downloadProgress: nextProgress,
              installedVersions: canceled
                ? state.installedVersions.filter(
                    (item) =>
                      !(item.rowKey === rowKey && !item.installed && !item.isActive),
                  )
                : state.installedVersions.map((item) => {
                    if (item.rowKey !== rowKey) return item;
                    if (item.isActive || item.installed) {
                      return { ...item, downloadState: "downloaded" };
                    }
                    return { ...item, downloadState: "failed" };
                  }),
            };
          });
          if (!canceled) {
            await bridge.updateFfmpegDownloadState(rowKey, "failed");
          }
          await get().refreshAll();
          if (options?.ensureFlow) {
            set({
              ensureStage: canceled ? "idle" : "error",
              ensureMessage: canceled
                ? "download canceled"
                : `download failed: ${reason}`,
              ensureTargetRowKey: canceled ? null : get().ensureTargetRowKey,
            });
          }
          if (!options?.silentError && !canceled) {
            throw new Error(`download failed: ${reason}`);
          }
        } finally {
          set((state) => {
            const next = { ...state.downloadProgress };
            delete next[rowKey];
            return { downloadProgress: next };
          });
        }
      },

      cancelDownload: async (rowKey: string) => {
        set({ cancelingRowKey: rowKey });
        try {
          await bridge.cancelFfmpegDownload(rowKey);
          set((state) => ({
            installedVersions: state.installedVersions.filter(
              (item) =>
                !(item.rowKey === rowKey && !item.installed && !item.isActive),
            ),
            ensureStage:
              state.ensureStage === "downloading" &&
              state.ensureTargetRowKey === rowKey
                ? "idle"
                : state.ensureStage,
            ensureMessage:
              state.ensureStage === "downloading" &&
              state.ensureTargetRowKey === rowKey
                ? "download canceled"
                : state.ensureMessage,
            ensureTargetRowKey:
              state.ensureStage === "downloading" &&
              state.ensureTargetRowKey === rowKey
                ? null
                : state.ensureTargetRowKey,
          }));
          await get().refreshAll();
        } finally {
          set((state) => {
            const next = { ...state.downloadProgress };
            delete next[rowKey];
            return { cancelingRowKey: null, downloadProgress: next };
          });
        }
      },

      activateVersion: async (
        rowKey: string,
        options?: { quietEnsure?: boolean },
      ) => {
        set((state) => ({
          busyRowKey: rowKey,
          ensureStage:
            state.ensureStage !== "ready" && !options?.quietEnsure
              ? "activating"
              : state.ensureStage,
          ensureMessage:
            state.ensureStage !== "ready" && !options?.quietEnsure
              ? "activating FFmpeg..."
              : state.ensureMessage,
        }));
        try {
          await bridge.activateFfmpegVersion(rowKey);
          await Promise.all([get().refreshAll(), get().refreshRuntime()]);
          if (!options?.quietEnsure) {
            set({ ensureStage: "ready", ensureMessage: "FFmpeg is ready" });
          }
        } catch (error) {
          if (!options?.quietEnsure) {
            const reason =
              error instanceof Error
                ? error.message
                : String(error ?? "unknown error");
            set({
              ensureStage: "error",
              ensureMessage: `activate failed: ${reason}`,
            });
          }
          throw error;
        } finally {
          set({ busyRowKey: null });
        }
      },

      deleteVersion: async (rowKey: string) => {
        set({ busyRowKey: rowKey });
        try {
          await bridge.deleteFfmpegVersion(rowKey);
          await Promise.all([get().refreshAll(), get().refreshRuntime()]);
        } finally {
          set({ busyRowKey: null });
        }
      },

      ensureReadyForHome: async () => {
        const stage = get().ensureStage;
        if (["ready", "checking", "downloading", "activating"].includes(stage)) {
          return;
        }

        set({
          ensureStage: "checking",
          ensureMessage: "checking...",
          ensureTargetRowKey: null,
        });

        try {
          await get().refreshRuntime();
          const installed = await bridge.listInstalledFfmpegVersions();
          set((state) => ({
            installedVersions: mergeInstalledWithPending(
              installed,
              state.installedVersions,
              state.downloadProgress,
            ),
          }));

          if (installed.length > 0) {
            const active = installed.find((item) => item.isActive);
            const first = active || installed[0];
            if (!active && first) {
              await get().activateVersion(first.rowKey);
            } else {
              await Promise.all([get().refreshAll(), get().refreshRuntime()]);
              set({ ensureStage: "ready", ensureMessage: "ready" });
            }
            return;
          }

          await get().loadRemote();
          const downloadingInstalled = get().installedVersions.find(
            (item) => item.downloadState === "downloading",
          );
          const downloadingRowKey =
            Object.keys(get().downloadProgress)[0] ||
            downloadingInstalled?.rowKey ||
            null;
          if (downloadingRowKey) {
            set({
              ensureStage: "downloading",
              ensureMessage: "downloading FFmpeg...",
              ensureTargetRowKey: downloadingRowKey,
            });
            return;
          }

          const firstRemote = get().remoteVersions[0];
          if (!firstRemote) {
            set({
              ensureStage: "error",
              ensureMessage: "no usable remote FFmpeg version",
            });
            return;
          }

          await get().downloadVersion(firstRemote.rowKey, {
            autoActivate: true,
            ensureFlow: true,
            silentError: true,
          });
        } catch (error) {
          const reason =
            error instanceof Error
              ? error.message
              : String(error ?? "unknown error");
          set({
            ensureStage: "error",
            ensureMessage: `FFmpeg failed: ${reason}`,
          });
        }
      },
    }),
    {
      name: "ffmpeg_store",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        remoteVersions: state.remoteVersions,
        installedVersions: state.installedVersions,
        sourceMessage: state.sourceMessage,
        runtimeVersion: state.runtimeVersion,
        runtimeExecutablePath: state.runtimeExecutablePath,
        lastSyncedAt: state.lastSyncedAt,
      }),
    },
  ),
);
