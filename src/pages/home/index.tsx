import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { Badge } from "@/components/ui/badge";
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
import { getMediaTaskQueue } from "@/lib/mediaTaskQueue";
import { useSettingsStore } from "@/stores/settingsStore";
import { MediaTaskType } from "@/types/tasks";
import { toast } from "sonner";
import { useUiStatus } from "@/hooks/useUiStatus";
import { FavoriteCommandList } from "@/pages/home/components/FavoriteCommandList";

function splitCommand(input: string): string[] {
  const result: string[] = [];
  const regex = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(input)) !== null) {
    result.push(match[1] ?? match[2] ?? match[3]);
  }
  return result;
}

function quoteArg(arg: string): string {
  if (arg.includes(" ") || arg.includes("\t") || arg.includes('"')) {
    return `"${arg.replaceAll('"', '\\"')}"`;
  }
  return arg;
}

function pickOutputExt(oldOutput: string | undefined, firstInput: string | undefined): string {
  const fromOutput = oldOutput?.match(/\.([A-Za-z0-9]+)$/)?.[1];
  if (fromOutput) return fromOutput;
  const fromInput = firstInput?.match(/\.([A-Za-z0-9]+)$/)?.[1];
  if (fromInput) return fromInput;
  return "mp4";
}

const FFMPEG_NO_VALUE_OPTIONS = new Set([
  "-y",
  "-n",
  "-vn",
  "-an",
  "-sn",
  "-dn",
  "-shortest",
  "-hide_banner",
  "-nostdin",
  "-stats",
  "-copyts",
]);

function optionTakesValue(token: string): boolean {
  if (!token.startsWith("-")) return false;
  if (FFMPEG_NO_VALUE_OPTIONS.has(token)) return false;
  return true;
}

function normalizeDir(dir: string): string {
  return dir.replace(/[\\/]$/, "");
}

function detectPathSeparator(dir: string): "/" | "\\" {
  // Windows drive/UNC path or explicit backslash style => '\'
  if (/^[A-Za-z]:[\\/]/.test(dir) || dir.startsWith("\\\\") || dir.includes("\\")) {
    return "\\";
  }
  return "/";
}

function joinPath(dir: string, filename: string): string {
  const base = normalizeDir(dir);
  if (!base) return filename;
  const sep = detectPathSeparator(base);
  return `${base}${sep}${filename}`;
}

function formatDateTime(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return "";
  return new Date(ts).toLocaleString();
}

function buildCommandText(
  raw: string,
  inputPaths: string[],
  outputDir: string,
): { text: string; outputPath: string } {
  const tokens = splitCommand(raw);
  if (tokens.length === 0) {
    return { text: raw, outputPath: "" };
  }

  const command = tokens[0];
  const srcArgs = tokens.slice(1);
  const transformedArgs: string[] = [];
  const oldOutputs: string[] = [];

  for (let i = 0; i < srcArgs.length; i += 1) {
    const token = srcArgs[i];

    if (token === "-i") {
      i += 1;
      continue;
    }

    if (token.startsWith("-")) {
      transformedArgs.push(token);
      if (optionTakesValue(token) && i + 1 < srcArgs.length) {
        i += 1;
        transformedArgs.push(srcArgs[i]);
      }
      continue;
    }

    oldOutputs.push(token);
    transformedArgs.push(`__OUT_${oldOutputs.length - 1}__`);
  }

  const generatedOutputs: string[] = [];
  if (outputDir) {
    const outputCount = oldOutputs.length > 0 ? oldOutputs.length : 1;
    const ts = Date.now();
    for (let i = 0; i < outputCount; i += 1) {
      const oldOutput = oldOutputs[i];
      const ext = pickOutputExt(oldOutput, inputPaths[0]);
      const suffix = outputCount > 1 ? `-${i + 1}` : "";
      const filename = `output-${ts}${suffix}.${ext}`;
      generatedOutputs.push(joinPath(outputDir, filename));
    }
  } else {
    generatedOutputs.push(...oldOutputs);
  }

  let outputPath = generatedOutputs[0] ?? "";

  const nextArgs: string[] = [];
  inputPaths.forEach((p) => {
    nextArgs.push("-i", p);
  });
  let outIndex = 0;
  transformedArgs.forEach((arg) => {
    if (arg.startsWith("__OUT_")) {
      const resolved = generatedOutputs[outIndex] ?? generatedOutputs[0];
      outIndex += 1;
      if (resolved) nextArgs.push(resolved);
      return;
    }
    nextArgs.push(arg);
  });

  if (oldOutputs.length === 0 && outputPath) {
    nextArgs.push(outputPath);
  }

  const text = [command, ...nextArgs.map(quoteArg)].join(" ");
  return { text, outputPath };
}

export default function Home() {
  const [ffmpegVersion, setFfmpegVersion] = useState<string>("读取中...");
  const [commandText, setCommandText] = useState<string>(
    "ffmpeg -i input.mp4 -vf scale=1280:720 -c:v libx264 output.mp4",
  );
  const [inputPaths, setInputPaths] = useState<string[]>([]);
  const [resolvedOutputPath, setResolvedOutputPath] = useState<string>("");
  const [currentTaskId, setCurrentTaskId] = useState<string>("");
  const [currentProgress, setCurrentProgress] = useState<number>(0);
  const [running, setRunning] = useState<boolean>(false);
  const [lastTaskCompleted, setLastTaskCompleted] = useState<boolean>(false);
  const { status, className: statusClassName, setStatus } = useUiStatus();
  const [favoriteCommands, setFavoriteCommands] = useState<FavoriteCommandItem[]>([]);
  const [favoriteDialogOpen, setFavoriteDialogOpen] = useState<boolean>(false);
  const [favoriteTitle, setFavoriteTitle] = useState<string>("");
  const [favoriteDescription, setFavoriteDescription] = useState<string>("");
  const [favoriteSaving, setFavoriteSaving] = useState<boolean>(false);
  const commandInputRef = useRef<HTMLTextAreaElement | null>(null);
  const outputDir = useSettingsStore((s) => s.outputPath);
  const settingsLoading = useSettingsStore((s) => s.isLoading);
  const initSettings = useSettingsStore((s) => s.init);
  const setOutputPath = useSettingsStore((s) => s.setOutputPath);

  const queue = useMemo(() => getMediaTaskQueue(), []);

  useEffect(() => {
    initSettings().catch((error) => {
      console.error("init settings failed:", error);
    });
  }, [initSettings]);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      try {
        const version = await bridge.getCurrentFfmpegVersion();
        if (!cancelled) setFfmpegVersion(version || "unknown");
      } catch {
        if (!cancelled) setFfmpegVersion("unknown");
      }
    };
    run();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      try {
        const list = await bridge.listFavoriteCommands(100, 0);
        if (!cancelled) setFavoriteCommands(list);
      } catch {
        if (!cancelled) setStatus({ text: "读取收藏命令失败", kind: "error" });
      }
    };
    run();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const off = queue.on((event) => {
      if (!currentTaskId || event.task_id !== currentTaskId) return;
      if (event.event_type === "progress") {
        setCurrentProgress(event.progress ?? 0);
        setStatus({ text: `执行中 ${event.progress ?? 0}%`, kind: "progress" });
      } else if (event.event_type === "complete") {
        setCurrentProgress(100);
        setRunning(false);
        setLastTaskCompleted(true);
        setStatus({ text: "执行完成", kind: "success" });
      } else if (event.event_type === "error") {
        setRunning(false);
        setLastTaskCompleted(false);
        setStatus({ text: event.error_message || "执行失败", kind: "error" });
      }
    });
    return () => off();
  }, [queue, currentTaskId]);

  useEffect(() => {
    if (!outputDir || inputPaths.length === 0) return;
    const rebuilt = buildCommandText(commandText, inputPaths, outputDir);
    setCommandText(rebuilt.text);
    setResolvedOutputPath(rebuilt.outputPath);
    // Only react to output directory changes from settings cache.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [outputDir]);

  const pickInputFile = async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "Media",
          extensions: ["mp4", "mov", "mkv", "avi", "mp3", "wav", "flac", "m4a"],
        },
      ],
    });
    if (!selected) return;

    const paths = Array.isArray(selected) ? selected : [selected];
    setInputPaths(paths);

    const rebuilt = buildCommandText(commandText, paths, outputDir);
    setCommandText(rebuilt.text);
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
    setCommandText(rebuilt.text);
    setResolvedOutputPath(rebuilt.outputPath);
  };

  const resolveActiveFfmpegExecutable = async (): Promise<string> => {
    try {
      const runtime = await bridge.getCurrentFfmpegRuntimeInfo();
      if (runtime.executablePath?.trim()) {
        return runtime.executablePath;
      }
      const installed = await bridge.listInstalledFfmpegVersions();
      const active = installed.find((item) => item.isActive && !!item.localPath?.trim());
      if (active?.localPath) return active.localPath;
    } catch (error) {
      console.warn("resolve active ffmpeg failed:", error);
    }
    return "ffmpeg";
  };

  const handleExecute = async () => {
    if (inputPaths.length === 0 || !outputDir) {
      setStatus({ text: "请先选择输入文件和输出目录", kind: "warning" });
      return;
    }

    const tokens = splitCommand(commandText);
    if (tokens.length === 0) {
      setStatus({ text: "命令不能为空", kind: "warning" });
      return;
    }

    const command = await resolveActiveFfmpegExecutable();
    const args = tokens[0]?.startsWith("-") ? tokens : tokens.slice(1);
    const taskId = `cli-${Date.now()}-${Math.random().toString(16).slice(2)}`;

    setCurrentTaskId(taskId);
    setCurrentProgress(0);
    setRunning(true);
    setLastTaskCompleted(false);
    setStatus({ text: "任务已提交", kind: "info" });

    try {
      await queue.submitCliTask({
        task_id: taskId,
        task_type: MediaTaskType.Ffmpeg,
        command,
        args,
        input_path: inputPaths[0],
        output_dir: outputDir,
      });
    } catch (e) {
      setRunning(false);
      toast.error(e instanceof Error ? e.message : "提交失败");
      setStatus({ text: e instanceof Error ? e.message : "提交失败", kind: "error" });
    }
  };

  const copyCommand = async () => {
    await navigator.clipboard.writeText(commandText);
    setStatus({ text: "命令已复制", kind: "success" });
  };

  const revealOutputInDir = async () => {
    if (!resolvedOutputPath || !lastTaskCompleted) return;
    try {
      await bridge.revealItemInDirFallback(resolvedOutputPath);
    } catch (e) {
      const message = e instanceof Error ? e.message : "打开目录失败";
      toast.error(message);
      setStatus({ text: message, kind: "error" });
    }
  };

  const openFavoriteDialog = () => {
    const suggestedTitle = commandText.trim().slice(0, 32) || "未命名命令";
    setFavoriteTitle(suggestedTitle);
    setFavoriteDescription("");
    setFavoriteDialogOpen(true);
  };

  const saveFavoriteCommand = async () => {
    const title = favoriteTitle.trim();
    const command = commandText.trim();
    if (!title) {
      setStatus({ text: "请输入命令标题", kind: "warning" });
      return;
    }
    if (!command) {
      setStatus({ text: "命令不能为空", kind: "warning" });
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
      setFavoriteDialogOpen(false);
      setStatus({ text: "收藏成功", kind: "success" });
    } catch (e) {
      setStatus({ text: e instanceof Error ? e.message : "收藏失败", kind: "error" });
    } finally {
      setFavoriteSaving(false);
    }
  };

  const handleSelectFavoriteCommand = (selected: FavoriteCommandItem) => {
    setCommandText(selected.command);
    setStatus({ text: `已载入收藏命令：${selected.title}`, kind: "info" });
    window.setTimeout(() => {
      const el = commandInputRef.current;
      if (!el) return;
      el.focus();
      el.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 0);
  };

  return (
    <main className="mx-auto w-full max-w-5xl space-y-6 px-4 py-6">
      <section className="space-y-2">
        <div className="flex items-end gap-2">
          <h1 className="text-3xl font-bold tracking-tight">FFmpeg</h1>
          <Button asChild variant="link" className="h-auto p-0 text-sm">
            <Link to="/ffmpeg/version-manager">版本管理</Link>
          </Button>
        </div>
        <Badge variant="secondary">{ffmpegVersion}</Badge>
      </section>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle>FFmpeg 命令执行</CardTitle>
          <CardDescription>选择输入/输出后会自动重建参数，不在执行时再替换</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <Textarea
            ref={commandInputRef}
            className="min-h-24 font-mono text-sm"
            value={commandText}
            onChange={(e) => setCommandText(e.target.value)}
          />

          <div className="flex flex-wrap gap-2">
            <Button type="button" onClick={handleExecute} disabled={!commandText || running}>
              {running ? `执行中 ${currentProgress}%` : "执行命令"}
            </Button>
            <Button type="button" variant="secondary" onClick={pickInputFile}>
              选择输入文件
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={pickOutputDir}
              disabled={settingsLoading}
            >
              选择输出目录
            </Button>
            <Button type="button" variant="secondary" onClick={copyCommand}>
              复制命令
            </Button>
            <Button type="button" variant="secondary" onClick={openFavoriteDialog}>
              收藏命令
            </Button>
          </div>

          <p className="text-xs text-muted-foreground break-all">
            输入: {inputPaths.length > 0 ? inputPaths.join(" ; ") : "未选择"}
          </p>
          <p className="text-xs text-muted-foreground break-all">输出目录: {outputDir || "未选择"}</p>
          <div className="flex items-center gap-1 text-xs text-muted-foreground break-all">
            <span>输出文件: {resolvedOutputPath || "未生成"}</span>
            {resolvedOutputPath && lastTaskCompleted && (
              <Button
                type="button"
                variant="ghost"
                className="h-5 px-1"
                onClick={revealOutputInDir}
                title="打开文件所在目录"
              >
                <FolderOpen className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
          {status.text && (
            <div
              className={`rounded-md border px-2 py-1 text-xs transition-colors ${statusClassName}`}
            >
              {status.text}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="">
          <CardTitle>收藏命令</CardTitle>
          <CardDescription>常用命令快速回看</CardDescription>
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
            <DialogTitle>收藏命令</DialogTitle>
            <DialogDescription>填写标题和可选描述，保存当前命令。</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-2">
              <Label htmlFor="favorite-title">命令标题</Label>
              <Input
                id="favorite-title"
                value={favoriteTitle}
                onChange={(e) => setFavoriteTitle(e.target.value)}
                placeholder="例如：HEVC 转码"
                maxLength={80}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="favorite-description">描述（可选）</Label>
              <Textarea
                id="favorite-description"
                value={favoriteDescription}
                onChange={(e) => setFavoriteDescription(e.target.value)}
                placeholder="简要说明这个命令的用途"
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
              取消
            </Button>
            <Button type="button" onClick={saveFavoriteCommand} disabled={favoriteSaving}>
              {favoriteSaving ? "保存中..." : "保存收藏"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </main>
  );
}
