import { describe, expect, it } from "vitest";

import { backendBinaryPath } from "./backend-binary";

describe("backendBinaryPath", () => {
  it("uses the repository target directory by default", () => {
    expect(backendBinaryPath()).toBe("../../target/release/redline-web");
  });

  it("honors the host runner Cargo target directory", () => {
    expect(backendBinaryPath("/tmp/jain-ci-target/redline-web")).toBe(
      "/tmp/jain-ci-target/redline-web/release/redline-web",
    );
  });
});
