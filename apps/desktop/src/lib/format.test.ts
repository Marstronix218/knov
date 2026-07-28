import { describe, expect, it } from "vitest";
import { domainFromUrl, formatDuration, formatTime } from "./format";

describe("formatDuration", () => {
  it("formats durations under one hour as whole minutes", () => {
    expect(formatDuration(3_599)).toBe("59m");
  });

  it("formats durations over one hour with zero-padded minutes", () => {
    expect(formatDuration(3_900)).toBe("1h 05m");
  });
});

describe("formatTime", () => {
  it("formats a timestamp with the runtime locale time formatter", () => {
    const timestamp = "2026-07-27T17:04:00.000Z";
    const expected = new Intl.DateTimeFormat(undefined, {
      hour: "numeric",
      minute: "2-digit",
    }).format(new Date(timestamp));

    expect(formatTime(timestamp)).toBe(expected);
  });
});

describe("domainFromUrl", () => {
  it("removes the www prefix from a valid URL hostname", () => {
    expect(domainFromUrl("https://www.example.com/path?q=1")).toBe("example.com");
  });

  it("returns undefined for malformed URLs", () => {
    expect(domainFromUrl("not a URL")).toBeUndefined();
  });

  it("returns undefined when no URL is supplied", () => {
    expect(domainFromUrl()).toBeUndefined();
  });
});
