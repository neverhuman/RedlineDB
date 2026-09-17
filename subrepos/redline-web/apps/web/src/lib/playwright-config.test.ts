import { describe, expect, it } from "vitest";

import { serverBinaryPath } from "./server-binary";

describe("serverBinaryPath", () => {
  it("uses the repository target directory by default", () => {
    expect(serverBinaryPath()).toBe("../../target/release/redline-web");
  });

  it("honors the host runner Cargo target directory", () => {
    expect(serverBinaryPath("/tmp/jain-ci-target/redline-web")).toBe(
      "/tmp/jain-ci-target/redline-web/release/redline-web",
    );
  });
});
