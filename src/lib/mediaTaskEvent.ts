import { FileType, MediaTaskType } from "@/types/tasks";

export type MediaTaskEvent = {
  task_id: string;
  task_type: MediaTaskType;
  file_type: FileType;
  event_type: "progress" | "complete" | "error";
  progress?: number;
  output_path?: string;
  output_size?: number;
  error_message?: string;
};
