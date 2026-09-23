import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { DebugModeProvider } from "./debug/DebugModeContext";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <DebugModeProvider>
      <App />
    </DebugModeProvider>
  </React.StrictMode>,
);
