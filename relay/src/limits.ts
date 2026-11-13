/** Per account; the real numbers get set after measuring cost (spec, step 6). */
export const LIMITS = { machines: 5, devices: 10, connections: 20, bytesPerDay: 2 * 1024 ** 3 };
/** Bytes counted in memory before they are written down. */
export const FLUSH_BYTES = 1024 * 1024;
