import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";

beforeEach(() => {
  localStorage.setItem("knoveyla.setup-complete", "true");
});

afterEach(() => {
  cleanup();
  localStorage.clear();
  window.location.hash = "";
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});
