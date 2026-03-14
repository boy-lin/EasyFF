import React, { Suspense, lazy } from "react";
import { createHashRouter, Outlet } from "react-router-dom";
import { ListOrdered } from "lucide-react";
import i18n from "@/lib/i18n";
import ErrorPage from "@/components/error/ErrorPage";
import HomeLinear from "@/components/icons/HomeLinear";
import FolderLinear from "@/components/icons/FolderLinear";

const RootLayout = lazy(() => import("@/layout/RootPage"));
const AuthLayout = lazy(() => import("@/layout/AuthLayout"));
const HomePage = lazy(() => import("@/pages/home"));
const TaskHistoryPage = lazy(() => import("@/pages/tasks"));
const FavoriteCommandsPage = lazy(() => import("@/pages/favorite-commands"));
const FFmpegVersionManagerPage = lazy(() => import("@/pages/ffmpeg"));
const ForceUpdatePage = lazy(() => import("@/pages/force-update"));

const preloadI18nNamespaces = (namespaces: string[]) => async () => {
  await i18n.loadNamespaces(namespaces);
  return null;
};

const hydrateFallbackElement = (
  <div className="loader-wrapper">
    <div className="loader"></div>
  </div>
);

const withSuspense = (element: React.ReactNode) => (
  <Suspense fallback={hydrateFallbackElement}>{element}</Suspense>
);

export const MENU_ITEMS = {
  home: "/",
  favoriteCommands: "/favorite/commands",
  tasks: "/tasks",
  ffmpegVersionManager: "/ffmpeg/version-manager",
} as const;

export type QuickAccessItem = {
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  color: string;
  activeGradient?: string;
  href?: string;
};

export type SidebarNavConfigItem = {
  labelKey: string;
  icon: React.ComponentType<{ className?: string }>;
  href?: string;
  disabled?: boolean;
  badgeKey?: "unreadFinishedCount";
};

export const SIDEBAR_NAV_ITEMS_CONFIG: SidebarNavConfigItem[] = [
  { labelKey: "nav.home", icon: HomeLinear, href: MENU_ITEMS.home },
  {
    labelKey: "nav.tasks",
    icon: ListOrdered,
    href: MENU_ITEMS.tasks,
    badgeKey: "unreadFinishedCount",
  },
  { labelKey: "nav.favorite_commands", icon: FolderLinear, href: MENU_ITEMS.favoriteCommands },
  // { labelKey: "nav.ai_tools", icon: AILinear, disabled: true },
];

export const QUICK_ACCESS_CONFIG: QuickAccessItem[] = [
];

export const appRouter = createHashRouter([
  {
    path: "/",
    element: withSuspense(<AuthLayout />),
    errorElement: <ErrorPage />,
    hydrateFallbackElement,
    children: [
      {
        path: "/",
        element: withSuspense(<RootLayout />),
        children: [
          {
            index: true,
            element: withSuspense(<HomePage />),
            loader: preloadI18nNamespaces(["home"]),
          },
          {
            path: "favorite",
            loader: preloadI18nNamespaces(["ffmpeg"]),
            children: [{ path: "commands", element: withSuspense(<FavoriteCommandsPage />) }],
          },
          {
            path: "tasks",
            element: <Outlet />,
            loader: preloadI18nNamespaces(["tasks"]),
            children: [{ index: true, element: withSuspense(<TaskHistoryPage />) }],
          },
          {
            path: "ffmpeg",
            children: [
              {
                path: "version-manager",
                element: withSuspense(<FFmpegVersionManagerPage />),
              },
            ],
          },
        ],
      },
    ],
  },
  {
    path: "/force-update",
    element: withSuspense(<ForceUpdatePage />),
    errorElement: <ErrorPage />,
    hydrateFallbackElement,
    loader: preloadI18nNamespaces(["common"]),
  },
]);

