// Build edition: "supervisor" for the side-by-side Supervisor build,
// "maestro" otherwise. Baked in at build time (VITE_EDITION); unset means
// regular Maestro, so every existing build behaves exactly as before.

/** Which app this is: "supervisor" or "maestro". */
export const EDITION: string = import.meta.env.VITE_EDITION ?? "maestro";

/** True in the side-by-side Supervisor build. */
export const isSupervisor: boolean = EDITION === "supervisor";

/** The name on the window, tray and Settings. */
export const APP_NAME: string = isSupervisor ? "Maestro Supervisor" : "Maestro";
