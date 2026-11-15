// The floating indicator: level bars while recording, a spinner while transcribing,
// or a short notice. Invisible when there is nothing to say.
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useStatus } from "./store";

const BARS = 18;
const SILENT = Array<number>(BARS).fill(0);

export function Pill() {
  const status = useStatus(null);
  const [levels, setLevels] = useState(SILENT);
  const phase = status?.phase ?? "idle";

  useEffect(() => {
    const off = listen<number>("level", (e) => setLevels((old) => [...old.slice(1), e.payload]));
    return () => {
      off.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (phase !== "recording") setLevels(SILENT);
  }, [phase]);

  if (!status || (!status.notice && phase === "idle")) return null;

  return (
    <div className="pill">
      {status.notice ? (
        <span>{status.notice}</span>
      ) : phase === "recording" ? (
        <>
          <span className="pill-dot" />
          <span className="pill-bars">
            {levels.map((level, i) => (
              <span key={i} style={{ height: 4 + level * 20 }} />
            ))}
          </span>
        </>
      ) : (
        <>
          <span className="spinner" />
          <span>Transcribing</span>
        </>
      )}
    </div>
  );
}
