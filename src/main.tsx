import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { App } from "./App";
import { Pill } from "./Pill";
import { StoreProvider } from "./store";
import "./styles.css";

// Both windows load this file; the floating indicator is the one labelled "pill".
const isPill = getCurrentWindow().label === "pill";
document.documentElement.classList.toggle("pill-window", isPill);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {isPill ? (
      <Pill />
    ) : (
      <StoreProvider>
        <App />
      </StoreProvider>
    )}
  </StrictMode>,
);
