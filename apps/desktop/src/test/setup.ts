import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";

// findBy*/waitFor default to 1s, which is too tight when the whole suite runs
// under CPU load (CI, or alongside cargo). Vitest's testTimeout is separate.
configure({ asyncUtilTimeout: 5000 });

beforeEach(() => {
  localStorage.setItem("knov.setup-complete", "true");
});

afterEach(() => {
  cleanup();
  localStorage.clear();
  sessionStorage.clear();
  window.location.hash = "";
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});
