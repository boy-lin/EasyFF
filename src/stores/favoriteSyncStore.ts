import { create } from "zustand";
import { hasDesktopAccessToken } from "@/lib/desktop-auth";
import { syncFavoriteCommandsNow } from "@/services/favorite-sync";

type SyncResult = { pushed: number; pulled: number } | null;

type FavoriteSyncState = {
  syncing: boolean;
  lastSyncAt: number | null;
  syncError: string | null;
  syncNow: (options?: { silent?: boolean }) => Promise<SyncResult>;
  scheduleSync: (delayMs?: number) => void;
  clearSyncError: () => void;
  resetSyncState: () => void;
};

let syncTimer: number | null = null;
let inflightSync: Promise<SyncResult> | null = null;

export const useFavoriteSyncStore = create<FavoriteSyncState>((set, get) => ({
  syncing: false,
  lastSyncAt: null,
  syncError: null,

  syncNow: async (options) => {
    if (!hasDesktopAccessToken()) {
      console.info("[favorite-sync] syncNow:skip_no_token");
      return null;
    }

    if (inflightSync) {
      console.info("[favorite-sync] syncNow:reuse_inflight");
      return inflightSync;
    }

    console.info("[favorite-sync] syncNow:start", { silent: Boolean(options?.silent) });
    set({ syncing: true, syncError: null });
    inflightSync = (async () => {
      try {
        const result = await syncFavoriteCommandsNow();
        console.info("[favorite-sync] syncNow:success", result);
        set({
          syncing: false,
          lastSyncAt: Date.now(),
          syncError: null,
        });
        return result;
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "favorite sync failed";
        console.error("[favorite-sync] syncNow:error", { message, error });
        set({ syncing: false, syncError: message });
        if (!options?.silent) {
          throw error;
        }
        return null;
      } finally {
        inflightSync = null;
      }
    })();

    return inflightSync;
  },

  scheduleSync: (delayMs = 1200) => {
    if (syncTimer !== null) {
      window.clearTimeout(syncTimer);
    }
    console.info("[favorite-sync] scheduleSync", { delayMs });
    syncTimer = window.setTimeout(() => {
      syncTimer = null;
      console.info("[favorite-sync] scheduleSync:fire");
      get()
        .syncNow({ silent: true })
        .catch(() => undefined);
    }, Math.max(300, delayMs));
  },

  clearSyncError: () => {
    set({ syncError: null });
  },

  resetSyncState: () => {
    if (syncTimer !== null) {
      window.clearTimeout(syncTimer);
      syncTimer = null;
    }
    set({
      syncing: false,
      lastSyncAt: null,
      syncError: null,
    });
  },
}));
