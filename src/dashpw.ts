// Words and checks for Settings > Remote > Password (the phone dashboard's
// password, kept as a hash by the app: dashauth.rs). No DOM: unit-tested.

/** Shortest password the app accepts (dashauth.rs MIN_LEN). */
export const MIN_PASSWORD = 10;

/** What is wrong with a new password typed twice, or null when it will do. */
export function passwordProblem(first: string, again: string): string | null {
  if (!first) return "Type a password.";
  if ([...first].length < MIN_PASSWORD) return `Use at least ${MIN_PASSWORD} characters.`;
  if (first !== again) return "The two don't match.";
  return null;
}

/** The Password row's line, and the LAN toggle's, for whether one is set. */
export function passwordNotes(hasPassword: boolean): { pw: string; lan: string } {
  return hasPassword
    ? {
      pw: "Set. Anyone opening the dashboard signs in with it, this computer included.",
      lan: "Reachable from devices on the same Wi-Fi, after signing in. Only turn on for a network you trust.",
    }
    : {
      pw: "None yet. Other devices need one before they can connect.",
      lan: "Set a password first. Without one, anyone on your network could type into your agents.",
    };
}
