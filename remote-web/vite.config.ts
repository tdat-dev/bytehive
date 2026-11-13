/// <reference types="vitest/config" />
import { defineConfig } from "vite";

// Above Windows' dynamic port range here, like the app's own dev port.
export default defineConfig(({ mode }) => {
  // Dev defaults: a dev page talks to `wrangler dev`. (A per-mode env file would
  // hold these, but a hook on this machine refuses to write one.) A build sets
  // its own VITE_RELAY and VITE_RELAY_WS.
  if (mode === "development") {
    process.env.VITE_RELAY ??= "http://127.0.0.1:18787";
    process.env.VITE_RELAY_WS ??= "ws://127.0.0.1:18787";
  }
  return {
    // The wasm and the shared protocol live above this folder; without this the dev
    // server answers 403 for them and the page stays blank.
    server: { port: 17640, strictPort: true, host: "127.0.0.1", fs: { allow: [".."] } },
    plugins: [{
      // The service worker is a page of its own at a fixed address (/sw.js, so its scope is the whole
      // site). A build writes it there; in dev this answers that address with the source.
      name: "service-worker-dev",
      apply: "serve",
      configureServer(server) {
        server.middlewares.use("/sw.js", (_req, res) => {
          res.setHeader("Content-Type", "text/javascript");
          res.setHeader("Cache-Control", "no-store");
          res.end('import "/src/sw.ts";');
        });
      },
    }],
    build: {
      target: "es2022",
      sourcemap: false,
      rollupOptions: {
        input: { main: "index.html", sw: "src/sw.ts" },
        output: { entryFileNames: (chunk) => (chunk.name === "sw" ? "sw.js" : "assets/[name]-[hash].js") },
      },
    },
    test: { environment: "happy-dom", setupFiles: ["./src/test-setup.ts"] },
  };
});
