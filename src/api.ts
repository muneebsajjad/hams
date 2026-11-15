// Types and calls shared with the Rust side (src-tauri/src/app.rs, main.rs).
import { invoke } from "@tauri-apps/api/core";

export type Phase = "idle" | "recording" | "transcribing";
export type ModelStatus = "missing" | "loading" | "ready" | "failed";

export interface Entry {
  id: string;
  date: string;
  text: string;
  duration: number;
  words: number;
  audioFile?: string;
  app?: string;
  /** Dictionary "heard as" replacements made in this transcript. */
  dictFixes?: number;
  /** Filler and repeated words removed from this transcript. */
  tidyFixes?: number;
}

export interface DictWord {
  id: string;
  word: string;
  misheard: string;
}

export interface Snippet {
  id: string;
  trigger: string;
  expansion: string;
}

export interface Settings {
  hotkey: string;
  model: string;
  language: string;
  autoPaste: boolean;
  keepAudio: boolean;
  sounds: boolean;
  tidy: boolean;
  polish: boolean;
  polishModel: string;
}

export interface Model {
  name: string;
  title: string;
  detail: string;
  sizeMb: number;
  downloaded: boolean;
  englishOnly: boolean;
}

export interface Status {
  phase: Phase;
  notice: string | null;
  modelStatus: ModelStatus;
  modelError: string | null;
  polishStatus: ModelStatus;
  polishError: string | null;
  polishModels: Model[];
  inputTrusted: boolean;
  micGranted: boolean;
  downloads: Record<string, number>;
  settings: Settings;
  models: Model[];
}

export interface Snapshot {
  status: Status;
  entries: Entry[];
  dictionary: DictWord[];
  snippets: Snippet[];
  scratchpad: string;
  hotkeys: { id: string; label: string }[];
  platform: string;
  firstName: string | null;
  inputPermission: string;
  inputHelp: string;
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  toggleRecording: () => invoke<void>("toggle_recording"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  setDictionary: (words: DictWord[]) => invoke<void>("set_dictionary", { words }),
  setSnippets: (snippets: Snippet[]) => invoke<void>("set_snippets", { snippets }),
  setScratchpad: (text: string) => invoke<void>("set_scratchpad", { text }),
  deleteEntry: (id: string) => invoke<void>("delete_entry", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  readAudio: (file: string) => invoke<ArrayBuffer>("read_audio", { file }),
  downloadModel: (name: string) => invoke<void>("download_model", { name }),
  deleteModel: (name: string) => invoke<void>("delete_model", { name }),
  requestInputAccess: () => invoke<void>("request_input_access"),
  openInputSettings: () => invoke<void>("open_input_settings"),
  openMicSettings: () => invoke<void>("open_mic_settings"),
  openDataFolder: () => invoke<void>("open_data_folder"),
  restartApp: () => invoke<void>("restart_app"),
};

export function newId(): string {
  return crypto.randomUUID().toUpperCase();
}
