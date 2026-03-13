import { useEffect, useRef } from "react";
import { hasDesktopAccessToken } from "@/lib/desktop-auth";
import { useFavoriteSyncStore } from "@/stores/favoriteSyncStore";

export function FavoriteSyncBootstrap() {
  const syncNow = useFavoriteSyncStore((s) => s.syncNow);
  const initializedRef = useRef(false);

  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;

    if (hasDesktopAccessToken()) {
      syncNow({ silent: true }).catch(() => undefined);
    }
  }, [syncNow]);

  useEffect(() => {
    const handleOnline = () => {
      syncNow({ silent: true }).catch(() => undefined);
    };
    const handleDesktopAuthSuccess = () => {
      syncNow({ silent: true }).catch(() => undefined);
    };

    window.addEventListener("online", handleOnline);
    window.addEventListener("desktop-auth:success", handleDesktopAuthSuccess);
    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("desktop-auth:success", handleDesktopAuthSuccess);
    };
  }, [syncNow]);

  return null;
}

export default FavoriteSyncBootstrap;
