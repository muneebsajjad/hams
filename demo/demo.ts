// Runs the real hams UI in a plain browser with made-up data, for screenshots and UI work
// without the Rust side. Open http://localhost:1420/demo/ while `npm run dev` is running.
//   ?window=pill&phase=recording|transcribing   the floating indicator instead of the main window
// Nothing here ships: the app build only uses the root index.html.
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { DictWord, Entry, Model, Phase, Snapshot, Snippet } from "../src/api";

const params = new URLSearchParams(location.search);
const pill = params.get("window") === "pill";
const phase = (params.get("phase") as Phase | null) ?? (pill ? "recording" : "idle");

// A fixed seed, so every run draws the same history.
let seed = 7;
const random = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;
const pick = <T>(items: T[]) => items[Math.floor(random() * items.length)];

const RECENT: [string, string][] = [
  ["Slack", "Sounds good. I'll push the fix this afternoon and ping you once the build is green."],
  ["Mail", "Hi Priya, thanks for the notes on the proposal. I've updated the timeline and moved the launch to the second week of March. Let me know if that works for your team."],
  ["Code", "Add a retry with exponential backoff when the download fails, and surface the error in settings."],
  ["Notes", "Groceries: oat milk, coffee beans, lemons, and something for dinner on Friday."],
  ["Linear", "The waveform freezes when the microphone changes mid-recording. Steps to reproduce are in the thread."],
  ["Messages", "Running about ten minutes late, save me a seat."],
  ["Safari", "best offline speech to text for linux"],
];
const OLDER = [
  "Let's move the sync to Thursday at three and send the agenda beforehand.",
  "Can you review the pull request when you get a chance? It's mostly tests.",
  "Reminder to renew the domain before the end of the month.",
  "The new onboarding flow cut setup time roughly in half in our last test.",
  "Thanks, that fixed it. I owe you a coffee.",
  "Draft reply: we can support Arabic and Urdu with the multilingual model.",
];
const APPS = ["Slack", "Mail", "Code", "Notes", "Linear", "Messages", "Safari", "Obsidian", "Terminal"];

function history(): Entry[] {
  const now = Date.now();
  const entries: Entry[] = [];
  const add = (date: Date, app: string, text: string) => {
    const words = text.split(/\s+/).length;
    entries.push({
      id: crypto.randomUUID(),
      date: date.toISOString(),
      text,
      words,
      duration: words / 2.4,
      app,
      audioFile: "demo.wav",
      tidyFixes: Math.floor(random() * 4),
      dictFixes: random() < 0.2 ? 1 : 0,
    });
  };
  RECENT.forEach(([app, text], i) => add(new Date(now - (i * 47 + 6) * 60_000), app, text));
  for (let day = 1; day < 130; day++) {
    if (random() < (day < 30 ? 0.08 : 0.4)) continue;
    const count = 1 + Math.floor(random() * random() * (day < 30 ? 18 : 10));
    for (let i = 0; i < count; i++) {
      const date = new Date(now - day * 86_400_000);
      date.setHours(8 + Math.floor(random() * 11), Math.floor(random() * 60));
      add(date, pick(APPS), pick(OLDER));
    }
  }
  return entries;
}

const MODELS: Model[] = [
  { name: "large-v3-turbo-q5_0", title: "Turbo", detail: "Best accuracy, 99 languages", sizeMb: 547, downloaded: true, englishOnly: false },
  { name: "small.en", title: "Small", detail: "Balanced, English only", sizeMb: 466, downloaded: false, englishOnly: true },
  { name: "base.en", title: "Base", detail: "Fastest, English only", sizeMb: 142, downloaded: true, englishOnly: true },
];

const dictionary: DictWord[] = [
  { id: "1", word: "hams", misheard: "hams" },
  { id: "2", word: "Tauri", misheard: "tory" },
  { id: "3", word: "whisper.cpp", misheard: "whisper cpp" },
  { id: "4", word: "Priya", misheard: "pria" },
  { id: "5", word: "Kubernetes", misheard: "" },
];
const snippets: Snippet[] = [
  { id: "1", trigger: "my address", expansion: "221B Baker Street, London NW1 6XE" },
  { id: "2", trigger: "sign off", expansion: "Thanks,\nSam" },
  { id: "3", trigger: "meeting link", expansion: "https://meet.example.com/team-standup" },
];

const snapshot: Snapshot = {
  status: {
    phase,
    notice: params.get("notice"),
    modelStatus: "ready",
    modelError: null,
    polishStatus: "ready",
    polishError: null,
    polishModels: [
      { name: "qwen2.5-1.5b", title: "Compact", detail: "Qwen2.5 1.5B · quickest", sizeMb: 1066, downloaded: true, englishOnly: false },
    ],
    inputTrusted: true,
    micGranted: true,
    downloads: {},
    settings: {
      hotkey: "rightOption",
      model: "large-v3-turbo-q5_0",
      language: "auto",
      autoPaste: true,
      keepAudio: true,
      sounds: true,
      tidy: true,
      polish: true,
      polishModel: "qwen2.5-1.5b",
    },
    models: MODELS,
  },
  entries: history(),
  dictionary,
  snippets,
  scratchpad:
    "Ideas for the release post\n\n- hold a key, speak, let go: the text lands wherever the cursor is\n- nothing leaves the machine, not even analytics\n- works in Urdu and Arabic too",
  hotkeys: [
    { id: "rightOption", label: "Right Option ⌥" },
    { id: "rightCommand", label: "Right Command ⌘" },
    { id: "rightControl", label: "Right Control ⌃" },
    { id: "fn", label: "Fn / Globe" },
  ],
  platform: "macos",
  firstName: "Sam",
  inputPermission: "Accessibility",
  inputHelp: "",
};

mockWindows(pill ? "pill" : "main");
mockIPC((cmd) => (cmd === "snapshot" ? snapshot : null), { shouldMockEvents: true });

await import("../src/main");

// Feed the indicator a speaking-like level so its bars move.
if (pill && phase === "recording") {
  let t = 0;
  setInterval(() => {
    t += 1;
    const level = Math.abs(Math.sin(t / 2.3) * 0.6 + Math.sin(t / 0.9) * 0.3) + random() * 0.15;
    emit("level", Math.min(1, level));
  }, 90);
}
