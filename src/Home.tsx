import { useMemo, useState } from "react";
import { api, type Entry } from "./api";
import { EmptyNote, IconButton, type Section } from "./App";
import { AlertIcon, CopyIcon, MicFillIcon, PlayIcon, SearchIcon, StopIcon, TrashIcon, WaveIcon } from "./icons";
import { useStore } from "./store";

const DAY = 86_400_000;

export function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/** Consecutive days with at least one dictation, ending today or yesterday. */
export function streakOf(entries: Entry[], now = new Date()): number {
  const days = new Set(entries.map((e) => startOfDay(new Date(e.date))));
  const day = new Date(startOfDay(now));
  if (!days.has(day.getTime())) day.setDate(day.getDate() - 1);
  let count = 0;
  while (days.has(day.getTime())) {
    count += 1;
    day.setDate(day.getDate() - 1); // calendar step, so daylight-saving days still line up
  }
  return count;
}

function dayLabel(day: number): string {
  const today = startOfDay(new Date());
  if (day === today) return "Today";
  if (day === startOfDay(new Date(today - DAY / 2))) return "Yesterday";
  return new Date(day).toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
}

function compact(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n);
}

export function Home({ go }: { go: (section: Section) => void }) {
  const { status, entries, hotkeys, firstName } = useStore();
  const [query, setQuery] = useState("");

  const hotkey = hotkeys.find((k) => k.id === status.settings.hotkey)?.label ?? "the dictation key";
  const hour = new Date().getHours();
  const part = hour < 12 ? "Good morning" : hour < 18 ? "Good afternoon" : "Good evening";
  const recording = status.phase === "recording";

  const totalWords = entries.reduce((sum, e) => sum + e.words, 0);
  const seconds = entries.reduce((sum, e) => sum + e.duration, 0);
  const wpm = seconds > 1 ? Math.round(totalWords / (seconds / 60)) : 0;
  const streak = useMemo(() => streakOf(entries), [entries]);

  const q = query.trim().toLowerCase();
  const groups = useMemo(() => {
    const filtered = q ? entries.filter((e) => e.text.toLowerCase().includes(q)) : entries;
    const byDay = new Map<number, Entry[]>();
    for (const e of filtered) {
      const day = startOfDay(new Date(e.date));
      byDay.set(day, [...(byDay.get(day) ?? []), e]);
    }
    return [...byDay.entries()]
      .sort((a, b) => b[0] - a[0])
      .map(([day, items]) => ({ day, items: items.sort((a, b) => b.date.localeCompare(a.date)) }));
  }, [entries, q]);

  return (
    <div className="page">
      <div className="page-inner">
        <header className="home-header">
          <div>
            <h1>{firstName ? `${part}, ${firstName}` : part}</h1>
            <p className="muted">Hold {hotkey} anywhere and speak. Release to type.</p>
          </div>
          <button
            className={`button primary large${recording ? " danger" : ""}`}
            disabled={status.phase === "transcribing"}
            onClick={() => api.toggleRecording()}
          >
            {recording ? <StopIcon /> : <MicFillIcon />}
            {recording ? "Stop" : "Dictate"}
          </button>
        </header>

        <SetupChecklist go={go} />

        <div className="stats">
          <Stat value={compact(totalWords)} label="words dictated" />
          <Stat value={String(wpm)} label="words per minute" />
          <Stat value={String(streak)} label="day streak" />
        </div>

        <div className="section-head">
          <h2>History</h2>
          <label className="search">
            <SearchIcon size={14} />
            <input placeholder="Search" value={query} onChange={(e) => setQuery(e.target.value)} />
          </label>
        </div>

        {entries.length === 0 ? (
          <EmptyNote
            icon={<WaveIcon size={28} />}
            title="Nothing dictated yet"
            message="Your transcripts appear here. They are stored only on this computer."
          />
        ) : groups.length === 0 ? (
          <EmptyNote
            icon={<SearchIcon size={28} />}
            title="No matches"
            message={`Nothing in your history contains “${query.trim()}”.`}
          />
        ) : (
          groups.map((group) => (
            <section key={group.day} className="day">
              <h4>{dayLabel(group.day)}</h4>
              <div className="card">
                {group.items.map((entry) => (
                  <EntryRow key={entry.id} entry={entry} />
                ))}
              </div>
            </section>
          ))
        )}
      </div>
    </div>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="stat">
      <div className="stat-value">{value}</div>
      <div className="muted">{label}</div>
    </div>
  );
}

function EntryRow({ entry }: { entry: Entry }) {
  const { playingId, play } = useStore();
  const playing = playingId === entry.id;
  const time = new Date(entry.date).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  return (
    <div className={`row entry${playing ? " playing" : ""}`}>
      <div className="entry-meta">
        <div>{time}</div>
        {entry.app && <div className="faint">{entry.app}</div>}
      </div>
      <div className="entry-text">{entry.text}</div>
      <div className="entry-actions">
        {entry.audioFile && (
          <IconButton label={playing ? "Stop" : "Play recording"} onClick={() => play(entry)}>
            {playing ? <StopIcon size={13} /> : <PlayIcon size={13} />}
          </IconButton>
        )}
        <IconButton label="Copy" onClick={() => api.copyText(entry.text)}>
          <CopyIcon size={15} />
        </IconButton>
        <IconButton label="Delete" onClick={() => api.deleteEntry(entry.id)}>
          <TrashIcon size={15} />
        </IconButton>
      </div>
    </div>
  );
}

function SetupChecklist({ go }: { go: (section: Section) => void }) {
  const { status, platform, inputPermission, inputHelp } = useStore();
  if (status.micGranted && status.inputTrusted && status.modelStatus !== "missing") return null;
  const mac = platform === "macos";
  return (
    <div className="setup">
      <h2>Finish setup</h2>
      {!status.micGranted && (
        <SetupRow title="Allow microphone" detail="Needed to hear you.">
          {mac && (
            <button className="button" onClick={() => api.openMicSettings()}>
              Open Settings
            </button>
          )}
        </SetupRow>
      )}
      {!status.inputTrusted && (
        <SetupRow
          title={`Allow ${inputPermission}`}
          detail={`Needed for the global key and for typing into other apps. ${inputHelp}`}
        >
          {mac && (
            <button className="button" onClick={() => api.requestInputAccess()}>
              Grant
            </button>
          )}
          <button className="button" onClick={() => api.restartApp()}>
            Restart hams
          </button>
        </SetupRow>
      )}
      {status.modelStatus === "missing" && (
        <SetupRow title="Download a speech model" detail="Runs fully offline once installed.">
          <button className="button" onClick={() => go("settings")}>
            Choose
          </button>
        </SetupRow>
      )}
    </div>
  );
}

function SetupRow({ title, detail, children }: { title: string; detail: string; children: React.ReactNode }) {
  return (
    <div className="setup-row">
      <span className="warn">
        <AlertIcon size={18} />
      </span>
      <div className="grow">
        <div className="strong">{title}</div>
        <div className="muted small">{detail}</div>
      </div>
      <div className="setup-actions">{children}</div>
    </div>
  );
}
