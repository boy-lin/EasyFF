import { useMemo } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { ChevronLeft } from "lucide-react";
import { Theme } from "@/components/ui/theme";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";
import { UserMenu } from "@/components/auth/UserMenu";
import { Button } from "@/components/ui/button";
import { SIDEBAR_NAV_ITEMS_CONFIG } from "@/config/app-navigation";
import OnlineHelpDialog from "./OnlineHelpDialog";

export default function Header() {
  const location = useLocation();
  const navigate = useNavigate();

  const sidebarRoutes = useMemo(
    () =>
      SIDEBAR_NAV_ITEMS_CONFIG.filter((item) => !!item.href && !item.disabled)
        .map((item) => item.href as string),
    [],
  );

  const isSidebarRoute = useMemo(() => {
    const pathname = location.pathname;
    return sidebarRoutes.some((route) => {
      if (route === "/") return pathname === "/";
      return pathname === route || pathname.startsWith(`${route}/`);
    });
  }, [location.pathname, sidebarRoutes]);

  const showBackButton = !isSidebarRoute;

  const handleBack = () => {
    if (window.history.length > 1) {
      navigate(-1);
      return;
    }
    navigate("/", { replace: true });
  };

  return (
    <header className="bg-background px-4 py-2 flex items-center justify-between gap-3">
      <div>
        {showBackButton ? (
          <Button size="sm" variant="ghost" onClick={handleBack} className="h-8 px-2">
            <ChevronLeft className="h-4 w-4" />
          </Button>
        ) : null}
      </div>
      <div className="flex items-center gap-3">
        <Theme
          size="sm"
          variant="dropdown"
          themes={["light", "dark", "system"]}
          className="cursor-pointer border-transparent bg-secondary px-[9px] py-[9px] h-auto"
        />
        <LanguageSwitcher />
        <OnlineHelpDialog />
        <UserMenu />
      </div>
    </header>
  );
}
