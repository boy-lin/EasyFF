import { useMemo, useState } from "react";

export type StatusKind = "idle" | "info" | "progress" | "success" | "warning" | "error";

export type UiStatus = {
  text: string;
  kind: StatusKind;
};

const STATUS_CLASS_MAP: Record<StatusKind, string> = {
  idle: "border-transparent bg-transparent text-muted-foreground",
  info: "border-slate-200 bg-slate-50 text-slate-700",
  progress: "border-sky-200 bg-sky-50 text-sky-700",
  success: "border-emerald-200 bg-emerald-50 text-emerald-700",
  warning: "border-amber-200 bg-amber-50 text-amber-700",
  error: "border-rose-200 bg-rose-50 text-rose-700",
};

export function useUiStatus(initial?: UiStatus) {
  const [status, setStatus] = useState<UiStatus>(initial ?? { text: "", kind: "idle" });

  const className = useMemo(() => STATUS_CLASS_MAP[status.kind], [status.kind]);

  const clear = () => setStatus({ text: "", kind: "idle" });
  const setInfo = (text: string) => setStatus({ text, kind: "info" });
  const setProgress = (text: string) => setStatus({ text, kind: "progress" });
  const setSuccess = (text: string) => setStatus({ text, kind: "success" });
  const setWarning = (text: string) => setStatus({ text, kind: "warning" });
  const setError = (text: string) => setStatus({ text, kind: "error" });

  return {
    status,
    className,
    setStatus,
    clear,
    setInfo,
    setProgress,
    setSuccess,
    setWarning,
    setError,
  };
}

