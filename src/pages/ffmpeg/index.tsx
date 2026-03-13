import { useEffect, useMemo, useState } from "react";
import { ColumnDef, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { toast } from "sonner";
import { type FfmpegVersionItem } from "@/lib/bridge";
import { type FfmpegRemoteVersion, useFfmpegStore } from "@/stores/ffmpegStore";
import { FfmpegRuntimeStatus } from "@/components/ffmpeg/FfmpegRuntimeStatus";
import { useTranslation } from "react-i18next";

const formatOsLabel = (os: string | null | undefined, t: (key: string) => string): string => {
  const value = (os || "").toLowerCase();
  if (value === "windows") return t("labels.os.windows");
  if (value === "linux") return t("labels.os.linux");
  if (value === "macos") return t("labels.os.macos");
  return os || t("common.na");
};

const formatArchLabel = (
  os: string | null | undefined,
  arch: string | null | undefined,
  t: (key: string) => string,
): string => {
  const osValue = (os || "").toLowerCase();
  const archValue = (arch || "").toLowerCase();
  if (osValue === "macos" && archValue === "arm64") return t("labels.arch.apple_silicon");
  if (osValue === "macos" && archValue === "x86_64") return t("labels.arch.intel");
  if (archValue === "x86_64") return t("labels.arch.x64");
  if (archValue === "arm64") return t("labels.arch.arm64");
  return arch || t("common.na");
};

const getInstalledStatusMeta = (
  item: FfmpegVersionItem,
  downloading: boolean,
  t: (key: string) => string,
): { text: string; className: string } => {
  if (item.isActive) {
    return { text: t("status.installed.active"), className: "bg-green-600 text-white hover:bg-green-600" };
  }
  if (downloading) {
    return { text: t("status.common.downloading"), className: "bg-blue-600 text-white hover:bg-blue-600" };
  }
  if (item.downloadState === "failed") {
    return { text: t("status.common.failed"), className: "bg-red-600 text-white hover:bg-red-600" };
  }
  return { text: t("status.installed.installed"), className: "bg-slate-600 text-white hover:bg-slate-600" };
};

const getRemoteStatusMeta = (
  item: FfmpegRemoteVersion,
  downloading: boolean,
  downloaded: boolean,
  t: (key: string) => string,
): { text: string; className: string } => {
  if (downloading) {
    return { text: t("status.common.downloading"), className: "bg-blue-600 text-white hover:bg-blue-600" };
  }
  if (downloaded) {
    return { text: t("status.remote.downloaded"), className: "bg-green-600 text-white hover:bg-green-600" };
  }
  if (item.downloadState === "failed") {
    return { text: t("status.common.failed"), className: "bg-red-600 text-white hover:bg-red-600" };
  }
  return { text: t("status.remote.not_downloaded"), className: "bg-slate-500 text-white hover:bg-slate-500" };
};

export default function FFmpegVersionManagerPage() {
  const { t } = useTranslation("ffmpeg");
  const remoteVersions = useFfmpegStore((s) => s.remoteVersions);
  const installedVersions = useFfmpegStore((s) => s.installedVersions);
  const sourceMessage = useFfmpegStore((s) => s.sourceMessage);
  const busyRowKey = useFfmpegStore((s) => s.busyRowKey);
  const downloadProgress = useFfmpegStore((s) => s.downloadProgress);
  const cancelingRowKey = useFfmpegStore((s) => s.cancelingRowKey);
  const init = useFfmpegStore((s) => s.init);
  const downloadVersion = useFfmpegStore((s) => s.downloadVersion);
  const cancelDownload = useFfmpegStore((s) => s.cancelDownload);
  const activateVersion = useFfmpegStore((s) => s.activateVersion);
  const deleteVersion = useFfmpegStore((s) => s.deleteVersion);
  const [overwriteConfirmTarget, setOverwriteConfirmTarget] = useState<FfmpegRemoteVersion | null>(null);

  const downloadedRowKeys = useMemo(
    () =>
      new Set(
        installedVersions
          .filter((item) => item.downloadState === "downloaded" || item.isActive)
          .map((item) => item.rowKey),
      ),
    [installedVersions],
  );

  const installedVersionsForView = useMemo(() => {
    const stableItems: FfmpegVersionItem[] = [];
    const downloadingItems: FfmpegVersionItem[] = [];
    installedVersions.forEach((item) => {
      if (item.downloadState === "downloading" || downloadProgress[item.rowKey] !== undefined) {
        downloadingItems.push(item);
        return;
      }
      stableItems.push(item);
    });
    return [...stableItems, ...downloadingItems];
  }, [installedVersions, downloadProgress]);

  useEffect(() => {
    init().catch((error) => {
      toast.error(error instanceof Error ? error.message : t("toast.load_list_failed"));
    });
  }, [init, t]);

  const handleDownload = async (rowKey: string) => {
    try {
      await downloadVersion(rowKey);
    } catch (error) {
      const message = error instanceof Error ? error.message : t("toast.download_failed");
      toast.error(message);
    }
  };

  const handleCancelDownload = async (rowKey: string) => {
    try {
      await cancelDownload(rowKey);
    } catch (error) {
      const message = error instanceof Error ? error.message : t("toast.cancel_download_failed");
      toast.error(message);
    }
  };

  const handleActivate = async (rowKey: string) => {
    try {
      await activateVersion(rowKey, { quietEnsure: true });
      toast.success(t("toast.activated"));
    } catch (error) {
      const message = error instanceof Error ? error.message : t("toast.activate_failed");
      toast.error(message);
    }
  };

  const handleDelete = async (rowKey: string) => {
    try {
      await deleteVersion(rowKey);
      toast.success(t("toast.deleted"));
    } catch (error) {
      const message = error instanceof Error ? error.message : t("toast.delete_failed");
      toast.error(message);
    }
  };

  const remoteColumns = useMemo<ColumnDef<FfmpegRemoteVersion>[]>(
    () => [
      {
        accessorKey: "source",
        header: t("table.source"),
        cell: ({ row }) => row.original.source,
      },
      {
        accessorKey: "version",
        header: t("table.version"),
        cell: ({ row }) => <span className="font-medium">{row.original.version}</span>,
      },
      {
        accessorKey: "releaseDate",
        header: t("table.release_date"),
        cell: ({ row }) => <span className="text-muted-foreground">{row.original.releaseDate}</span>,
      },
      {
        accessorKey: "arch",
        header: t("table.arch"),
        cell: ({ row }) => row.original.arch,
      },
      {
        id: "status",
        header: t("table.status"),
        cell: ({ row }) => {
          const item = row.original;
          const downloading = downloadProgress[item.rowKey] !== undefined;
          const downloaded = item.downloadState === "downloaded" || downloadedRowKeys.has(item.rowKey);
          const status = getRemoteStatusMeta(item, downloading, downloaded, t);
          return <Badge className={status.className}>{status.text}</Badge>;
        },
      },
      {
        id: "actions",
        header: t("table.actions"),
        cell: ({ row }) => {
          const item = row.original;
          const downloading = downloadProgress[item.rowKey] !== undefined;
          const downloaded = item.downloadState === "downloaded" || downloadedRowKeys.has(item.rowKey);
          const canceling = cancelingRowKey === item.rowKey;
          if (downloading) {
            return (
              <Button
                size="sm"
                variant="destructive"
                disabled={canceling}
                onClick={() => handleCancelDownload(item.rowKey)}
              >
                {canceling ? t("actions.canceling") : t("actions.cancel_download")}
              </Button>
            );
          }
          return (
            <Button
              size="sm"
              onClick={() => {
                if (downloaded) {
                  setOverwriteConfirmTarget(item);
                  return;
                }
                handleDownload(item.rowKey);
              }}
            >
              {downloaded ? t("actions.downloaded") : t("actions.download")}
            </Button>
          );
        },
      },
    ],
    [downloadProgress, downloadedRowKeys, cancelingRowKey, t],
  );

  const remoteTable = useReactTable({
    data: remoteVersions,
    columns: remoteColumns,
    getCoreRowModel: getCoreRowModel(),
  });
  const sourceMessageText = sourceMessage ? t(sourceMessage.key, sourceMessage.values) : "";

  return (
    <main className="mx-auto w-full max-w-6xl space-y-4 px-4 py-2">
      <section className="space-y-1">
        <h1 className="text-3xl font-bold tracking-tight">{t("page.title")}</h1>
        <p className="text-sm text-muted-foreground">
          {t("page.description")}
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-lg font-bold tracking-tight">{t("installed.title")}</h2>
        <div className="space-y-2 px-0">
          {installedVersionsForView.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("installed.empty")}</p>
          ) : (
            installedVersionsForView.map((item) => {
              const label = `${item.version} (${formatOsLabel(item.os, t)} / ${formatArchLabel(item.os, item.arch, t)})`;
              const busy = busyRowKey === item.rowKey;
              const progress = downloadProgress[item.rowKey] ?? 0;
              const downloading =
                item.downloadState === "downloading" || downloadProgress[item.rowKey] !== undefined;
              const canceling = cancelingRowKey === item.rowKey;
              const status = getInstalledStatusMeta(item, downloading, t);
              return (
                <div
                  key={item.rowKey}
                  className="flex flex-col gap-2 rounded-lg border p-3 md:flex-row md:items-center md:justify-between"
                >
                  <div className="min-w-0 flex-1 space-y-2">
                    <div className="flex items-center gap-2">
                      <p className="text-sm">{label}</p>
                      <Badge className={status.className}>{status.text}</Badge>
                    </div>
                    {downloading && (
                      <div className="space-y-1">
                        <Progress value={progress} className="h-2" />
                        <p className="text-xs text-muted-foreground">{progress}%</p>
                      </div>
                    )}
                  </div>
                  {downloading ? (
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant="destructive"
                        disabled={canceling}
                        onClick={() => handleCancelDownload(item.rowKey)}
                      >
                        {canceling ? t("actions.canceling") : t("actions.cancel_download")}
                      </Button>
                    </div>
                  ) : (
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        disabled={busy || item.isActive}
                        onClick={() => handleActivate(item.rowKey)}
                      >
                        {item.isActive ? t("actions.in_use") : t("actions.activate")}
                      </Button>
                      <Button
                        size="sm"
                        variant="destructive"
                        disabled={busy || item.isActive}
                        onClick={() => handleDelete(item.rowKey)}
                      >
                        {t("actions.delete")}
                      </Button>
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </section>
      <Card className="border-none shadow-none px-0 py-2 gap-2">
        <CardHeader className=" px-0">
          <CardTitle>{t("remote.title")}</CardTitle>
          <CardDescription>{t("remote.description")}</CardDescription>
        </CardHeader>
        <CardContent className=" px-0">
          <div className="mb-3 text-xs text-muted-foreground">
            <span>{sourceMessageText}</span>
          </div>
          <Table className="min-w-[860px]" wrapperClassName="rounded-lg border">
            <TableHeader className="bg-muted/50">
              {remoteTable.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <TableHead
                      key={header.id}
                      className={`px-4 py-3 font-medium ${
                        header.column.id === "actions"
                          ? "sticky right-0 z-20 bg-muted/50 text-right shadow-[-8px_0_8px_-8px_rgba(0,0,0,0.15)]"
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
              {remoteTable.getRowModel().rows.map((row) => (
                <TableRow key={row.id} className="border-t">
                  {row.getVisibleCells().map((cell) => (
                    <TableCell
                      key={cell.id}
                      className={`px-4 py-3 ${
                        cell.column.id === "actions"
                          ? "sticky right-0 z-10 bg-background text-right shadow-[-8px_0_8px_-8px_rgba(0,0,0,0.12)]"
                          : ""
                      }`}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
      <Dialog
        open={Boolean(overwriteConfirmTarget)}
        onOpenChange={(open) => {
          if (!open) {
            setOverwriteConfirmTarget(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("confirm.overwrite_download.title")}</DialogTitle>
            <DialogDescription>
              {t("confirm.overwrite_download.description", {
                version: overwriteConfirmTarget?.version || "-",
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOverwriteConfirmTarget(null)}>
              {t("confirm.overwrite_download.cancel")}
            </Button>
            <Button
              onClick={async () => {
                if (!overwriteConfirmTarget) return;
                const rowKey = overwriteConfirmTarget.rowKey;
                setOverwriteConfirmTarget(null);
                await handleDownload(rowKey);
              }}
            >
              {t("confirm.overwrite_download.confirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </main>
  );
}
