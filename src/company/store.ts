/* company.json and the Job files. Only the app writes these; the MCP server
 * sends requests instead, so two processes never write the same file. */
import { fsCreateDir, fsCreateFile, fsReadDir, fsReadFile, fsWriteFile } from "../ipc";
import { emptyCompany } from "./org";
import type { Company, Job } from "./types";

const COMPANY_REL = "company.json";
const missing = (e: unknown) => /no such path/i.test(typeof e === "string" ? e : String((e as { Failed?: string })?.Failed ?? ""));

export function companyRoot(home: string): string {
  return `${home.replace(/[\\/]+$/, "")}\\.bytehive\\company`;
}

/** Create each folder of `rel` under `root`; a folder already there is fine. */
export async function ensureDirs(root: string, rel: string): Promise<void> {
  let at = "";
  for (const part of rel.split(/[\\/]/).filter(Boolean)) {
    at = at ? `${at}\\${part}` : part;
    await fsCreateDir(root, at).catch(() => {});
  }
}

/** Write a file, making it and its folders first. fs_write_file needs it to exist. */
export async function writeNew(root: string, rel: string, content: string): Promise<void> {
  const cut = rel.lastIndexOf("\\");
  if (cut > 0) await ensureDirs(root, rel.slice(0, cut));
  await fsCreateFile(root, rel).catch(() => {});
  await fsWriteFile(root, rel, content, null);
}

export function parseCompany(text: string): Company {
  const c = JSON.parse(text) as Company;
  if (c?.version !== 1 || !Array.isArray(c.people) || !Array.isArray(c.floors)) throw new Error("company.json has the wrong shape");
  return c;
}

export async function loadCompany(root: string): Promise<Company> {
  try {
    return parseCompany((await fsReadFile(root, COMPANY_REL)).content);
  } catch (e) {
    if (missing(e)) return emptyCompany();
    throw e;
  }
}

export const saveCompany = (root: string, c: Company) => writeNew(root, COMPANY_REL, JSON.stringify(c, null, 2));
export const saveJob = (root: string, job: Job) => writeNew(root, `jobs\\${job.id}.json`, JSON.stringify(job, null, 2));

export async function loadJobs(root: string): Promise<Job[]> {
  await ensureDirs(root, "jobs");
  const entries = await fsReadDir(root, "jobs");
  const out: Job[] = [];
  for (const e of entries) {
    if (e.is_dir || !e.name.endsWith(".json")) continue;
    try {
      out.push(JSON.parse((await fsReadFile(root, `jobs\\${e.name}`)).content) as Job);
    } catch { /* a half-written Job is read again next start */ }
  }
  return out.sort((a, b) => a.createdAt - b.createdAt);
}
