import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { downloadDir } from "@tauri-apps/api/path";

interface SettingsState {
  outputPath: string;
  isLoading: boolean;
  init: () => Promise<void>;
  setOutputPath: (path: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      outputPath: "",
      isLoading: true,
      init: async () => {
        try {
          let outputPath = get().outputPath?.trim();
          if (!outputPath) {
            outputPath = await downloadDir();
            set({ outputPath });
          }
          set({ isLoading: false });
        } catch (error) {
          console.error("Failed to load settings:", error);
          set({ isLoading: false });
        }
      },
      setOutputPath: async (path) => {
        set({ outputPath: path });
      }
    }),
    {
      name: "settings_store",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        outputPath: state.outputPath,
      }),
    },
  ),
);
