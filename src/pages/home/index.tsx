import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Loader2, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { bridge, type FavoriteCommandItem } from "@/lib/bridge";
import { useSettingsStore } from "@/stores/settingsStore";
import { toast } from "sonner";
import { useUiStatus } from "@/hooks/useUiStatus";
import { FavoriteCommandList } from "@/pages/home/components/FavoriteCommandList";
import { useFfmpegStore } from "@/stores/ffmpegStore";
import { useFavoriteSyncStore } from "@/stores/favoriteSyncStore";
import { FfmpegRuntimeStatus } from "@/components/ffmpeg/FfmpegRuntimeStatus";
import { VirtualLogViewer } from "@/components/cli-logs/VirtualLogViewer";
import { buildCommandText } from "@/pages/home/lib/commandComposer";
import { useCliTaskRunner } from "@/pages/home/hooks/useCliTaskRunner";
import { useTranslation } from "react-i18next";

function formatDateTime(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return "";
  return new Date(ts).toLocaleString();
}

export default function Home() {
  const { t } = useTranslation("ffmpeg");
  const [searchParams, setSearchParams] = useSearchParams();
  const [inputPaths, setInputPaths] = useState<string[]>([]);
  const [resolvedOutputPath, setResolvedOutputPath] = useState<string>("");
  const { status, setStatus, className: statusClassName } = useUiStatus();
  const { execute, running, taskLogs, lastTaskCompleted } = useCliTaskRunner(setStatus);
  const [favoriteCommands, setFavoriteCommands] = useState<FavoriteCommandItem[]>([]);
  const [favoriteDialogOpen, setFavoriteDialogOpen] = useState<boolean>(false);
  const [favoriteTitle, setFavoriteTitle] = useState<string>("");
  const [favoriteDescription, setFavoriteDescription] = useState<string>("");
  const [favoriteSaving, setFavoriteSaving] = useState<boolean>(false);
  const commandInputRef = useRef<HTMLTextAreaElement | null>(null);
  const outputDir = useSettingsStore((s) => s.outputPath);
  const commandText = useSettingsStore((s) => s.homeCommandText);
  const settingsLoading = useSettingsStore((s) => s.isLoading);
  const initSettings = useSettingsStore((s) => s.init);
  const setOutputPath = useSettingsStore((s) => s.setOutputPath);
  const setHomeCommandText = useSettingsStore((s) => s.setHomeCommandText);
  const ffmpegRuntimeVersion = useFfmpegStore((s) => s.runtimeVersion);
  const ffmpegExecutablePath = useFfmpegStore((s) => s.runtimeExecutablePath);
  const ffmpegInstalled = useFfmpegStore((s) => s.installedVersions);
  const ffmpegEnsureStage = useFfmpegStore((s) => s.ensureStage);
  const initFfmpegStore = useFfmpegStore((s) => s.init);
  const ensureFfmpegReady = useFfmpegStore((s) => s.ensureReadyForHome);
  const scheduleFavoriteSync = useFavoriteSyncStore((s) => s.scheduleSync);

  const ffmpegPreparing =
    ffmpegEnsureStage === "checking" ||
    ffmpegEnsureStage === "downloading" ||
    ffmpegEnsureStage === "activating";

  useEffect(() => {
    initSettings().catch((error) => {
      console.error("init settings failed:", error);
    });
  }, [initSettings]);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      try {
        await initFfmpegStore();
        await ensureFfmpegReady();
      } catch (error) {
        if (cancelled) return;
        const message =
          error instanceof Error ? error.message : t("homePage.toast.init_failed");
        setStatus({ text: message, kind: "error" });
      }
    };
    run();
    return () => {
      cancelled = true;
    };
  }, [initFfmpegStore, ensureFfmpegReady, setStatus, t]);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      try {
        const list = await bridge.listFavoriteCommands(6, 0);
        if (!cancelled) setFavoriteCommands(list);
      } catch {
        toast.error(t("homePage.toast.get_favorites_failed"));
      }
    };
    run();
    return () => {
      cancelled = true;
    };
  }, [t]);

  useEffect(() => {
    const incoming = searchParams.get("commandText");
    if (!incoming) return;
    const nextCommand = incoming.trim();
    if (nextCommand) {
      setHomeCommandText(nextCommand);
      window.setTimeout(() => {
        const el = commandInputRef.current;
        if (!el) return;
        el.focus();
        el.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 0);
    }
    const nextParams = new URLSearchParams(searchParams);
    nextParams.delete("commandText");
    setSearchParams(nextParams, { replace: true });
  }, [searchParams, setHomeCommandText, setSearchParams]);

  useEffect(() => {
    if (!outputDir || inputPaths.length === 0) return;
    const rebuilt = buildCommandText(commandText, inputPaths, outputDir);
    setHomeCommandText(rebuilt.text);
    setResolvedOutputPath(rebuilt.outputPath);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [outputDir]);

  const pickInputFile = async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: t("homePage.media_filter_name"),
          extensions: ["mp4", "mov", "mkv", "avi", "mp3", "wav", "flac", "m4a"],
        },
      ],
    });
    if (!selected) return;

    const paths = Array.isArray(selected) ? selected : [selected];
    setInputPaths(paths);

    const rebuilt = buildCommandText(commandText, paths, outputDir);
    setHomeCommandText(rebuilt.text);
    setResolvedOutputPath(rebuilt.outputPath);
  };

  const pickOutputDir = async () => {
    const selected = await open({
      multiple: false,
      directory: true,
    });
    if (!selected) return;

    const dir = Array.isArray(selected) ? selected[0] : selected;
    if (!dir) return;

    await setOutputPath(dir);
    const rebuilt = buildCommandText(commandText, inputPaths, dir);
    setHomeCommandText(rebuilt.text);
    setResolvedOutputPath(rebuilt.outputPath);
  };

  const resolveActiveFfmpegExecutable = (): string => {
    const active = ffmpegInstalled.find((item) => item.isActive && !!item.localPath?.trim());
    if (active?.localPath?.trim()) return active.localPath.trim();
    if (ffmpegRuntimeVersion !== "unknown" && ffmpegExecutablePath?.trim()) {
      return ffmpegExecutablePath.trim();
    }
    return "";
  };

  const handleExecute = async () => {
    if (ffmpegPreparing) {
      toast.info(t("homePage.toast.ffmpeg_preparing"));
      return;
    }
    if (ffmpegEnsureStage !== "ready") {
      await ensureFfmpegReady();
      if (useFfmpegStore.getState().ensureStage !== "ready") {
        toast.warning(t("homePage.toast.ffmpeg_not_ready"));
        return;
      }
    }
    if (!commandText) {
      toast.warning(t("homePage.toast.command_empty"));
      return;
    }
    await execute({
      commandText,
      inputPaths,
      outputDir,
      executable: resolveActiveFfmpegExecutable(),
    });
  };

  const revealOutputInDir = async () => {
    if (!resolvedOutputPath || !lastTaskCompleted) return;
    try {
      await bridge.revealItemInDirFallback(resolvedOutputPath);
    } catch (e) {
      const message = e instanceof Error ? e.message : t("homePage.toast.open_output_failed");
      toast.error(message);
    }
  };

  const openFavoriteDialog = () => {
    const suggestedTitle = commandText.trim().slice(0, 32) || t("homePage.favorites.unnamed_command");
    setFavoriteTitle(suggestedTitle);
    setFavoriteDescription("");
    setFavoriteDialogOpen(true);
  };

  const saveFavoriteCommand = async () => {
    const title = favoriteTitle.trim();
    const command = commandText.trim();
    if (!title) {
      toast.warning(t("homePage.toast.favorite_title_required"));
      return;
    }
    if (!command) {
      toast.warning(t("homePage.toast.command_empty"));
      return;
    }

    setFavoriteSaving(true);
    try {
      const item = await bridge.createFavoriteCommand({
        title,
        description: favoriteDescription.trim() || undefined,
        command,
      });
      setFavoriteCommands((prev) => [item, ...prev]);
      scheduleFavoriteSync();
      setFavoriteDialogOpen(false);
      toast.success(t("homePage.toast.favorite_saved"));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : t("homePage.toast.favorite_save_failed"));
    } finally {
      setFavoriteSaving(false);
    }
  };

  const handleSelectFavoriteCommand = (selected: FavoriteCommandItem) => {
    setHomeCommandText(selected.command);
    window.setTimeout(() => {
      const el = commandInputRef.current;
      if (!el) return;
      el.focus();
      el.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 0);
  };

  return (
    <main className="mx-auto w-full max-w-5xl space-y-4 px-4 py-2">
      <section className="space-y-2">
        <div className="flex items-end gap-2">
          <h1 className="text-lg font-bold tracking-tight">FFmpeg</h1>
          <FfmpegRuntimeStatus />
          <Button asChild variant="link" className="h-auto p-0 text-sm">
            <Link to="/ffmpeg/version-manager">{t("homePage.actions.version_manager")}</Link>
          </Button>
        </div>
      </section>

      <section className="space-y-2">
        <Textarea
          ref={commandInputRef}
          className="min-h-24 font-mono text-sm"
          value={commandText}
          onChange={(e) => setHomeCommandText(e.target.value)}
        />

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            onClick={handleExecute}
            disabled={!commandText || running || ffmpegPreparing}
          >
            {running ? <Loader2 className="h-4 w-4 animate-spin" /> : <Sparkles className="h-4 w-4" />}
            {t("homePage.actions.execute")}
          </Button>
          <Button type="button" variant="secondary" onClick={pickInputFile}>
            {t("homePage.actions.select_input")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={pickOutputDir}
            disabled={settingsLoading}
          >
            {t("homePage.actions.select_output")}
          </Button>
          <Button type="button" variant="secondary" onClick={openFavoriteDialog}>
            {t("homePage.actions.save_favorite")}
          </Button>
        </div>

        {/* <p className="text-xs text-muted-foreground break-all">
          {t("homePage.labels.input")}: {inputPaths.length > 0 ? inputPaths.join(" ; ") : t("homePage.labels.not_selected")}
        </p>
        <p className="text-xs text-muted-foreground break-all">
          {t("homePage.labels.output")}: {outputDir || t("homePage.labels.not_selected")}
        </p> */}
        <div className="space-y-1">
          
          <div className="flex items-center gap-1 text-xs">
            <span className=" text-muted-foreground whitespace-nowrap">
              {t("homePage.labels.status")}
            </span>
            <span className={statusClassName}>{status.text}</span>
            {/* <span className="font-bold text-sm">{t("homePage.labels.realtime_logs")}</span> */}
          </div>
          {resolvedOutputPath && lastTaskCompleted && (
            <div className="flex items-center gap-1 text-xs text-muted-foreground break-all">
              <span>{t("homePage.labels.output_file")}: {resolvedOutputPath}</span>
              <Button
                type="button"
                variant="ghost"
                className="h-5 px-1"
                onClick={revealOutputInDir}
                title={t("homePage.actions.open_output_folder")}
              >
                <FolderOpen className="h-3.5 w-3.5" />
              </Button>
            </div>
          )}
          <VirtualLogViewer
            lines={taskLogs}
            height={192}
            rowHeight={22}
            emptyText={t("homePage.viewer.empty")}
            backToBottomText={t("homePage.viewer.back_to_bottom")}
          />
        </div>
       
      </section>

      <Card>
        <CardHeader className="">
          <CardTitle>{t("homePage.favorites.title")}</CardTitle>
          <CardDescription>{t("homePage.favorites.desc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <FavoriteCommandList
            items={favoriteCommands}
            onSelect={handleSelectFavoriteCommand}
            formatUpdatedAt={formatDateTime}
          />
        </CardContent>
      </Card>

      <Dialog open={favoriteDialogOpen} onOpenChange={setFavoriteDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("homePage.dialog.title")}</DialogTitle>
            <DialogDescription>{t("homePage.dialog.desc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-2">
              <Label htmlFor="favorite-title">{t("homePage.dialog.title_label")}</Label>
              <Input
                id="favorite-title"
                value={favoriteTitle}
                onChange={(e) => setFavoriteTitle(e.target.value)}
                placeholder={t("homePage.dialog.title_placeholder")}
                maxLength={80}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="favorite-description">{t("homePage.dialog.desc_label")}</Label>
              <Textarea
                id="favorite-description"
                value={favoriteDescription}
                onChange={(e) => setFavoriteDescription(e.target.value)}
                placeholder={t("homePage.dialog.desc_placeholder")}
                className="min-h-20"
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="secondary"
              onClick={() => setFavoriteDialogOpen(false)}
              disabled={favoriteSaving}
            >
              {t("homePage.actions.cancel")}
            </Button>
            <Button type="button" onClick={saveFavoriteCommand} disabled={favoriteSaving}>
              {favoriteSaving ? t("homePage.actions.saving") : t("homePage.actions.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </main>
  );
}
