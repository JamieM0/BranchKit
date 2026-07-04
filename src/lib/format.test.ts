import { describe, expect, it } from "vitest";
import { relativeTime } from "./format";

describe("relativeTime", () => {
  const now = 1_700_000_000;

  it("shows just now under a minute", () => {
    expect(relativeTime(now - 30, now)).toBe("just now");
  });

  it("shows minutes under an hour", () => {
    expect(relativeTime(now - 5 * 60, now)).toBe("5m ago");
  });

  it("shows hours under a day", () => {
    expect(relativeTime(now - 3 * 3600, now)).toBe("3h ago");
  });

  it("shows days beyond that", () => {
    expect(relativeTime(now - 2 * 86400, now)).toBe("2d ago");
  });

  it("never goes negative for a clock skewed into the future", () => {
    expect(relativeTime(now + 1000, now)).toBe("just now");
  });
});
