/* The roles ByteHive ships with. A copy lands in ~/.bytehive/company/roles the
 * first time, and that copy wins, so a role you edit stays edited. */
import { fsReadDir, fsReadFile } from "../ipc";
import { parseOwned } from "./org";
import { writeNew } from "./store";
import type { RoleDef } from "./types";

const jsons = import.meta.glob("./roles/*/role.json", { eager: true, import: "default" }) as Record<string, Omit<RoleDef, "prompt">>;
const prompts = import.meta.glob("./roles/*/prompt.md", { eager: true, query: "?raw", import: "default" }) as Record<string, string>;
const skills = import.meta.glob("./roles/*/skills/*/SKILL.md", { eager: true, query: "?raw", import: "default" }) as Record<string, string>;

export const DEFAULT_ROLES: RoleDef[] = Object.entries(jsons).map(([path, def]) => ({
  ...def,
  prompt: prompts[path.replace("role.json", "prompt.md")],
}));

export function skillFiles(role: RoleDef): Array<{ name: string; text: string }> {
  return role.skills.flatMap((name) => {
    const text = skills[`./roles/${role.id}/skills/${name}/SKILL.md`];
    return text ? [{ name, text }] : [];
  });
}

export async function seedRoles(root: string): Promise<void> {
  for (const role of DEFAULT_ROLES) {
    const exists = await fsReadFile(root, `roles\\${role.id}\\role.json`).then(() => true, () => false);
    if (exists) continue;
    const { prompt, ...def } = role;
    await writeNew(root, `roles\\${role.id}\\role.json`, JSON.stringify(def, null, 2));
    await writeNew(root, `roles\\${role.id}\\prompt.md`, prompt);
    for (const s of skillFiles(role)) await writeNew(root, `roles\\${role.id}\\skills\\${s.name}\\SKILL.md`, s.text);
  }
}

export async function loadRoles(root: string): Promise<RoleDef[]> {
  const out: RoleDef[] = [];
  for (const e of await fsReadDir(root, "roles")) {
    try {
      const def = parseOwned<Omit<RoleDef, "prompt">>((await fsReadFile(root, `roles\\${e.name}\\role.json`)).content);
      out.push({ ...def, prompt: (await fsReadFile(root, `roles\\${e.name}\\prompt.md`)).content });
    } catch { /* a role folder missing a file is not offered */ }
  }
  return out;
}
