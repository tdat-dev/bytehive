// Goldens for native/src/office3d life.rs, from the office's own risingAt.
import { writeFileSync } from "node:fs";
import { risingAt } from "D:/bytehive-worktrees/office/src/company/office/life";
const seats = [{ x: 1.2, z: -0.4, rot: 0 }, { x: -2.1, z: 1.0, rot: Math.PI }, { x: 0.3, z: 0.7, rot: 0.3 }];
const out = seats.flatMap((d, i) => [true, false].map((printing) => ({ d, y: 0.5 + i * 4, seed: (i + 1) * 0.173, printing, marks: risingAt(d as any, 0.5 + i * 4, (i + 1) * 0.173, printing) })));
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", out.length);
