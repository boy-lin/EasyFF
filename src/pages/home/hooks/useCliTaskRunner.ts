import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { bridge } from "@/lib/bridge";
import { getMediaTaskQueue } from "@/lib/mediaTaskQueue";
import { MediaTaskType } from "@/types/tasks";
import { splitCommand } from "@/pages/home/lib/commandComposer";

type UiStatusKind = "info" | "progress" | "success" | "error" | "warning";

type SetStatus = (status: { text: string; kind: UiStatusKind }) => void;

type ExecuteOptions = {
  commandText: string;
  inputPaths: string[];
  outputDir: string;
  executable: string;
};

export function useCliTaskRunner(setStatus: SetStatus) {
  const [currentTaskId, setCurrentTaskId] = useState<string>("");
  const [taskLogs, setTaskLogs] = useState<string[]>([]);
  const [running, setRunning] = useState<boolean>(false);
  const [lastTaskCompleted, setLastTaskCompleted] = useState<boolean>(false);
  const currentTaskIdRef = useRef<string>("");
  const queue = useMemo(() => getMediaTaskQueue(), []);

  useEffect(() => {
    currentTaskIdRef.current = currentTaskId;
  }, [currentTaskId]);

  useEffect(() => {
    const off = queue.on((event) => {
      const activeTaskId = currentTaskIdRef.current;
      if (!activeTaskId || event.task_id !== activeTaskId) return;
      if (event.event_type === "progress") {
        setStatus({ text: "执行中", kind: "progress" });
      } else if (event.event_type === "complete") {
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
  }, [queue, setStatus]);

  useEffect(() => {
    let off: (() => void) | null = null;
    let cancelled = false;
    const bind = async () => {
      off = await bridge.on("media_task_log", (payload) => {
        const activeTaskId = currentTaskIdRef.current;
        if (!activeTaskId || payload.task_id !== activeTaskId) return;
        const prefix = payload.stream === "stderr" ? "[stderr]" : "[stdout]";
        const line = `${prefix} ${payload.line}`;
        setTaskLogs((prev) => {
          const next = [...prev, line];
          if (next.length > 500) {
            return next.slice(next.length - 500);
          }
          return next;
        });
      });
      if (cancelled && off) {
        off();
      }
    };
    bind();
    return () => {
      cancelled = true;
      if (off) off();
    };
  }, []);

  const execute = useCallback(
    async ({ commandText, inputPaths, outputDir, executable }: ExecuteOptions) => {
      const tokens = splitCommand(commandText);
      if (tokens.length === 0) {
        setStatus({ text: "命令不能为空", kind: "warning" });
        return;
      }

      const command = executable.trim();
      if (!command) {
        setStatus({ text: "FFmpeg 不可用", kind: "warning" });
        return;
      }
      const args = tokens[0]?.startsWith("-") ? tokens : tokens.slice(1);
      const taskId = `cli-${Date.now()}-${Math.random().toString(16).slice(2)}`;

      setCurrentTaskId(taskId);
      setTaskLogs([`[runner] resolved executable: ${command}`]);
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
      } catch (error) {
        setRunning(false);
        toast.error(error instanceof Error ? error.message : "提交失败");
        setStatus({ text: error instanceof Error ? error.message : "提交失败", kind: "error" });
      }
    },
    [queue, setStatus],
  );

  return {
    execute,
    running,
    taskLogs,
    lastTaskCompleted,
  };
}
