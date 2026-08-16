import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// Design tokens — SSOT visual em _docs/DESIGN-SYSTEM-APP.md
import "./styles/tokens.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
