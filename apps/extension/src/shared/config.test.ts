import { describe, expect, it } from "vitest";
import {
  DEFAULT_CONFIG,
  isExcludedUrl,
  normalizeDomains,
  normalizeLoopbackEndpoint
} from "./config";

describe("domain exclusions", () => {
  it("starts disabled and unbound until the user approves a profile", () => {
    expect(DEFAULT_CONFIG.collectionEnabled).toBe(false);
    expect(DEFAULT_CONFIG.browserProfileId).toBe("");
  });

  it("normalizes, deduplicates, and sorts domains", () => {
    expect(
      normalizeDomains([" Example.com ", "https://private.example/path", "example.com"])
    ).toEqual(["example.com", "private.example"]);
  });

  it("matches only exact hosts and subdomains", () => {
    expect(isExcludedUrl("https://docs.example.com/a", ["example.com"])).toBe(true);
    expect(isExcludedUrl("https://example.com/a", ["example.com"])).toBe(true);
    expect(isExcludedUrl("https://notexample.com/a", ["example.com"])).toBe(false);
  });
});

describe("loopback endpoint validation", () => {
  it("accepts supported loopback addresses", () => {
    expect(normalizeLoopbackEndpoint("http://127.0.0.1:48321/")).toBe(
      "http://127.0.0.1:48321"
    );
    expect(normalizeLoopbackEndpoint("http://localhost:48321")).toBe(
      "http://localhost:48321"
    );
  });

  it("rejects remote and encrypted endpoints", () => {
    expect(() => normalizeLoopbackEndpoint("https://127.0.0.1:48321")).toThrow();
    expect(() => normalizeLoopbackEndpoint("http://example.com:48321")).toThrow();
  });
});
