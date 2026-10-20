import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { GlobalNotices } from "./components/GlobalNotices";
import { errorMessage } from "./components/ui";
import { notifyError } from "./lib/notices";

window.addEventListener("unhandledrejection", (event) => {
  console.error("Unhandled promise rejection", event.reason);
  notifyError(`Something went wrong: ${errorMessage(event.reason)}`);
});
window.addEventListener("error", (event) => {
  // Benign browser layout notice, not an app failure.
  if (event.message?.includes("ResizeObserver loop")) return;
  console.error("Uncaught error", event.error ?? event.message);
  notifyError(`Something went wrong: ${event.error ? errorMessage(event.error) : event.message}`);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    <GlobalNotices />
  </React.StrictMode>,
);
