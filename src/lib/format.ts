export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** exponent;
  return `${value.toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

export function formatSpeed(bytesPerSecond: number): string {
  if (!Number.isFinite(bytesPerSecond) || bytesPerSecond <= 0) return "—";
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatGiB(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 GiB";
  return `${(bytes / 1e9).toFixed(2)} GiB`;
}

export function formatClockTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour12: false });
}
