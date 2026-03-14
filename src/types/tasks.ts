
export enum MediaTaskType {
  Ffmpeg = "ffmpeg",
}


export enum FileType {
  Video = "video",
  Audio = "audio",
  Image = "image",
  Gif = "gif",
}

export interface FFmpegTask {
  id: string;
  status: "idle" | "queued" | "processing" | "finished" | "error" | "cancelled";
  progress: number;
  errorMessage?: string;
  outputTitle?: string;
  fileType: FileType;
  taskType: MediaTaskType;
}
