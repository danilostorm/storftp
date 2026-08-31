import { describe, expect, it } from "vitest";
import { formatBytes, formatEta, formatSpeed } from "./format";
describe("format helpers", () => {
  it("formats byte values", () => expect(formatBytes(1024)).toContain("KB"));
  it("formats speed", () => expect(formatSpeed(1024 * 1024)).toContain("MB/s"));
  it("formats eta", () => expect(formatEta(65)).toBe("01:05"));
});
