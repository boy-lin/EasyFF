import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Loader2, LogOut } from "lucide-react";
import ProfileLinear from "@/components/icons/ProfileLinear";
import { AuthDialog } from "@/components/auth/AuthDialog";
import { Badge } from "@/components/ui/badge";
import { signOut } from "@/lib/auth-client";
import { clearDesktopToken, hasDesktopAccessToken } from "@/lib/desktop-auth";
import { useFavoriteSyncStore } from "@/stores/favoriteSyncStore";
import { useUserStore } from "@/stores/user";
import { toast } from "sonner";
import { analytics } from "@/lib/analytics";
import { useTranslation } from "react-i18next";

export const UserMenu = () => {
  const { t } = useTranslation("common");
  const { isLoading, userInfo, isTokenPreview, isProfileRefreshing, fetchUserInfo, clearUser } = useUserStore();
  const syncFavoriteNow = useFavoriteSyncStore((s) => s.syncNow);
  const resetFavoriteSyncState = useFavoriteSyncStore((s) => s.resetSyncState);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [desktopLoggedIn, setDesktopLoggedIn] = useState(hasDesktopAccessToken());
  const prevRefreshingRef = useRef(false);
  const isLoggedIn =  desktopLoggedIn;
  const displayName = useMemo(
    () => userInfo?.name  || t("auth.user_menu.default_user"),
    [userInfo]
  );
  useEffect(() => {
    if (isLoggedIn) {
      fetchUserInfo().catch((e: any) => {
        toast.error(e.message || t("auth.user_menu.toast.fetch_user_failed"));
      });
    }
  }, [isLoggedIn, fetchUserInfo]);

  useEffect(() => {
    setDesktopLoggedIn(hasDesktopAccessToken());
  }, [dialogOpen]);

  useEffect(() => {
    const handleDesktopAuthSuccess = () => {
      setDesktopLoggedIn(true);
      setDialogOpen(false);
      toast.success(t("auth.user_menu.toast.desktop_login_success"));
    };

    const handleDesktopAuthError = (event: Event) => {
      const detail = (event as CustomEvent<{ message?: string }>).detail;
      toast.error(detail?.message || t("auth.user_menu.toast.desktop_login_failed"));
    };

    window.addEventListener("desktop-auth:success", handleDesktopAuthSuccess);
    window.addEventListener("desktop-auth:error", handleDesktopAuthError as EventListener);
    return () => {
      window.removeEventListener("desktop-auth:success", handleDesktopAuthSuccess);
      window.removeEventListener("desktop-auth:error", handleDesktopAuthError as EventListener);
    };
  }, []);

  useEffect(() => {
    if (!isLoggedIn) {
      prevRefreshingRef.current = false;
      return;
    }
    if (isProfileRefreshing) {
      prevRefreshingRef.current = true;
      return;
    }
    if (prevRefreshingRef.current) {
      prevRefreshingRef.current = false;
      if (!isTokenPreview) {
        toast.success(t("auth.user_menu.toast.profile_synced"));
      }
    }
  }, [isLoggedIn, isProfileRefreshing, isTokenPreview, t]);

  useEffect(() => {
    if (userInfo?.id) {
      analytics.identify(userInfo.id, {
        email: userInfo.email,
        name: userInfo.name,
      });
    }
  }, [userInfo?.id, userInfo?.email, userInfo?.name]);

  useEffect(() => {
    if (!isLoggedIn || !hasDesktopAccessToken()) return;
    syncFavoriteNow({ silent: true }).catch(() => undefined);
  }, [isLoggedIn, syncFavoriteNow]);

  const handleLogout = async () => {
    try {
      if (userInfo) {
        await signOut();
      }
      analytics.reset();
      clearDesktopToken();
      setDesktopLoggedIn(false);
      clearUser();
      resetFavoriteSyncState();
      toast.success(t("auth.user_menu.toast.signed_out"));
    } catch (e) {
      toast.error(t("auth.user_menu.toast.sign_out_failed"));
    }
  };

  if (isLoading) {
    return (
      <Button variant="secondary" size="icon" className="shadow-none" disabled>
        <Loader2 className="w-4 h-4 animate-spin" />
      </Button>
    );
  }

  if (!isLoggedIn) {
    return (
      <>
        <Button
          variant="secondary"
          size="sm"
          className="cursor-pointer shadow-none font-medium px-3"
          onClick={() => setDialogOpen(true)}
        >
          {t("auth.user_menu.actions.login")}
        </Button>
        <AuthDialog
          open={dialogOpen}
          onOpenChange={setDialogOpen}
          onSuccess={() => {
            setDialogOpen(false);
            fetchUserInfo();
          }}
        />
      </>
    );
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="secondary" size="icon" className="cursor-pointer shadow-none">
          <ProfileLinear className="w-5 h-5" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuLabel className="flex flex-col">
          <span className="font-semibold text-foreground">{displayName}</span>
          {(userInfo?.email) && (
            <span className="text-xs text-muted-foreground">
              {userInfo?.email}
            </span>
          )}
          {isProfileRefreshing && (
            <Badge variant="outline" className="mt-2 w-fit text-[10px] px-1.5 py-0.5">
              {t("auth.user_menu.status.syncing_profile")}
            </Badge>
          )}
          {!isProfileRefreshing && isTokenPreview && (
            <Badge variant="secondary" className="mt-2 w-fit text-[10px] px-1.5 py-0.5">
              {t("auth.user_menu.status.basic_profile_mode")}
            </Badge>
          )}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          className="text-red-600 focus:text-red-700"
          onClick={handleLogout}
        >
          <LogOut className="w-4 h-4 mr-2" />
          {t("auth.user_menu.actions.logout")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
};

export default UserMenu;
