/**
 * Validates a port string and returns the parsed port number.
 * Returns null if invalid (non-numeric, < 1, or > 65535).
 */
export function validatePort(value: string): number | null {
  const port = parseInt(value, 10);
  if (isNaN(port) || port < 1 || port > 65535) {
    return null;
  }
  return port;
}
