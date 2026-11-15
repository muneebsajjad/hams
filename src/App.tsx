import { useState, type ReactNode } from "react";
import { BookIcon, ChartIcon, GearIcon, MicIcon, NoteIcon, ScissorsIcon } from "./icons";
import { Insights } from "./Insights";
import { Home } from "./Home";
import { DictionaryPage, ScratchpadPage, SnippetsPage } from "./Pages";
import { SettingsPage } from "./Settings";
import { useStore } from "./store";

export type Section = "home" | "insights" | "dictionary" | "snippets" | "scratchpad" | "settings";

const SECTIONS: { id: Section; label: string; icon: ReactNode }[] = [
  { id: "home", label: "Dictation", icon: <MicIcon /> },
  { id: "insights", label: "Insights", icon: <ChartIcon /> },
  { id: "dictionary", label: "Dictionary", icon: <BookIcon /> },
  { id: "snippets", label: "Snippets", icon: <ScissorsIcon /> },
  { id: "scratchpad", label: "Scratchpad", icon: <NoteIcon /> },
  { id: "settings", label: "Settings", icon: <GearIcon /> },
];

export function App() {
  const [section, setSection] = useState<Section>("home");

  return (
    <div className="shell">
      <nav className="sidebar">
        <div className="brand">hams</div>
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            className={`nav-item${section === s.id ? " active" : ""}`}
            onClick={() => setSection(s.id)}
          >
            {s.icon}
            {s.label}
          </button>
        ))}
        <EngineBadge />
      </nav>
      <main className="detail">
        {section === "home" && <Home go={setSection} />}
        {section === "insights" && <Insights />}
        {section === "dictionary" && <DictionaryPage />}
        {section === "snippets" && <SnippetsPage />}
        {section === "scratchpad" && <ScratchpadPage />}
        {section === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}

function EngineBadge() {
  const { status } = useStore();
  const active = status.models.find((m) => m.name === status.settings.model);
  const label = {
    ready: `On-device · ${active?.title ?? ""}`,
    loading: "Loading model…",
    missing: "No model installed",
    failed: "Model failed to load",
  }[status.modelStatus];
  return (
    <div className="engine" title={status.modelError ?? undefined}>
      <span className={`dot ${status.modelStatus}`} />
      <span>{label}</span>
    </div>
  );
}

// MARK: - Shared building blocks

export function Page({ title, subtitle, children }: { title: string; subtitle: string; children: ReactNode }) {
  return (
    <div className="page">
      <div className="page-inner">
        <header>
          <h1>{title}</h1>
          <p className="muted">{subtitle}</p>
        </header>
        {children}
      </div>
    </div>
  );
}

export function EmptyNote({ icon, title, message }: { icon: ReactNode; title: string; message: string }) {
  return (
    <div className="empty">
      <div className="empty-icon">{icon}</div>
      <h3>{title}</h3>
      <p className="muted">{message}</p>
    </div>
  );
}

export function IconButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button className="icon-button" title={label} aria-label={label} onClick={onClick}>
      {children}
    </button>
  );
}
