import { describe, expect, it } from "vitest";
import { domainFromUrl, formatDuration, formatPercentage, formatTime } from "./format";

describe("formatDuration", () => {
  it("formats durations under one hour as whole minutes", () => {
    expect(formatDuration(3_599)).toBe("59m");
  });

  it("formats durations over one hour with zero-padded minutes", () => {
    expect(formatDuration(3_900)).toBe("1h 05m");
  });
});

describe("formatPercentage", () => {
  it("rounds percentages to exactly one decimal place", () => {
    expect(formatPercentage(58.333333333333336)).toBe("58.3%");
    expect(formatPercentage(41.666666666666664)).toBe("41.7%");
    expect(formatPercentage(0)).toBe("0.0%");
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
