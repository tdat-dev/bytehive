use super::*;
use crate::company::types::{DEFAULT_LIMITS, Job, JobStatus};
use std::fs;
use std::path::Path;

const COMPANY_FIXTURE: &str = include_str!("fixtures/company.json");
const JOB_FIXTURE: &str = include_str!("fixtures/J-0002.json");

fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

fn job() -> Job {
    serde_json::from_str(r#"{"id":"J-0001","title":"t","ask":"a","status":"planning","plan":null,"tokenCap":1,"tokensUsed":0,"tokenBase":{},"people":[],"createdAt":1}"#).unwrap()
}

#[test]
fn the_fixtures_read_and_write_back_with_no_field_lost() {
    let c = parse_company(COMPANY_FIXTURE).unwrap();
    assert_eq!(c.people.len(), 5);
    let back: serde_json::Value = serde_json::from_str(&serde_json::to_string_pretty(&c).unwrap()).unwrap();
    assert_eq!(back, serde_json::from_str::<serde_json::Value>(COMPANY_FIXTURE).unwrap());

    let j = parse_job(JOB_FIXTURE).unwrap();
    assert_eq!((j.id.as_str(), j.status), ("J-0002", JobStatus::AwaitingMerge));
    let back: serde_json::Value = serde_json::from_str(&serde_json::to_string_pretty(&j).unwrap()).unwrap();
    assert_eq!(back, serde_json::from_str::<serde_json::Value>(JOB_FIXTURE).unwrap());
}

#[test]
fn keeps_fields_this_build_does_not_know() {
    let text = r##"{"version":1,"floors":[],"people":[{"id":"founder","name":"Founder","role":"founder","rank":"founder","floor":null,"boss":null,"cli":"claude","look":{"shirt":"#f5b83d","hair":"#2a2e3a","prop":"none"},"mood":"calm"}],"later":[1,2]}"##;
    let c = parse_company(text).unwrap();
    let back = serde_json::to_value(&c).unwrap();
    assert_eq!(back["later"], serde_json::json!([1, 2]));
    assert_eq!(back["people"][0]["mood"], "calm");
    let mut j = serde_json::to_value(job()).unwrap();
    j["reviewer"] = "ana".into();
    let back = serde_json::to_value(parse_job(&j.to_string()).unwrap()).unwrap();
    assert_eq!(back["reviewer"], "ana");
}

// pickRoot

#[test]
fn is_the_real_folder_in_a_release_build() {
    assert_eq!(pick_root("C:\\Users\\me\\", false, None), "C:\\Users\\me\\.bytehive\\company");
}

#[test]
fn ignores_an_override_in_a_release_build() {
    assert_eq!(pick_root("C:\\Users\\me", false, Some("D:\\x")), "C:\\Users\\me\\.bytehive\\company");
}

#[test]
fn is_the_sandbox_in_a_dev_build() {
    assert_eq!(pick_root("C:\\Users\\me", true, None), "C:\\Users\\me\\.bytehive\\company-dev");
}

#[test]
fn takes_an_override_in_a_dev_build_without_a_trailing_slash() {
    assert_eq!(pick_root("C:\\Users\\me", true, Some("D:\\scratch\\co\\")), "D:\\scratch\\co");
}

#[test]
fn writes_the_override_with_backslashes() {
    assert_eq!(pick_root("C:\\Users\\me", true, Some("D:/scratch/co/")), "D:\\scratch\\co");
}

#[test]
fn never_lets_a_dev_build_use_the_real_company_in_any_spelling() {
    let sandbox = "C:\\Users\\me\\.bytehive\\company-dev";
    for root in [
        "C:\\Users\\me\\.bytehive\\company",
        "C:\\Users\\me\\.bytehive\\company\\",
        "c:/users/ME/.ByteHive/Company",
        "C:\\Users\\me\\.bytehive\\company\\people\\x",
        "C:/Users/me/.bytehive/company/jobs",
        "C:\\Users\\me\\.bytehive\\scratch\\..\\company",
        "C:\\Users\\me\\.bytehive\\.\\company",
    ] {
        assert_eq!(pick_root("C:\\Users\\me", true, Some(root)), sandbox, "{root}");
    }
}

#[test]
fn keeps_a_sibling_whose_name_only_starts_like_the_real_folder() {
    assert_eq!(pick_root("C:\\Users\\me", true, Some("C:\\Users\\me\\.bytehive\\company-x")), "C:\\Users\\me\\.bytehive\\company-x");
}

#[test]
fn falls_back_to_the_sandbox_for_a_path_that_is_not_absolute() {
    assert_eq!(pick_root("C:\\Users\\me", true, Some("scratch\\co")), "C:\\Users\\me\\.bytehive\\company-dev");
}

// store

#[test]
fn puts_the_company_under_the_home_folder() {
    assert_eq!(company_root("C:\\Users\\me\\"), "C:\\Users\\me\\.bytehive\\company");
}

#[test]
fn starts_an_empty_company_when_there_is_no_file() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load_company(dir.path()).unwrap().people[0].name, "Founder");
    assert_eq!(load_company(&dir.path().join("not-yet")).unwrap().next_job, 1);
}

#[test]
fn round_trips_the_company() {
    let dir = tempfile::tempdir().unwrap();
    let c = Company { next_job: 7, ..Company::empty() };
    save_company(dir.path(), &c).unwrap();
    assert_eq!(load_company(dir.path()).unwrap().next_job, 7);
}

#[test]
fn swaps_company_json_in_whole_so_a_reader_never_sees_half_a_write() {
    let dir = tempfile::tempdir().unwrap();
    save_company(dir.path(), &Company::empty()).unwrap();
    save_company(dir.path(), &Company { next_job: 7, ..Company::empty() }).unwrap();
    assert_eq!(names(dir.path()), ["company.json"]);
    let text = fs::read_to_string(dir.path().join("company.json")).unwrap();
    assert!(text.starts_with("{\n  \"version\": 1,\n  \"floors\": []"), "{text}");
}

#[test]
fn reads_a_file_saved_with_a_byte_order_mark_as_windows_editors_do() {
    assert_eq!(parse_company("\u{FEFF}{\"version\":1,\"people\":[],\"floors\":[]}").unwrap().version, 1);
}

#[test]
fn fills_in_the_limits_a_file_does_not_have_and_keeps_the_ones_it_does() {
    let c = parse_company(r#"{"version":1,"people":[],"floors":[],"limits":{"peopleTotal":3}}"#).unwrap();
    assert_eq!(c.limits, crate::company::types::Limits { people_total: 3, ..DEFAULT_LIMITS });
    assert_eq!(parse_company(r#"{"version":1,"people":[],"floors":[]}"#).unwrap().limits, DEFAULT_LIMITS);
}

#[test]
fn refuses_a_corrupt_file_instead_of_starting_over() {
    assert!(parse_company("{oops").is_err());
    assert!(parse_company(r#"{"version":2,"people":[],"floors":[]}"#).is_err());
    assert!(parse_company(r#"{"version":1,"floors":[]}"#).is_err());
}

#[test]
fn round_trips_a_job() {
    let dir = tempfile::tempdir().unwrap();
    save_job(dir.path(), &job()).unwrap();
    let (jobs, unreadable) = load_jobs(dir.path()).unwrap();
    assert_eq!(jobs[0].id, "J-0001");
    assert!(unreadable.is_empty());
}

#[test]
fn swaps_a_job_file_in_whole_so_a_write_cut_short_costs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    save_job(dir.path(), &job()).unwrap();
    save_job(dir.path(), &Job { title: "new".into(), ..job() }).unwrap();
    assert_eq!(names(&dir.path().join("jobs")), ["J-0001.json"]);
    assert_eq!(load_jobs(dir.path()).unwrap().0[0].title, "new");
}

#[test]
fn reads_a_job_saved_with_a_byte_order_mark_as_windows_editors_do() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("jobs")).unwrap();
    fs::write(dir.path().join("jobs").join("J-0001.json"), format!("\u{FEFF}{}", serde_json::to_string(&job()).unwrap())).unwrap();
    let (jobs, unreadable) = load_jobs(dir.path()).unwrap();
    assert_eq!(jobs[0].id, "J-0001");
    assert!(unreadable.is_empty());
}

fn leaves_a_bad_job_file_in_place(text: &str) {
    let dir = tempfile::tempdir().unwrap();
    save_job(dir.path(), &job()).unwrap();
    let bad = dir.path().join("jobs").join("J-0002.json");
    fs::write(&bad, text).unwrap();
    let (jobs, unreadable) = load_jobs(dir.path()).unwrap();
    assert_eq!(jobs.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["J-0001"]);
    assert_eq!(fs::read_to_string(&bad).unwrap(), text);
    assert_eq!(unreadable, ["J-0002.json"]);
    let (key, words) = job_file_alert(&unreadable[0]);
    assert_eq!(key, "job-file-J-0002.json");
    assert!(words.contains("J-0002.json"));
}

#[test]
fn raises_an_alert_for_a_job_file_that_is_half_written_and_leaves_it_in_place() {
    leaves_a_bad_job_file_in_place("{ \"id\": \"J-0002\", ");
}

#[test]
fn raises_an_alert_for_a_job_file_that_is_not_a_job_and_leaves_it_in_place() {
    leaves_a_bad_job_file_in_place("{}");
}

#[test]
fn raises_an_alert_for_a_job_file_that_is_not_an_object_and_leaves_it_in_place() {
    leaves_a_bad_job_file_in_place("null");
}

#[test]
fn does_not_start_an_empty_company_over_one_it_could_not_read() {
    let dir = tempfile::tempdir().unwrap();
    // A folder where the file should be: reading it is access denied, not "missing".
    fs::create_dir_all(dir.path().join("company.json")).unwrap();
    let e = load_company(dir.path()).unwrap_err();
    assert!(format!("{e:#}").contains("os error 5"), "{e:#}");
}

#[test]
fn skips_a_temp_file_a_crash_left_behind() {
    let dir = tempfile::tempdir().unwrap();
    save_job(dir.path(), &job()).unwrap();
    fs::write(dir.path().join("jobs").join("J-0002.json.tmp"), "{ half").unwrap();
    let (jobs, unreadable) = load_jobs(dir.path()).unwrap();
    assert_eq!(jobs.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["J-0001"]);
    assert!(unreadable.is_empty());
}

#[test]
fn lists_jobs_oldest_first() {
    let dir = tempfile::tempdir().unwrap();
    save_job(dir.path(), &Job { id: "J-0009".into(), created_at: 5, ..job() }).unwrap();
    save_job(dir.path(), &Job { id: "J-0010".into(), created_at: 2, ..job() }).unwrap();
    let ids: Vec<String> = load_jobs(dir.path()).unwrap().0.into_iter().map(|j| j.id).collect();
    assert_eq!(ids, ["J-0010", "J-0009"]);
}
