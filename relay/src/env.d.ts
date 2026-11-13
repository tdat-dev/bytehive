// Bindings and secrets of the Worker (wrangler.jsonc, .dev.vars, wrangler secret).
declare namespace Cloudflare {
  interface GlobalProps {
    mainModule: typeof import("./index");
  }
  interface Env {
    ROOM: DurableObjectNamespace<import("./room").Room>;
    APP_ORIGIN: string;
    TOKEN_SECRET: string;
    GITHUB_CLIENT_ID: string;
    GITHUB_CLIENT_SECRET: string;
    /** "1" on a local run or test: /auth/start on 127.0.0.1 or localhost skips GitHub. */
    DEV_LOGIN?: string;
    /** The private half of the VAPID key (a JWK, as JSON) and its public half (raw point, base64url). A secret and a var. */
    VAPID_PRIVATE_JWK: string;
    VAPID_PUBLIC: string;
    /** `mailto:` or `https:` contact the push services may write to. */
    VAPID_SUBJECT: string;
  }
}
interface Env extends Cloudflare.Env {}
