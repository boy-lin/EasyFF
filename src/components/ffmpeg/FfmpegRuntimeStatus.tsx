import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { toast } from "sonner";
import { type FfmpegEnsureStage, useFfmpegStore } from "@/stores/ffmpegStore";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { Loader2 } from "lucide-react";

function getFfmpegBadgeMeta(
  stage: FfmpegEnsureStage,
  version: string,
  t: (key: string) => string,
  progress?: number,
): { text: string; className: string } {
  if (stage === "checking") {
    return {
      text: t("runtime.badge.checking"),
      className: "bg-blue-600 text-white hover:bg-blue-600",
    };
  }
  if (stage === "activating") {
    return {
      text: t("runtime.badge.activating"),
      className: "bg-cyan-600 text-white hover:bg-cyan-600",
    };
  }
  if (stage === "downloading") {
    return {
      text: `${t("runtime.badge.downloading")}${typeof progress === "number" ? ` ${progress}%` : ""}`,
      className: "bg-amber-500 text-black hover:bg-amber-500",
    };
  }
  if (stage === "error") {
    return {
      text: t("runtime.badge.error"),
      className: "bg-red-600 text-white hover:bg-red-600",
    };
  }
  if (stage === "ready") {
    return {
      text: version || t("runtime.badge.ready"),
      className: "bg-green-600 text-white hover:bg-green-600",
    };
  }
  return {
    text: version || t("runtime.badge.idle"),
    className: "bg-slate-600 text-white hover:bg-slate-600",
  };
}

function getEnsureDetailMessage(
  stage: FfmpegEnsureStage,
  t: (key: string) => string,
  progress?: number,
): string {
  if (stage === "checking") {
    return t("runtime.detail.checking");
  }
  if (stage === "activating") {
    return t("runtime.detail.activating");
  }
  if (stage === "downloading") {
    return `${t("runtime.detail.downloading")}${typeof progress === "number" ? ` ${progress}%` : ""}`;
  }
  if (stage === "error") {
    return t("runtime.detail.error");
  }
  if (stage === "ready") {
    return t("runtime.detail.ready");
  }
  return t("runtime.detail.idle");
}

type FfmpegRuntimeStatusProps = {
  showDetails?: boolean;
};

export function FfmpegRuntimeStatus({ showDetails = false }: FfmpegRuntimeStatusProps) {
  const { t } = useTranslation("ffmpeg");
  const ffmpegVersion = useFfmpegStore((s) => s.runtimeVersion);
  const ffmpegEnsureStage = useFfmpegStore((s) => s.ensureStage);
  const ffmpegEnsureMessage = useFfmpegStore((s) => s.ensureMessage);
  const ffmpegEnsureTargetRowKey = useFfmpegStore((s) => s.ensureTargetRowKey);
  const ffmpegDownloadProgress = useFfmpegStore((s) => s.downloadProgress);
  const ffmpegCancelingRowKey = useFfmpegStore((s) => s.cancelingRowKey);
  const cancelFfmpegDownload = useFfmpegStore((s) => s.cancelDownload);

  const ffmpegEnsureProgress =
    ffmpegEnsureTargetRowKey ? ffmpegDownloadProgress[ffmpegEnsureTargetRowKey] : undefined;
  const badge = getFfmpegBadgeMeta(ffmpegEnsureStage, ffmpegVersion, t, ffmpegEnsureProgress);
  const ensureDetailMessage =
    ffmpegEnsureMessage?.trim() || getEnsureDetailMessage(ffmpegEnsureStage, t, ffmpegEnsureProgress);

  return (
    <div className="flex gap-2">
      <Tooltip>
        <TooltipTrigger asChild>
          <Badge className={cn("w-[50px] whitespace-nowrap text-xs", badge.className)}>{
            ffmpegEnsureStage === 'ready' ? badge.text : ffmpegEnsureStage === 'error' ? t("runtime.badge.error") : <Loader2 className="h-5 w-5 animate-spin" />
          }</Badge>
        </TooltipTrigger>
        <TooltipContent>
          <p>{badge.text}</p>
          {ensureDetailMessage ? <p className="mt-1 max-w-[360px] break-words">{ensureDetailMessage}</p> : null}
        </TooltipContent>
      </Tooltip>
      {showDetails && (
        <div className="rounded-full border px-3 py-2 text-xs">
          <div className="flex items-center justify-between gap-3">
            <span
              className={
                ffmpegEnsureStage === "error"
                  ? "text-red-600"
                  : ffmpegEnsureStage === "ready"
                    ? "text-green-600"
                    : "text-blue-600"
              }
            >
              {ensureDetailMessage}
            </span>
            {ffmpegEnsureStage === "downloading" && ffmpegEnsureTargetRowKey && (
              <Button
                size="sm"
                variant="destructive"
                disabled={ffmpegCancelingRowKey === ffmpegEnsureTargetRowKey}
                onClick={() => {
                  void cancelFfmpegDownload(ffmpegEnsureTargetRowKey).catch((error) => {
                    const message =
                      error instanceof Error ? error.message : t("runtime.toast.cancel_download_failed");
                    toast.error(message);
                  });
                }}
              >
                {ffmpegCancelingRowKey === ffmpegEnsureTargetRowKey
                  ? t("runtime.actions.canceling")
                  : t("runtime.actions.cancel_download")}
              </Button>
            )}
          </div>
          {ffmpegEnsureStage === "downloading" && typeof ffmpegEnsureProgress === "number" && (
            <div className="mt-2 space-y-1">
              <Progress value={ffmpegEnsureProgress} className="h-2" />
              <p className="text-muted-foreground">{ffmpegEnsureProgress}%</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}


