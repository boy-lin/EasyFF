import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { type FfmpegEnsureStage, useFfmpegStore } from "@/stores/ffmpegStore";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { Loader2 } from "lucide-react";

function getFfmpegBadgeMeta(
  stage: FfmpegEnsureStage,
  version: string,
  t: (key: string) => string,
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
      text: t("runtime.badge.downloading"),
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
): string {
  if (stage === "checking") {
    return t("runtime.detail.checking");
  }
  if (stage === "activating") {
    return t("runtime.detail.activating");
  }
  if (stage === "downloading") {
    return t("runtime.detail.downloading");
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
};

export function FfmpegRuntimeStatus({ }: FfmpegRuntimeStatusProps) {
  const { t } = useTranslation("ffmpeg");
  const ffmpegVersion = useFfmpegStore((s) => s.runtimeVersion);
  const ffmpegEnsureStage = useFfmpegStore((s) => s.ensureStage);
  const ffmpegEnsureMessage = useFfmpegStore((s) => s.ensureMessage);

  const badge = getFfmpegBadgeMeta(ffmpegEnsureStage, ffmpegVersion, t);
  const ensureDetailMessage =
    ffmpegEnsureMessage?.trim() || getEnsureDetailMessage(ffmpegEnsureStage, t);

  return (
    <div className="flex gap-2">
      <Tooltip>
        <TooltipTrigger asChild>
          <Badge className={cn("w-[50px] whitespace-nowrap text-xs", badge.className)}>{
            ffmpegEnsureStage === 'ready' ? badge.text.substring(0, 5) : ffmpegEnsureStage === 'error' ? t("runtime.badge.error") : <Loader2 className="h-5 w-5 animate-spin" />
          }</Badge>
        </TooltipTrigger>
        <TooltipContent>
          <p>{badge.text}</p>
          {ensureDetailMessage ? <p className="mt-1 max-w-[360px] break-words">{ensureDetailMessage}</p> : null}
        </TooltipContent>
      </Tooltip>
    </div>
  );
}


