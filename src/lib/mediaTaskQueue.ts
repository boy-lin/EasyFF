import { bridge } from "@/lib/bridge";
import { getBridgeErrorMessage } from "@/lib/bridgeError";
import { MediaTaskType } from "@/types/tasks";
import { MediaTaskEvent } from "./mediaTaskEvent";

export type CliMediaTaskRequest = {
  task_id: string;
  task_type: MediaTaskType | string;
  command: string;
  args: string[];
  input_path?: string;
  output_dir?: string;
};

class MediaTaskQueue {
  private static instance: MediaTaskQueue | null = null;
  private eventUnlisten: (() => void) | null = null;
  private listeners: Array<(event: MediaTaskEvent) => void> = [];

  private constructor() {}

  static getInstance(): MediaTaskQueue {
    if (MediaTaskQueue.instance === null) {
      MediaTaskQueue.instance = new MediaTaskQueue();
    }
    return MediaTaskQueue.instance;
  }

  async ensureEventListener(): Promise<void> {
    if (this.eventUnlisten !== null) return;
    this.eventUnlisten = await bridge.on("media_task_event", (payload) => {
      this.listeners.forEach((listener) => listener(payload));
    });
  }

  on(listener: (event: MediaTaskEvent) => void): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
      if (this.listeners.length === 0 && this.eventUnlisten) {
        this.eventUnlisten();
        this.eventUnlisten = null;
      }
    };
  }

  async submitCliTask(task: CliMediaTaskRequest): Promise<void> {
    await this.ensureEventListener();
    try {
      await bridge.submitMediaTasks([task], "normal");
    } catch (error) {
      throw new Error(getBridgeErrorMessage(error, "提交任务失败"));
    }
  }

  async hasRunningTasksByType(taskType?: MediaTaskType | string): Promise<boolean> {
    return bridge.hasRunningMediaTasksByType(taskType as MediaTaskType | undefined);
  }

  async clearQueueByType(
    stopRunning = false,
    taskType?: MediaTaskType | string,
  ): Promise<void> {
    await bridge.clearMediaTaskQueueByType(
      stopRunning,
      taskType as MediaTaskType | undefined,
    );
  }

  async cancelTaskById(id: string): Promise<void> {
    await bridge.cancelMediaTaskById(id);
  }
}

export function getMediaTaskQueue(): MediaTaskQueue {
  return MediaTaskQueue.getInstance();
}
