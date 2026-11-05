/* company.json and the Job files. Only the app writes these; the MCP server
 * sends requests instead, so two processes never write the same file. */
import { fsCreateDir, fsCreateFile, fsReadDir, fsReadFile, fsRename, fsWriteFile } from "../ipc";
import { raiseAlert } from "./alerts";
import { parseLock, type EngineLock } from "./lock";
import { emptyCompany, parseOwned } from "./org";
import { DEFAULT_LIMITS, type Company, type Job } from "./types";

const COMPANY_REL = "company.json";
const COMPANY_TMP_REL = "company.json.tmp";
const LOCK_REL = "engine.lock";
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
  const c = parseOwned<Company>(text);
  if (c?.version !== 1 || !Array.isArray(c.people) || !Array.isArray(c.floors)) throw new Error("company.json has the wrong shape");
  return { ...c, limits: { ...DEFAULT_LIMITS, ...c.limits } }; // a limit added since it was saved, or left out by hand
}

export async function loadCompany(root: string): Promise<Company> {
  try {
    return parseCompany((await fsReadFile(root, COMPANY_REL)).content);
  } catch (e) {
    if (missing(e)) return emptyCompany();
    throw e;
  }
}

/** Through a temp file: the MCP server reads company.json at any moment, and a
 *  write cut short must not cost the company. fs_rename replaces the old file. */
export async function saveCompany(root: string, c: Company): Promise<void> {
  await writeNew(root, COMPANY_TMP_REL, JSON.stringify(c, null, 2));
  await fsRename(root, COMPANY_TMP_REL, COMPANY_REL);
}

/** A lock that can't be read is no lock: the heartbeat comes round again in moments. */
export const readLock = (root: string): Promise<EngineLock | null> =>
  fsReadFile(root, LOCK_REL).then((f) => parseLock(f.content), () => null);

/** Swapped in whole, or another app could read half a lock as none. The temp file
 *  has the pid in its name: two apps can be writing at the same moment. */
export async function writeLock(root: string, lock: EngineLock): Promise<void> {
  const tmp = `${LOCK_REL}.${lock.pid}.tmp`;
  await writeNew(root, tmp, JSON.stringify(lock));
  await fsRename(root, tmp, LOCK_REL);
}

/** Through a temp file, like company.json: a write cut short must not cost a Job. */
export async function saveJob(root: string, job: Job): Promise<void> {
  const tmp = `jobs\\${job.id}.json.tmp`;
  await writeNew(root, tmp, JSON.stringify(job, null, 2));
  await fsRename(root, tmp, `jobs\\${job.id}.json`);
}

function parseJob(text: string): Job {
  const j = parseOwned<Job>(text);
  if (typeof j?.id !== "string" || typeof j.status !== "string" || !Array.isArray(j.people)) throw new Error("not a job");
  return j;
}

export async function loadJobs(root: string): Promise<Job[]> {
  await ensureDirs(root, "jobs");
  const entries = await fsReadDir(root, "jobs");
  const out: Job[] = [];
  for (const e of entries) {
    if (e.is_dir || !e.name.endsWith(".json")) continue;
    try {
      out.push(parseJob((await fsReadFile(root, `jobs\\${e.name}`)).content));
    } catch {
      raiseAlert(`job-file-${e.name}`, `The job file ${e.name} could not be read. It was left in place: mend it and restart ByteHive.`);
    }
  }
  return out.sort((a, b) => a.createdAt - b.createdAt);
}
