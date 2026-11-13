import { accountFor, authorizeUrl } from "./github";
import { allowedEndpoint } from "./push";
import { PEER } from "./room";
import { storeRoute } from "./storeroutes";
import { sign, verify } from "./token";

export { Room } from "./room";

const TOKEN_SECONDS = 90 * 24 * 3600;
const TICKET_SECONDS = 60;
const STATE_SECONDS = 600;
const LOOPBACK = new Set(["127.0.0.1", "localhost"]);

function cors(env: Env, res: Response): Response {
  const r = new Response(res.body, res);
  r.headers.set("Access-Control-Allow-Origin", env.APP_ORIGIN);
  r.headers.set("Access-Control-Allow-Headers", "Authorization, Content-Type");
  r.headers.set("Access-Control-Allow-Methods", "GET, POST, PUT, OPTIONS");
  return r;
}

/**
 * Hand the login token to whoever asked for the sign-in, and only to them: a
 * machine's app listens on its own loopback port, a browser gets it in the URL
 * fragment (which never reaches a server). No other target exists.
 */
async function finish(env: Env, account: string, peer: string, port: number, clientState: string): Promise<Response> {
  const token = await sign({ typ: "login", account, peer }, TOKEN_SECONDS, env.TOKEN_SECRET);
  if (peer.startsWith("m:")) {
    const back = new URL(`http://127.0.0.1:${port}/callback`);
    back.searchParams.set("token", token);
    back.searchParams.set("state", clientState);
    return Response.redirect(back.toString(), 302);
  }
  return Response.redirect(`${env.APP_ORIGIN}/#token=${encodeURIComponent(token)}`, 302);
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const q = url.searchParams;
    if (req.method === "OPTIONS") return cors(env, new Response(null, { status: 204 }));
    const stored = await storeRoute(url, req, env);
    if (stored) return cors(env, stored);

    switch (url.pathname) {
      case "/auth/start": {
        const peer = q.get("peer") ?? "";
        const clientState = (q.get("state") ?? "").slice(0, 200);
        const port = Number(q.get("port"));
        if (!PEER.test(peer)) return new Response("bad peer", { status: 400 });
        if (peer.startsWith("m:") && !(Number.isInteger(port) && port >= 1024 && port <= 65535)) {
          return new Response("bad port", { status: 400 });
        }
        // Local runs and tests only: DEV_LOGIN is never set on the deployed Worker,
        // and even where it is, only a loopback request skips GitHub.
        if (env.DEV_LOGIN === "1" && LOOPBACK.has(url.hostname)) {
          return finish(env, q.get("account") ?? "dev", peer, port, clientState);
        }
        const state = await sign({ typ: "state", peer, port, state: clientState }, STATE_SECONDS, env.TOKEN_SECRET);
        return Response.redirect(authorizeUrl(env.GITHUB_CLIENT_ID, state, `${url.origin}/auth/callback`), 302);
      }
      case "/auth/callback": {
        const s = await verify(q.get("state") ?? "", env.TOKEN_SECRET, "state");
        const account = s ? await accountFor(q.get("code") ?? "", env) : null;
        if (!s || !account) return new Response("Sign-in failed.", { status: 400 });
        return finish(env, account, String(s.peer), Number(s.port), String(s.state));
      }
      case "/ticket": {
        if (req.method !== "POST") return cors(env, new Response("method not allowed", { status: 405 }));
        const login = await verify((req.headers.get("Authorization") ?? "").replace(/^Bearer /, ""), env.TOKEN_SECRET, "login");
        if (!login || typeof login.account !== "string" || typeof login.peer !== "string") {
          return cors(env, new Response("sign in", { status: 401 }));
        }
        const body = (await req.json().catch(() => null)) as { name?: unknown } | null;
        const name = typeof body?.name === "string" ? body.name.slice(0, 80) : "";
        if (await env.ROOM.getByName(login.account).isRevoked(login.peer)) {
          return cors(env, new Response("revoked", { status: 403 }));
        }
        const ticket = await sign({ typ: "ticket", account: login.account, peer: login.peer, name }, TICKET_SECONDS, env.TOKEN_SECRET);
        return cors(env, Response.json({ ticket }));
      }
      case "/push/key":
        return cors(env, env.VAPID_PUBLIC ? Response.json({ key: env.VAPID_PUBLIC }) : new Response("not set up", { status: 404 }));
      case "/push/subscribe":
      case "/push/pending": {
        if (req.method !== "POST") return cors(env, new Response("method not allowed", { status: 405 }));
        const login = await verify((req.headers.get("Authorization") ?? "").replace(/^Bearer /, ""), env.TOKEN_SECRET, "login");
        // A device's own login: the note it may take is the one for the peer in its token.
        if (!login || typeof login.account !== "string" || typeof login.peer !== "string" || !login.peer.startsWith("d:")) {
          return cors(env, new Response("sign in", { status: 401 }));
        }
        const room = env.ROOM.getByName(login.account);
        if (await room.isRevoked(login.peer)) return cors(env, new Response("revoked", { status: 403 }));
        if (url.pathname === "/push/pending") return cors(env, Response.json(await room.takePush(login.peer)));
        const body = (await req.json().catch(() => null)) as { endpoint?: unknown } | null;
        if (typeof body?.endpoint !== "string" || !allowedEndpoint(body.endpoint, env.DEV_LOGIN === "1")) {
          return cors(env, new Response("bad request", { status: 400 }));
        }
        await room.subscribe(login.peer, body.endpoint);
        return cors(env, Response.json({ ok: true }));
      }
      case "/ws": {
        const t = await verify(q.get("ticket") ?? "", env.TOKEN_SECRET, "ticket");
        if (!t || typeof t.account !== "string" || typeof t.peer !== "string") return new Response("sign in", { status: 401 });
        const to = new URL("https://room/ws");
        to.searchParams.set("peer", t.peer);
        to.searchParams.set("name", String(t.name ?? ""));
        return env.ROOM.getByName(t.account).fetch(new Request(to, req));
      }
    }
    return new Response("not found", { status: 404 });
  },
} satisfies ExportedHandler<Env>;
