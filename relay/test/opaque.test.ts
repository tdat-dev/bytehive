import { env } from "cloudflare:workers";
import { runInDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { Room } from "../src/room";

async function join(room: string, peer: string) {
  const r = await env.ROOM.getByName(room).fetch(`https://r/ws?peer=${peer}&name=${peer}`, { headers: { Upgrade: "websocket" } });
  const ws = r.webSocket!;
  ws.accept();
  ws.binaryType = "arraybuffer";
  const got: Uint8Array[] = [];
  ws.addEventListener("message", (e) => { if (typeof e.data !== "string") got.push(new Uint8Array(e.data)); });
  return { ws, got };
}

const frame = (to: string, payload: Uint8Array) => {
  const p = new TextEncoder().encode(to);
  return Uint8Array.from([1, 3, p.length, ...p, ...payload]);
};
const settle = () => new Promise((r) => setTimeout(r, 100));

describe("the relay is opaque", () => {
  it("passes a payload on byte for byte and keeps none of it", async () => {
    const payload = crypto.getRandomValues(new Uint8Array(512));
    // A readable marker too: what a leaked JSON message would look like.
    payload.set(new TextEncoder().encode('{"type":"chat.send","text":"hunter2"}'), 100);

    const machine = await join("opaque", "m:desk");
    const device = await join("opaque", "d:phone");
    device.ws.send(frame("m:desk", payload));
    await settle();

    expect(machine.got).toHaveLength(1);
    const delivered = machine.got[0];
    const peerLen = delivered[2];
    expect([...delivered.subarray(3, 3 + peerLen)]).toEqual([...new TextEncoder().encode("d:phone")]);
    expect([...delivered.subarray(3 + peerLen)]).toEqual([...payload]);

    // Closing writes down whatever the room counts (the usage row).
    device.ws.close(1000, "done");
    machine.ws.close(1000, "done");
    await settle();

    const dump = await runInDurableObject(env.ROOM.getByName("opaque"), (_: Room, state) => {
      const sql = state.storage.sql;
      const tables = sql.exec<{ name: string }>("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE '_cf_%' AND name NOT LIKE 'sqlite_%'").toArray().map((t) => t.name);
      const rows: Record<string, unknown[]> = {};
      for (const t of tables) rows[t] = sql.exec(`SELECT * FROM ${t}`).toArray();
      return rows;
    });

    // Notes and subscriptions are the notification tables, the rest the session
    // store (sealed by machines over HTTP): nothing of a frame goes in any of them.
    expect(Object.keys(dump).sort()).toEqual(["notes", "peers", "segments", "sessions", "subs", "usage", "vault"]);
    expect([dump.notes, dump.subs, dump.segments, dump.sessions, dump.vault]).toEqual([[], [], [], [], []]);
    const text = JSON.stringify(dump);
    const needles = {
      marker: "hunter2",
      hex: [...payload.subarray(0, 24)].map((b) => b.toString(16).padStart(2, "0")).join(""),
      base64: btoa(String.fromCharCode(...payload.subarray(0, 24))),
    };
    for (const [what, n] of Object.entries(needles)) expect(text.includes(n), what).toBe(false);
    // The search can see what the room does keep: names, and a byte count.
    expect(text.includes("d:phone")).toBe(true);
    expect(dump.usage).toEqual([{ day: expect.any(String), bytes: frame("m:desk", payload).length }]);
  });
});
