// One store for the main window: a snapshot from Rust, kept current by its events.
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type DictWord, type Entry, type Settings, type Snapshot, type Snippet, type Status } from "./api";

interface Store extends Snapshot {
  playingId: string | null;
  updateSettings: (patch: Partial<Settings>) => void;
  setDictionary: (words: DictWord[]) => void;
  setSnippets: (snippets: Snippet[]) => void;
  setScratchpad: (text: string) => void;
  play: (entry: Entry) => void;
}

export const StoreContext = createContext<Store | null>(null);

export function useStore(): Store {
  const store = useContext(StoreContext);
  if (!store) throw new Error("useStore outside StoreProvider");
  return store;
}

/** Keeps `status` in step with the Rust side. Shared by the main window and the pill. */
export function useStatus(initial: Status | null): Status | null {
  const [status, setStatus] = useState<Status | null>(initial);
  useEffect(() => {
    let alive = true;
    if (!initial) {
      api.snapshot().then((s) => alive && setStatus((current) => current ?? s.status));
    }
    const off = listen<Status>("status", (e) => setStatus(e.payload));
    return () => {
      alive = false;
      off.then((f) => f());
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  return status;
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [playingId, setPlayingId] = useState<string | null>(null);
  const player = useRef<{ audio: HTMLAudioElement; url: string } | null>(null);
  const scratchpadTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    api.snapshot().then(setSnapshot);
    const offStatus = listen<Status>("status", (e) => setSnapshot((s) => s && { ...s, status: e.payload }));
    const offEntries = listen<Entry[]>("entries", (e) => setSnapshot((s) => s && { ...s, entries: e.payload }));
    // A transcript finished while this window had focus: type it into the text field
    // being edited, or copy it if there is none.
    const offInsert = listen<string>("insert", (e) => {
      const el = document.activeElement;
      const editable =
        el instanceof HTMLTextAreaElement ||
        (el instanceof HTMLInputElement && ["text", "search", ""].includes(el.type));
      if (editable && document.execCommand("insertText", false, e.payload)) return;
      api.copyText(e.payload.trim());
    });
    return () => {
      for (const off of [offStatus, offEntries, offInsert]) off.then((f) => f());
    };
  }, []);

  const stopPlayback = useCallback(() => {
    if (player.current) {
      player.current.audio.pause();
      URL.revokeObjectURL(player.current.url);
      player.current = null;
    }
    setPlayingId(null);
  }, []);

  const play = useCallback(
    async (entry: Entry) => {
      const wasPlaying = playingId === entry.id;
      stopPlayback();
      if (wasPlaying || !entry.audioFile) return;
      try {
        const bytes = await api.readAudio(entry.audioFile);
        const url = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
        const audio = new Audio(url);
        audio.onended = stopPlayback;
        player.current = { audio, url };
        setPlayingId(entry.id);
        await audio.play();
      } catch {
        stopPlayback();
      }
    },
    [playingId, stopPlayback],
  );

  const store = useMemo<Store | null>(() => {
    if (!snapshot) return null;
    return {
      ...snapshot,
      playingId,
      play,
      updateSettings: (patch) => {
        const settings = { ...snapshot.status.settings, ...patch };
        setSnapshot({ ...snapshot, status: { ...snapshot.status, settings } });
        api.setSettings(settings);
      },
      setDictionary: (dictionary) => {
        setSnapshot({ ...snapshot, dictionary });
        api.setDictionary(dictionary);
      },
      setSnippets: (snippets) => {
        setSnapshot({ ...snapshot, snippets });
        api.setSnippets(snippets);
      },
      setScratchpad: (scratchpad) => {
        setSnapshot({ ...snapshot, scratchpad });
        window.clearTimeout(scratchpadTimer.current);
        scratchpadTimer.current = window.setTimeout(() => api.setScratchpad(scratchpad), 600);
      },
    };
  }, [snapshot, playingId, play]);

  if (!store) return null;
  return <StoreContext.Provider value={store}>{children}</StoreContext.Provider>;
}
