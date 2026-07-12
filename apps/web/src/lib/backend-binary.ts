export function backendBinaryPath(targetDir?: string): string {
  return `${targetDir || "../../target"}/release/redline-web`;
}
