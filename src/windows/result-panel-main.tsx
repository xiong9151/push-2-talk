import React from "react";
import ReactDOM from "react-dom/client";
import "../index.css";
import ResultPanelWindow from "./ResultPanelWindow";

ReactDOM.createRoot(document.getElementById("result-panel-root")!).render(
  <React.StrictMode>
    <ResultPanelWindow />
  </React.StrictMode>,
);
