// HTTP routes of the session store (store.ts). Every route needs a login; only a
// machine writes, any peer of the account reads. What is read is sealed.

import { verify } from "./token";

/** The signed-in peer, or the response that turns the request away. */
async function who(req: Request, env: Env): Promise<{ account: string; peer: string } | Response> {
  const login = await verify((req.headers.get("Authorization") ?? "").replace(/^Bearer /, ""), env.TOKEN_SECRET, "login");
  if (!login || typeof login.account !== "string" || typeof login.peer !== "string") return new Response("sign in", { status: 401 });
  if (await env.ROOM.getByName(login.account).isRevoked(login.peer)) return new Response("revoked", { status: 403 });
  return { account: login.account, peer: login.peer };
}

const body = (req: Request) => req.json().catch(() => null) as Promise<Record<string, unknown> | null>;
const answer = (r: string) =>
  r === "ok" ? Response.json({ ok: true }) : new Response(r, { status: r === "quota" ? 413 : r === "exists" || r === "taken" ? 409 : 400 });

/** The store's answer to `req`, or null when the path is not one of its routes. */
export async function storeRoute(url: URL, req: Request, env: Env): Promise<Response | null> {
  const routes = ["/vault", "/store/put", "/store/drop", "/store/list", "/store/segments"];
  if (!routes.includes(url.pathname)) return null;
  const w = await who(req, env);
  if (w instanceof Response) return w;
  const room = env.ROOM.getByName(w.account);
  const machine = w.peer.startsWith("m:");
  const writes = req.method === "PUT" || req.method === "POST";
  if (writes && !machine) return new Response("machines only", { status: 403 });

  switch (`${req.method} ${url.pathname}`) {
    case "GET /vault": {
      const v = await room.getVault();
      return v ? Response.json(v) : new Response("no vault", { status: 404 });
    }
    case "PUT /vault": {
      const b = await body(req);
      return answer(await room.setVault(b?.vault, b?.replace === true));
    }
    case "POST /store/put":
      return answer(await room.putSession(w.peer, await body(req)));
    case "POST /store/drop":
      await room.dropSession(w.peer, (await body(req))?.sid);
      return Response.json({ ok: true });
    case "GET /store/list":
      return Response.json(await room.listSessions());
    case "GET /store/segments":
      return Response.json(await room.sessionSegments(url.searchParams.get("sid") ?? "", Number(url.searchParams.get("from") ?? 0)));
  }
  return new Response("method not allowed", { status: 405 });
}
