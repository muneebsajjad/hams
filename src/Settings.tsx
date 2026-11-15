import { useState, type ReactNode } from "react";
import { api, type Model } from "./api";
import { IconButton, Page } from "./App";
import { CheckIcon, CrossIcon, TrashIcon } from "./icons";
import { useStore } from "./store";

const LANGUAGES: [code: string, name: string][] = [
  ["en", "English"],
  ["auto", "Auto-detect"],
  ["ur", "Urdu"],
  ["hi", "Hindi"],
  ["ar", "Arabic"],
  ["es", "Spanish"],
  ["fr", "French"],
  ["de", "German"],
];

export function SettingsPage() {
  const { status, entries, hotkeys, platform, inputPermission, inputHelp, updateSettings } = useStore();
  const [confirmClear, setConfirmClear] = useState(false);
  const { settings } = status;
  const active = status.models.find((m) => m.name === settings.model);
  const mac = platform === "macos";

  return (
    <Page title="Settings" subtitle="Everything runs and stays on this computer.">
      <Group title="Dictation">
        <Line label="Push-to-talk key">
          <select value={settings.hotkey} onChange={(e) => updateSettings({ hotkey: e.target.value })}>
            {hotkeys.map((k) => (
              <option key={k.id} value={k.id}>
                {k.label}
              </option>
            ))}
          </select>
        </Line>
        <Line label="Language">
          <select
            value={active?.englishOnly ? "en" : settings.language}
            disabled={active?.englishOnly}
            onChange={(e) => updateSettings({ language: e.target.value })}
          >
            {LANGUAGES.map(([code, name]) => (
              <option key={code} value={code}>
                {name}
              </option>
            ))}
          </select>
        </Line>
        <Line label="Type into the focused app">
          <Toggle on={settings.autoPaste} onChange={(autoPaste) => updateSettings({ autoPaste })} />
        </Line>
        <Line label="Tidy long dictations" hint="From four sentences up, drops filler sounds and repeated words.">
          <Toggle on={settings.tidy} onChange={(tidy) => updateSettings({ tidy })} />
        </Line>
        <Line label="Keep audio recordings">
          <Toggle on={settings.keepAudio} onChange={(keepAudio) => updateSettings({ keepAudio })} />
        </Line>
        <Line label="Start and stop sounds">
          <Toggle on={settings.sounds} onChange={(sounds) => updateSettings({ sounds })} />
        </Line>
      </Group>

      <Group title="Speech model">
        {status.models.map((m) => (
          <div key={m.name} className="row">
            <div className="grow">
              <div className="strong">{m.title}</div>
              <div className="muted small">
                {m.detail} · {m.sizeMb} MB
              </div>
            </div>
            <ModelControls model={m} />
          </div>
        ))}
      </Group>
      {status.modelStatus === "failed" && <p className="error small">{status.modelError}</p>}

      <Group title="Cleanup model">
        <Line
          label="Polish long dictations"
          hint="From four sentences up, a small language model on this computer fixes grammar, stumbles and self-corrections. Adds about a second."
        >
          <Toggle on={settings.polish} onChange={(polish) => updateSettings({ polish })} />
        </Line>
        {status.polishModels.map((m) => (
          <div key={m.name} className="row">
            <div className="grow">
              <div className="strong">{m.title}</div>
              <div className="muted small">
                {m.detail} · {(m.sizeMb / 1024).toFixed(1)} GB
              </div>
            </div>
            <PolishControls model={m} />
          </div>
        ))}
      </Group>
      {status.polishStatus === "failed" && <p className="error small">{status.polishError}</p>}

      <Group title="Permissions">
        <Line label="Microphone">
          <Granted ok={status.micGranted} />
          {!status.micGranted && mac && (
            <button className="button" onClick={() => api.openMicSettings()}>
              Open
            </button>
          )}
        </Line>
        <Line label={inputPermission}>
          <Granted ok={status.inputTrusted} />
          {!status.inputTrusted && mac && (
            <button className="button" onClick={() => api.requestInputAccess()}>
              Open
            </button>
          )}
          {!status.inputTrusted && (
            <button className="button" onClick={() => api.restartApp()}>
              Restart hams
            </button>
          )}
        </Line>
      </Group>
      {!status.inputTrusted && <p className="muted small">{inputHelp}</p>}

      <Group title="Data">
        <Line label="Transcripts, audio and models">
          <button className="button" onClick={() => api.openDataFolder()}>
            Show folder
          </button>
        </Line>
        <Line label={`History (${entries.length} ${entries.length === 1 ? "item" : "items"})`}>
          {confirmClear ? (
            <>
              <span className="muted small">Delete all transcripts and recordings? This cannot be undone.</span>
              <button
                className="button danger"
                onClick={() => {
                  api.clearHistory();
                  setConfirmClear(false);
                }}
              >
                Delete everything
              </button>
              <button className="button" onClick={() => setConfirmClear(false)}>
                Cancel
              </button>
            </>
          ) : (
            <button className="button danger-text" disabled={entries.length === 0} onClick={() => setConfirmClear(true)}>
              Clear…
            </button>
          )}
        </Line>
      </Group>
    </Page>
  );
}

function ModelControls({ model }: { model: Model }) {
  const { status, updateSettings } = useStore();
  const progress = status.downloads[model.name];
  if (progress !== undefined) {
    return (
      <>
        <progress value={progress} max={1} />
        <span className="percent">{Math.floor(progress * 100)}%</span>
      </>
    );
  }
  if (!model.downloaded) {
    return (
      <button className="button" onClick={() => api.downloadModel(model.name)}>
        Download
      </button>
    );
  }
  if (status.settings.model === model.name) {
    return (
      <span className="ok">
        <CheckIcon />
        {status.modelStatus === "loading" ? "Loading" : "Active"}
      </span>
    );
  }
  return (
    <>
      <button className="button" onClick={() => updateSettings({ model: model.name })}>
        Use
      </button>
      <IconButton label="Delete download" onClick={() => api.deleteModel(model.name)}>
        <TrashIcon size={15} />
      </IconButton>
    </>
  );
}

function PolishControls({ model }: { model: Model }) {
  const { status, updateSettings } = useStore();
  const progress = status.downloads[model.name];
  if (progress !== undefined) {
    return (
      <>
        <progress value={progress} max={1} />
        <span className="percent">{Math.floor(progress * 100)}%</span>
      </>
    );
  }
  if (!model.downloaded) {
    return (
      <button className="button" onClick={() => api.downloadModel(model.name)}>
        Download
      </button>
    );
  }
  const chosen = status.settings.polishModel === model.name;
  return (
    <>
      {chosen && status.settings.polish ? (
        <span className="ok">
          <CheckIcon />
          {status.polishStatus === "loading" ? "Loading" : status.polishStatus === "ready" ? "Active" : "Selected"}
        </span>
      ) : (
        <button className="button" onClick={() => updateSettings({ polish: true, polishModel: model.name })}>
          Use
        </button>
      )}
      <IconButton label="Delete download" onClick={() => api.deleteModel(model.name)}>
        <TrashIcon size={15} />
      </IconButton>
    </>
  );
}

function Granted({ ok }: { ok: boolean }) {
  return (
    <span className={ok ? "ok" : "bad"}>
      {ok ? <CheckIcon /> : <CrossIcon />}
      {ok ? "Granted" : "Not granted"}
    </span>
  );
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="group">
      <h2>{title}</h2>
      <div className="card">{children}</div>
    </section>
  );
}

function Line({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="row">
      <span>
        {label}
        {hint && <span className="muted small hint">{hint}</span>}
      </span>
      <span className="grow" />
      {children}
    </div>
  );
}

function Toggle({ on, onChange }: { on: boolean; onChange: (on: boolean) => void }) {
  return (
    <button role="switch" aria-checked={on} className={`toggle${on ? " on" : ""}`} onClick={() => onChange(!on)}>
      <span />
    </button>
  );
}
