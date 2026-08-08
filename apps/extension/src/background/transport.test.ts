import { describe, expect, it } from "vitest";
import { DEFAULT_CONFIG } from "../shared/config";
import type { NativeResponse } from "../shared/types";
import { configWithDesktopPolicy } from "./transport";

function status(response: Partial<NativeResponse>): NativeResponse {
  return {
    protocolVersion: 1,
    requestId: "request",
    ok: true,
    ...response
  };
}

describe("desktop privacy policy sync", () => {
  it("applies and normalizes the desktop exclusion policy", () => {
    const config = {
      ...DEFAULT_CONFIG,
      collectionEnabled: true,
      excludedDomains: ["old.example"]
    };

    expect(
      configWithDesktopPolicy(
        config,
        status({ excludedDomains: [" Private.Example ", "private.example"] })
      )
    ).toMatchObject({
      collectionEnabled: true,
      excludedDomains: ["private.example"]
    });
  });

  it("preserves local exclusions for older desktop responses", () => {
    const config = {
      ...DEFAULT_CONFIG,
      collectionEnabled: true,
      excludedDomains: ["local.example"]
    };

    expect(configWithDesktopPolicy(config, status({})).excludedDomains).toEqual([
      "local.example"
    ]);
  });

  it("honors a desktop pause without remotely enabling a local pause", () => {
    const enabled = { ...DEFAULT_CONFIG, collectionEnabled: true };
    const paused = { ...DEFAULT_CONFIG, collectionEnabled: false };

    expect(
      configWithDesktopPolicy(enabled, status({ collectionEnabled: false }))
        .collectionEnabled
    ).toBe(false);
    expect(
      configWithDesktopPolicy(paused, status({ collectionEnabled: true }))
        .collectionEnabled
    ).toBe(false);
  });
});
