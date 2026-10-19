import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { QuickAddWindow } from "./components/QuickAddWindow";
import { DayPreviewWindow } from "./components/DayPreviewWindow";

function Root() {
  switch (window.location.hash) {
    case "#quickadd":
      return <QuickAddWindow />;
    case "#preview":
      return <DayPreviewWindow />;
    default:
      return <App />;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
