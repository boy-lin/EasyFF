import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { downloadDir } from "@tauri-apps/api/path";

const DEFAULT_HOME_COMMAND_TEXT = "ffmpeg -i input.mp4 -vn -c:a libmp3lame -q:a 2 output.mp3";

interface SettingsState {
  outputPath: string;
  homeCommandText: string;
  isLoading: boolean;
  init: () => Promise<void>;
  setOutputPath: (path: string) => Promise<void>;
  setHomeCommandText: (text: string) => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      outputPath: "",
      homeCommandText: DEFAULT_HOME_COMMAND_TEXT,
      isLoading: true,
      init: async () => {
        try {
          let outputPath = get().outputPath?.trim();
          if (!outputPath) {
            outputPath = await downloadDir();
          }
          const homeCommandText =
            get().homeCommandText?.trim() || DEFAULT_HOME_COMMAND_TEXT;
          set({ outputPath, homeCommandText, isLoading: false });
        } catch (error) {
          console.error("Failed to load settings:", error);
          set({ isLoading: false });
        }
      },
      setOutputPath: async (path) => {
        set({ outputPath: path });
      },
      setHomeCommandText: (text) => {
        set({ homeCommandText: text || DEFAULT_HOME_COMMAND_TEXT });
      },
    }),
    {
      name: "settings_store",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        outputPath: state.outputPath,
        homeCommandText: state.homeCommandText,
      }),
    },
  ),
);
