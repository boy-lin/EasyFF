import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ColumnDef,
  flexRender,
  getCoreRowModel,
  type SortingState,
  useReactTable,
} from "@tanstack/react-table";
import { ArrowUpDown, Search, Sparkles, Trash2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { bridge, type FavoriteCommandItem } from "@/lib/bridge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { EllipsisName } from "@/components/ui-lab/ellipsis-name";
import { AuthDialog } from "@/components/auth/AuthDialog";
import { useFavoriteSyncStore } from "@/stores/favoriteSyncStore";
import { hasDesktopAccessToken } from "@/lib/desktop-auth";

const PAGE_SIZE = 5;
const QUERY_DEBOUNCE_MS = 350;
const SEARCH_FETCH_LIMIT = 5000;

function formatDateTime(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return "-";
  return new Date(ts).toLocaleString();
}

function sortFavoriteCommands(list: FavoriteCommandItem[], sorting: SortingState): FavoriteCommandItem[] {
  if (!sorting.length) return list;
  const [{ id, desc }] = sorting;
  const factor = desc ? -1 : 1;

  return [...list].sort((a, b) => {
    let left: string | number = "";
    let right: string | number = "";

    if (id === "title") {
      left = a.title || "";
      right = b.title || "";
    } else if (id === "updatedAt") {
      left = a.updatedAt || 0;
      right = b.updatedAt || 0;
    } else if (id === "createdAt") {
      left = a.createdAt || 0;
      right = b.createdAt || 0;
    }

    if (typeof left === "number" && typeof right === "number") {
      return (left - right) * factor;
    }

    return String(left).localeCompare(String(right)) * factor;
  });
}

function filterFavoriteCommands(list: FavoriteCommandItem[], keyword: string): FavoriteCommandItem[] {
  const q = keyword.trim().toLowerCase();
  if (!q) return list;

  return list.filter((item) => {
    const text = [item.title, item.description || "", item.command].join(" ").toLowerCase();
    return text.includes(q);
  });
}

export default function FavoriteCommandsPage() {
  const { t } = useTranslation("ffmpeg");
  const navigate = useNavigate();
  const syncFavoriteNow = useFavoriteSyncStore((s) => s.syncNow);
  const scheduleFavoriteSync = useFavoriteSyncStore((s) => s.scheduleSync);
  const syncing = useFavoriteSyncStore((s) => s.syncing);
  const lastSyncAt = useFavoriteSyncStore((s) => s.lastSyncAt);
  const syncError = useFavoriteSyncStore((s) => s.syncError);
  const clearSyncError = useFavoriteSyncStore((s) => s.clearSyncError);

  const [items, setItems] = useState<FavoriteCommandItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [queryInput, setQueryInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [sorting, setSorting] = useState<SortingState>([{ id: "updatedAt", desc: true }]);
  const [page, setPage] = useState(0);
  const [hasNextPage, setHasNextPage] = useState(false);
  const [total, setTotal] = useState(0);
  const [reloadKey, setReloadKey] = useState(0);
  const [authDialogOpen, setAuthDialogOpen] = useState(false);

  const requestSeqRef = useRef(0);

  const loadPageData = useCallback(async () => {
    const seq = ++requestSeqRef.current;
    setLoading(true);

    try {
      if (keyword.trim()) {
        const allRows = await bridge.listFavoriteCommands(SEARCH_FETCH_LIMIT, 0);
        const filtered = filterFavoriteCommands(allRows, keyword);
        const sorted = sortFavoriteCommands(filtered, sorting);
        const start = page * PAGE_SIZE;
        const end = start + PAGE_SIZE;

        if (requestSeqRef.current !== seq) return;

        setItems(sorted.slice(start, end));
        setHasNextPage(end < sorted.length);
        setTotal(sorted.length);
        return;
      }

      const offset = page * PAGE_SIZE;
      const rows = await bridge.listFavoriteCommands(PAGE_SIZE + 1, offset);

      if (requestSeqRef.current !== seq) return;

      const pageRows = rows.slice(0, PAGE_SIZE);
      setItems(sortFavoriteCommands(pageRows, sorting));
      setHasNextPage(rows.length > PAGE_SIZE);
      setTotal(offset + pageRows.length + (rows.length > PAGE_SIZE ? 1 : 0));
    } catch (error) {
      if (requestSeqRef.current !== seq) return;
      const message = error instanceof Error ? error.message : t("favoriteCommandsPage.toast.load_failed");
      toast.error(message);
    } finally {
      if (requestSeqRef.current === seq) {
        setLoading(false);
      }
    }
  }, [keyword, page, sorting]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setKeyword(queryInput.trim());
      setPage(0);
    }, QUERY_DEBOUNCE_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [queryInput]);

  useEffect(() => {
    loadPageData();
  }, [loadPageData, reloadKey]);

  useEffect(() => {
    let cancelled = false;
    syncFavoriteNow({ silent: true })
      .then((result) => {
        if (cancelled || !result) return;
        if (result.pushed > 0 || result.pulled > 0) {
          setReloadKey((prev) => prev + 1);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [syncFavoriteNow]);

  useEffect(() => {
    setPage(0);
  }, [sorting]);

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await bridge.deleteFavoriteCommand(id);
        scheduleFavoriteSync();
        toast.success(t("favoriteCommandsPage.toast.deleted"));

        if (items.length <= 1 && page > 0) {
          setPage((prev) => Math.max(0, prev - 1));
          return;
        }
        setReloadKey((prev) => prev + 1);
      } catch (error) {
        const message = error instanceof Error ? error.message : t("favoriteCommandsPage.toast.delete_failed");
        toast.error(message);
      }
    },
    [items.length, page, scheduleFavoriteSync, t],
  );

  const handleStartCreate = useCallback(
    (value: string) => {
      const text = value.trim();
      if (!text) {
        toast.warning(t("favoriteCommandsPage.toast.command_empty"));
        return;
      }
      navigate({
        pathname: "/",
        search: `?commandText=${encodeURIComponent(text)}`,
      });
    },
    [navigate, t],
  );


  const runManualSync = useCallback(async () => {
    try {
      const result = await syncFavoriteNow();
      if (!result) return;
      setReloadKey((prev) => prev + 1);
      toast.success(
        t("favoriteCommandsPage.toast.synced", { pushed: result.pushed, pulled: result.pulled }),
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : t("favoriteCommandsPage.toast.sync_failed");
      toast.error(message);
    }
  }, [syncFavoriteNow, t]);

  const handleManualSync = useCallback(async () => {
    if (!hasDesktopAccessToken()) {
      setAuthDialogOpen(true);
      return;
    }
    await runManualSync();
  }, [runManualSync]);

  const columns = useMemo<ColumnDef<FavoriteCommandItem>[]>(
    () => [
      {
        id: "index",
        header: "#",
        cell: ({ row }) => page * PAGE_SIZE + row.index + 1,
        enableSorting: false,
      },
      {
        accessorKey: "title",
        header: ({ column }) => (
          <Button
            variant="ghost"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            className="h-auto p-0"
          >
            {t("favoriteCommandsPage.table.title")}
            <ArrowUpDown className="ml-2 h-4 w-4" />
          </Button>
        ),
        cell: ({ row }) => <span className="font-medium"><EllipsisName name={row.original.title} /></span>,
      },
      {
        accessorKey: "description",
        header: t("favoriteCommandsPage.table.description"),
        cell: ({ row }) => (
          <span className="line-clamp-2 text-sm text-muted-foreground">
            {row.original.description ? <EllipsisName name={row.original.description} /> : "-"}
          </span>
        ),
        enableSorting: false,
      },
      {
        accessorKey: "command",
        header: t("favoriteCommandsPage.table.command"),
        cell: ({ row }) => (
          <EllipsisName name={row.original.command} startCount={20} />
        ),
        enableSorting: false,
      },
      {
        accessorKey: "updatedAt",
        header: ({ column }) => (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            className="h-auto p-0"
          >
            {t("favoriteCommandsPage.table.updated_at")}
            <ArrowUpDown className="ml-2 h-4 w-4" />
          </Button>
        ),
        cell: ({ row }) => formatDateTime(row.original.updatedAt),
      },
      {
        id: "actions",
        header: t("favoriteCommandsPage.table.actions"),
        cell: ({ row }) => {
          const item = row.original;
          return (
            <div className="flex flex-wrap items-center gap-2">
              <Button className="" size="sm" onClick={() => handleStartCreate(item.command)}>
                <Sparkles className="mr-1 h-4 w-4" />
                {t("favoriteCommandsPage.actions.use")}
              </Button>
              <Button size="sm" variant="destructive" onClick={() => handleDelete(item.id)}>
                <Trash2 className="mr-1 h-4 w-4" />
                {t("favoriteCommandsPage.actions.delete")}
              </Button>
            </div>
          );
        },
        enableSorting: false,
      },
    ],
    [handleDelete, handleStartCreate, page, t],
  );

  const table = useReactTable({
    data: items,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <>
      <Card className="h-full w-full  p-0 gap-2 border-none bg-transparent shadow-none">
        <CardHeader className="rounded-none px-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <CardTitle>{t("favoriteCommandsPage.title")}</CardTitle>
              <CardDescription className="opacity-0"></CardDescription>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" onClick={handleManualSync} disabled={syncing}>
                {syncing ? t("favoriteCommandsPage.actions.syncing") : t("favoriteCommandsPage.actions.sync_now")}
              </Button>
              <div className="relative w-72">
                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  placeholder={t("favoriteCommandsPage.search.placeholder")}
                  className="pl-9"
                  value={queryInput}
                  onChange={(e) => setQueryInput(e.target.value)}
                />
              </div>
            </div>
          </div>
          <div className="mt-2 flex items-center gap-3 text-xs text-muted-foreground">
            <span>{syncing ? t("favoriteCommandsPage.sync.syncing_favorites") : t("favoriteCommandsPage.sync.idle")}</span>
            {lastSyncAt ? (
              <span>{t("favoriteCommandsPage.sync.last_sync", { time: new Date(lastSyncAt).toLocaleString() })}</span>
            ) : null}
            {syncError ? (
              <button
                type="button"
                className="text-red-500 underline-offset-2 hover:underline"
                onClick={clearSyncError}
                title={syncError}
              >
                {t("favoriteCommandsPage.sync.error_click_dismiss")}
              </button>
            ) : null}
          </div>
        </CardHeader>

        <CardContent className="px-4 relative min-h-0 flex-1 overflow-auto">
          <Table className="min-w-[1100px]" wrapperClassName="rounded-lg border">
            <TableHeader className="bg-muted/50">
              {table.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <TableHead
                      key={header.id}
                      className={`px-4 py-3 font-medium ${header.column.id === "actions"
                          ? "sticky right-0 z-20 min-w-[180px] bg-muted/95 shadow-[-8px_0_8px_-8px_hsl(var(--border))]"
                          : ""
                        }`}
                    >
                      {header.isPlaceholder
                        ? null
                        : flexRender(header.column.columnDef.header, header.getContext())}
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>

            <TableBody>
              {table.getRowModel().rows.map((row) => (
                <TableRow key={row.id} className="border-t">
                  {row.getVisibleCells().map((cell) => (
                    <TableCell
                      key={cell.id}
                      className={`px-4 py-3 align-middle ${cell.column.id === "actions"
                          ? "sticky right-0 z-10 min-w-[180px] bg-background shadow-[-8px_0_8px_-8px_hsl(var(--border))]"
                          : ""
                        }`}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))}

              {loading && items.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="py-8">
                    <div className="flex w-full items-center justify-center">
                      <div className="loader"></div>
                    </div>
                  </TableCell>
                </TableRow>
              )}

              {!loading && items.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="py-8 text-center text-muted-foreground">
                    {t("favoriteCommandsPage.table.no_data")}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>

          <div className="flex items-center justify-end space-x-2 py-4 pr-2">
            <span className="mr-2 text-sm text-muted-foreground">
              {keyword
                ? t("favoriteCommandsPage.pagination.page_with_matched", { page: page + 1, total })
                : t("favoriteCommandsPage.pagination.page", { page: page + 1 })}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((prev) => Math.max(0, prev - 1))}
              disabled={page === 0 || loading}
            >
              {t("favoriteCommandsPage.actions.prev")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((prev) => prev + 1)}
              disabled={!hasNextPage || loading}
            >
              {t("favoriteCommandsPage.actions.next")}
            </Button>
          </div>
        </CardContent>
      </Card>
      <AuthDialog
        open={authDialogOpen}
        onOpenChange={setAuthDialogOpen}
        onSuccess={() => {
          setAuthDialogOpen(false);
          runManualSync().catch(() => undefined);
        }}
      />
    </>
  );
}



