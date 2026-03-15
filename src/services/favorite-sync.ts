import { bridge, type FavoriteCommandItem, type FavoriteSyncAck } from "@/lib/bridge";
import { getDesktopAccessToken } from "@/lib/desktop-auth";
import { baseApiUrl } from "@/lib/env";

type ApiResponse<T> = {
  code: number;
  message: string;
  data?: T;
};

type FavoriteChangePayload = {
  changes: Array<Record<string, unknown>>;
  next_cursor: number;
  has_more: boolean;
};

type ServerFavoriteItem = {
  id: string;
  title: string;
  description?: string | null;
  command: string;
  created_at?: number;
  updated_at?: number;
  deleted_at?: number | null;
  sync_state?: "synced" | "pending_upsert" | "pending_delete" | "conflict";
  server_version?: number | null;
  updated_by_device_id?: string | null;
  createdAt?: number;
  updatedAt?: number;
  deletedAt?: number | null;
  syncState?: "synced" | "pending_upsert" | "pending_delete" | "conflict";
  serverVersion?: number | null;
  updatedByDeviceId?: string | null;
};

const DEFAULT_BATCH_SIZE = 200;
const MAX_RETRY_TIMES = 5;
const RETRY_BASE_DELAY_MS = 1000;

function logFavoriteSync(message: string, meta?: Record<string, unknown>) {
  if (meta) {
    console.info(`[favorite-sync] ${message} ${JSON.stringify(meta)}`);
    return;
  }
  console.info(`[favorite-sync] ${message}`);
}

function assertApiBase() {
  if (!baseApiUrl) {
    throw new Error("VITE_BASE_API_URL is not configured");
  }
}

function buildAuthHeaders(): HeadersInit {
  const accessToken = getDesktopAccessToken();
  return accessToken ? { Authorization: `Bearer ${accessToken}` } : {};
}

async function postApi<T>(path: string, body: Record<string, unknown>): Promise<T> {
  assertApiBase();
  logFavoriteSync("request:start", { path, body });
  const response = await fetch(`${baseApiUrl}${path}`, {
    method: "POST",
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...buildAuthHeaders(),
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    logFavoriteSync("request:http_error", { path, status: response.status });
    throw new Error(`favorite sync request failed: ${response.status}`);
  }

  const json = (await response.json()) as ApiResponse<T>;
  if (json.code !== 0) {
    logFavoriteSync("request:api_error", { path, code: json.code, message: json.message });
    throw new Error(json.message || "favorite sync api failed");
  }
  logFavoriteSync("request:success", { path, code: json.code });
  return json.data as T;
}

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function isRetryableError(error: unknown): boolean {
  const message = error instanceof Error ? error.message.toLowerCase() : String(error).toLowerCase();
  if (message.includes("favorite sync request failed: 4")) return false;
  if (message.includes("no auth")) return false;
  return true;
}

async function postApiWithRetry<T>(path: string, body: Record<string, unknown>): Promise<T> {
  let attempt = 0;
  let lastError: unknown = null;

  while (attempt < MAX_RETRY_TIMES) {
    try {
      return await postApi<T>(path, body);
    } catch (error) {
      lastError = error;
      attempt += 1;
      if (attempt >= MAX_RETRY_TIMES || !isRetryableError(error)) {
        throw error;
      }
      const delay = RETRY_BASE_DELAY_MS * Math.pow(2, Math.min(2, attempt - 1));
      await sleep(delay);
    }
  }

  throw (lastError instanceof Error ? lastError : new Error("favorite sync failed"));
}

function toServerItem(item: FavoriteCommandItem) {
  return {
    id: item.id,
    title: item.title,
    description: item.description ?? undefined,
    command: item.command,
    created_at: item.createdAt,
    updated_at: item.updatedAt,
    deleted_at: item.deletedAt ?? null,
    base_version: item.serverVersion ?? null,
  };
}

function toServerAck(item: FavoriteCommandItem): FavoriteSyncAck {
  return {
    id: item.id,
    serverVersion: item.serverVersion ?? null,
    updatedAt: item.updatedAt,
    deletedAt: item.deletedAt ?? null,
    updatedByDeviceId: item.updatedByDeviceId ?? null,
  };
}

function toLocalFavoriteItem(raw: ServerFavoriteItem): FavoriteCommandItem {
  return {
    id: raw.id,
    title: raw.title,
    description: raw.description ?? null,
    command: raw.command,
    createdAt: Number(raw.createdAt ?? raw.created_at ?? Date.now()),
    updatedAt: Number(raw.updatedAt ?? raw.updated_at ?? Date.now()),
    deletedAt: (raw.deletedAt ?? raw.deleted_at ?? null) as number | null,
    syncState: (raw.syncState ?? raw.sync_state ?? "synced") as FavoriteCommandItem["syncState"],
    serverVersion: (raw.serverVersion ?? raw.server_version ?? null) as number | null,
    updatedByDeviceId: (raw.updatedByDeviceId ?? raw.updated_by_device_id ?? null) as string | null,
  };
}

export async function pushFavoriteCommands(deviceId: string): Promise<number> {
  const pending = await bridge.listPendingFavoriteCommandSync(1000);
  logFavoriteSync("push:pending_loaded", {
    deviceId,
    pendingCount: pending.length,
    pendingIds: pending.map((item) => item.id),
  });
  if (!pending.length) return 0;

  const sent = await postApiWithRetry<{
    accepted?: Array<{
      id: string;
      version?: number;
      updated_at?: number;
      deleted_at?: number | null;
      updated_by_device_id?: string;
    }>;
  }>("/api/app/favorite/commands/upsert", {
    device_id: deviceId,
    items: pending.map(toServerItem),
  });

  const acks: FavoriteSyncAck[] =
    sent?.accepted && sent.accepted.length > 0
      ? sent.accepted.map((row) => ({
          id: row.id,
          serverVersion: row.version ?? null,
          updatedAt: row.updated_at ?? null,
          deletedAt: row.deleted_at ?? null,
          updatedByDeviceId: row.updated_by_device_id ?? null,
        }))
      : pending.map(toServerAck);

  logFavoriteSync("push:ack_received", {
    ackCount: acks.length,
    ackIds: acks.map((item) => item.id),
  });
  await bridge.markFavoriteCommandsSynced(acks);
  logFavoriteSync("push:mark_synced_done", { ackCount: acks.length });
  return acks.length;
}

export async function pullFavoriteCommandsChanges(): Promise<number> {
  let cursor = await bridge.getFavoriteCommandSyncCursor();
  let total = 0;
  logFavoriteSync("pull:start", { cursor });

  while (true) {
    const data = await postApiWithRetry<FavoriteChangePayload>("/api/app/favorite/commands/changes", {
      cursor,
      limit: DEFAULT_BATCH_SIZE,
    });

    const changes = (data?.changes || []).map((row) => toLocalFavoriteItem(row as ServerFavoriteItem));
    logFavoriteSync("pull:batch_loaded", {
      cursor,
      nextCursor: Number(data?.next_cursor || cursor),
      hasMore: Boolean(data?.has_more),
      changeCount: changes.length,
      changeIds: changes.map((item) => item.id),
      deletedIds: changes.filter((item) => item.deletedAt != null).map((item) => item.id),
    });
    if (changes.length > 0) {
      await bridge.applyRemoteFavoriteCommandChanges(changes);
      total += changes.length;
      logFavoriteSync("pull:batch_applied", {
        appliedCount: changes.length,
        totalApplied: total,
      });
    }

    const nextCursor = Number(data?.next_cursor || cursor);
    if (nextCursor > cursor) {
      await bridge.setFavoriteCommandSyncCursor(nextCursor);
      cursor = nextCursor;
      logFavoriteSync("pull:cursor_updated", { cursor });
    }

    if (!data?.has_more) {
      break;
    }
  }

  logFavoriteSync("pull:done", { total });
  return total;
}

export async function syncFavoriteCommandsNow(): Promise<{ pushed: number; pulled: number }> {
  const deviceId = await bridge.getDeviceId();
  logFavoriteSync("sync:start", { deviceId });
  const pushed = await pushFavoriteCommands(deviceId);
  const pulled = await pullFavoriteCommandsChanges();
  logFavoriteSync("sync:done", { deviceId, pushed, pulled });
  return { pushed, pulled };
}
