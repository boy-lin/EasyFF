import { create } from "zustand";
import { downloadDir } from "@tauri-apps/api/path";

const STORAGE_PREFIX = "settings:";

const readSetting = <T>(key: string): T | undefined => {
  if (typeof localStorage === "undefined") return undefined;
  const raw = localStorage.getItem(`${STORAGE_PREFIX}${key}`);
  if (!raw) return undefined;
  try {
    return JSON.parse(raw) as T;
  } catch (error) {
    console.warn("Failed to parse setting:", key, error);
    return undefined;
  }
};

const writeSetting = (key: string, value: unknown) => {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(`${STORAGE_PREFIX}${key}`, JSON.stringify(value));
};

interface SettingsState {
  outputPath: string;
  isLoading: boolean;
  init: () => Promise<void>;
  setOutputPath: (path: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  outputPath: "",
  isLoading: true,
  init: async () => {
    try {
      let outputPath = readSetting<string>("outputPath");
      if (!outputPath) {
        outputPath = await downloadDir();
        writeSetting("outputPath", outputPath);
      }
      set({
        outputPath,
        isLoading: false,
      });
    } catch (error) {
      console.error("Failed to load settings:", error);
      set({ isLoading: false });
    }
  },
  setOutputPath: async (path) => {
    try {
      writeSetting("outputPath", path);
      set({ outputPath: path });
    } catch (error) {
      console.error("Failed to save output path:", error);
    }
  }
}));
