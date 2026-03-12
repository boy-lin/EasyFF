import { useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  CardAction,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { toast } from "sonner";
import { bridge, type FfmpegVersionItem } from "@/lib/bridge";

type RemoteVersion = {
  rowKey: string;
  version: string;
  releaseDate: string;
  os: string;
  arch: string;
  state: string;
  downloadState: FfmpegVersionItem["downloadState"];
};

type SourceKey = "gyan" | "btbn" | "johnvansickle" | "evermeet";

type SourceConfig = {
  key: SourceKey;
  label: string;
  homepage: string;
};

type WebHostOS = "windows" | "linux" | "macos";

const formatOsLabel = (os?: string | null): string => {
  const value = (os || "").toLowerCase();
  if (value === "windows") return "Windows";
  if (value === "linux") return "Linux";
  if (value === "macos") return "macOS";
  return os || "-";
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

const mapDbRowToView = (row: FfmpegVersionItem): RemoteVersion => {
  const os = formatOsLabel(row.os);
  const arch = formatArchLabel(row.os, row.arch);
  return {
    rowKey: row.rowKey,
    version: row.version,
    releaseDate: row.publishedAt?.slice(0, 10) || "-",
    os,
    arch,
    state: mapDownloadState(row),
    downloadState: row.downloadState,
  };
};

const SOURCE_CONFIG: Record<SourceKey, SourceConfig> = {
  gyan: {
    key: "gyan",
    label: "Gyan.dev (Windows)",
    homepage: "https://www.gyan.dev/ffmpeg/builds/",
  },
  btbn: {
    key: "btbn",
    label: "BtbN (GitHub Releases)",
    homepage: "https://github.com/BtbN/FFmpeg-Builds/releases",
  },
  johnvansickle: {
    key: "johnvansickle",
    label: "John Van Sickle (Linux)",
    homepage: "https://johnvansickle.com/ffmpeg/",
  },
  evermeet: {
    key: "evermeet",
    label: "evermeet (macOS)",
    homepage: "https://evermeet.cxmpeg/",
  },
};

const SOURCE_BY_WEB_OS: Record<WebHostOS, SourceKey[]> = {
  windows: ["gyan", "btbn"],
  linux: ["btbn", "johnvansickle"],
  macos: ["evermeet", "btbn"],
};

const detectWebHostOS = (): WebHostOS => {
  if (typeof navigator === "undefined") return "windows";
  const ua = navigator.userAgent.toLowerCase();
  const platform = (navigator.platform || "").toLowerCase();
  if (platform.includes("mac") || ua.includes("mac os")) return "macos";
  if (platform.includes("linux") || ua.includes("linux")) return "linux";
  return "windows";
};

export default function FFmpegVersionManagerPage() {
  const availableSources = useMemo<SourceConfig[]>(() => {
    const os = detectWebHostOS();
    return SOURCE_BY_WEB_OS[os].map((key) => SOURCE_CONFIG[key]);
  }, []);

  const [selectedSourceKey, setSelectedSourceKey] = useState<SourceKey>(
    availableSources[0]?.key ?? "btbn",
  );
  const [remoteVersions, setRemoteVersions] = useState<RemoteVersion[]>([]);
  const [installedVersions, setInstalledVersions] = useState<FfmpegVersionItem[]>([]);
  const [sourceMessage, setSourceMessage] = useState<string>("");
  const [busyRowKey, setBusyRowKey] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});

  const selectedSource =
    availableSources.find((source) => source.key === selectedSourceKey) ??
    availableSources[0];

  const loadInstalled = async () => {
    const rows = await bridge.listInstalledFfmpegVersions();
    setInstalledVersions(rows);
  };

  const loadRemoteBySource = async (sourceKey: SourceKey) => {
    const page = await bridge.listFfmpegVersions({
      source: sourceKey,
      limit: 50,
      offset: 0,
    });
    setRemoteVersions(page.list.map(mapDbRowToView));
    setSourceMessage(
      `Current source: ${SOURCE_CONFIG[sourceKey].label}. Loaded ${page.list.length} rows from ffmpeg_versions table.`,
    );
  };

  const refreshAll = async (sourceKey: SourceKey) => {
    await Promise.all([loadRemoteBySource(sourceKey), loadInstalled()]);
  };

  useEffect(() => {
    if (!selectedSource) return;
    let cancelled = false;

    const run = async () => {
      try {
        setSourceMessage(`Current source: ${selectedSource.label}. Loading...`);
        await refreshAll(selectedSource.key);
      } catch {
        if (cancelled) return;
        setRemoteVersions([]);
        setSourceMessage(`Current source: ${selectedSource.label}. Query failed.`);
      }
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [selectedSource]);

  const handleDownload = async (rowKey: string) => {
    if (!selectedSource) return;
    try {
      setBusyRowKey(rowKey);
      setDownloadProgress((prev) => ({ ...prev, [rowKey]: 0 }));
      await bridge.downloadFfmpegVersion(rowKey, (progress) => {
        const percent = Number.isFinite(progress.percent)
          ? Math.max(0, Math.min(100, Math.round(progress.percent)))
          : 0;
        setDownloadProgress((prev) => ({ ...prev, [rowKey]: percent }));
      });
      await refreshAll(selectedSource.key);
    } catch (error) {
      await bridge.updateFfmpegDownloadState(rowKey, "failed");
      await refreshAll(selectedSource.key);
      const reason =
        error instanceof Error ? error.message : String(error ?? "unknown error");
      toast.error(`FFmpeg download failed: ${reason}`);
    } finally {
      setBusyRowKey(null);
      setDownloadProgress((prev) => {
        const next = { ...prev };
        delete next[rowKey];
        return next;
      });
    }
  };

  const handleActivate = async (rowKey: string) => {
    if (!selectedSource) return;
    setBusyRowKey(rowKey);
    try {
      await bridge.activateFfmpegVersion(rowKey);
      await refreshAll(selectedSource.key);
    } finally {
      setBusyRowKey(null);
    }
  };

  const handleDelete = async (rowKey: string) => {
    if (!selectedSource) return;
    setBusyRowKey(rowKey);
    try {
      await bridge.deleteFfmpegVersion(rowKey);
      await refreshAll(selectedSource.key);
    } finally {
      setBusyRowKey(null);
    }
  };

  return (
    <main className="mx-auto w-full max-w-6xl space-y-6 px-4 py-6">
      <section className="space-y-1">
        <h1 className="text-3xl font-bold tracking-tight">
          FFmpeg Static Package Version Manager
        </h1>
        <p className="text-sm text-muted-foreground">
          Download remote static builds, cache status in DB, and manage active installed version.
        </p>
      </section>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle>Installed Versions</CardTitle>
          <CardDescription>
            Activate/Delete actions are managed here and will not disappear when source changes.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          {installedVersions.length === 0 ? (
            <p className="text-sm text-muted-foreground">No installed FFmpeg version.</p>
          ) : (
            installedVersions.map((item) => {
              const label = `${item.version} (${formatOsLabel(item.os)} / ${formatArchLabel(item.os, item.arch)})`;
              const busy = busyRowKey === item.rowKey;
              return (
                <div
                  key={item.rowKey}
                  className="flex flex-col gap-2 rounded border p-3 md:flex-row md:items-center md:justify-between"
                >
                  <div className="flex items-center gap-2">
                    <p className="text-sm">{label}</p>
                    {item.isActive && (
                      <Badge className="bg-green-600 text-white hover:bg-green-600">Active</Badge>
                    )}
                  </div>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      disabled={busy || item.isActive}
                      onClick={() => handleActivate(item.rowKey)}
                    >
                      {item.isActive ? "In Use" : "Activate"}
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      disabled={busy || item.isActive}
                      onClick={() => handleDelete(item.rowKey)}
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              );
            })
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader className="pb-3">
          <CardTitle>Remote Available Versions</CardTitle>
          <CardDescription>
            Source switching only changes this list; installed list is always visible below.
          </CardDescription>
          <CardAction className="w-full pt-3 md:w-auto">
            <div className="flex flex-col gap-3 md:items-end">
              <RadioGroup
                value={selectedSourceKey}
                onValueChange={(value) => setSelectedSourceKey(value as SourceKey)}
                className="grid grid-cols-1 gap-2 md:grid-cols-2"
              >
                {availableSources.map((source) => (
                  <label
                    key={source.key}
                    className="flex cursor-pointer items-center gap-2 rounded-md border px-2 py-1 text-xs"
                  >
                    <RadioGroupItem value={source.key} id={`source-${source.key}`} />
                    <span>{source.label}</span>
                  </label>
                ))}
              </RadioGroup>
            </div>
          </CardAction>
        </CardHeader>
        <CardContent>
          <div className="mb-3 text-xs text-muted-foreground">
            <span>{sourceMessage}</span>
            {selectedSource && (
              <>
                {" "}
                <a
                  href={selectedSource.homepage}
                  target="_blank"
                  rel="noreferrer"
                  className="underline underline-offset-2"
                >
                  Open source page
                </a>
              </>
            )}
          </div>
          <div className="overflow-x-auto rounded-lg border">
            <table className="w-full min-w-[860px] text-sm">
              <thead className="bg-muted/50 text-left">
                <tr>
                  <th className="px-4 py-3 font-medium">Version</th>
                  <th className="px-4 py-3 font-medium">Release Date</th>
                  <th className="px-4 py-3 font-medium">OS</th>
                  <th className="px-4 py-3 font-medium">Arch</th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {remoteVersions.map((item) => {
                  const downloading = busyRowKey === item.rowKey || item.downloadState === "downloading";
                  const downloaded = item.downloadState === "downloaded";
                  const percent = downloadProgress[item.rowKey] ?? 0;
                  return (
                    <tr key={item.rowKey} className="border-t">
                      <td className="px-4 py-3 font-medium">{item.version}</td>
                      <td className="px-4 py-3 text-muted-foreground">{item.releaseDate}</td>
                      <td className="px-4 py-3">{item.os}</td>
                      <td className="px-4 py-3">{item.arch}</td>
                      <td className="px-4 py-3">{item.state}</td>
                      <td className="px-4 py-3">
                        <Button
                          size="sm"
                          disabled={downloading || downloaded}
                          onClick={() => handleDownload(item.rowKey)}
                        >
                          {downloading
                            ? `Downloading ${percent}%`
                            : downloaded
                              ? "Downloaded"
                              : "Download"}
                        </Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

    </main>
  );
}
