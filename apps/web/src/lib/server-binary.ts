export function serverBinaryPath(targetDir?: string): string {
  return `${targetDir || "../../target"}/release/redline-web`;
}
