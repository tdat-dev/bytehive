export function authorizeUrl(clientId: string, state: string, redirect: string): string {
  const q = new URLSearchParams({ client_id: clientId, redirect_uri: redirect, state, scope: "" });
  return `https://github.com/login/oauth/authorize?${q}`;
}

/** The GitHub user id behind a login code; only the id is kept. */
export async function accountFor(code: string, env: Env): Promise<string | null> {
  const t = await fetch("https://github.com/login/oauth/access_token", {
    method: "POST",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ client_id: env.GITHUB_CLIENT_ID, client_secret: env.GITHUB_CLIENT_SECRET, code }),
  });
  const { access_token } = (await t.json()) as { access_token?: string };
  if (!access_token) return null;
  const u = await fetch("https://api.github.com/user", {
    headers: { Authorization: `Bearer ${access_token}`, "User-Agent": "bytehive-relay", Accept: "application/vnd.github+json" },
  });
  if (!u.ok) return null;
  const { id } = (await u.json()) as { id?: number };
  return id ? String(id) : null;
}
