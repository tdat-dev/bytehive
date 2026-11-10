//! reduce.test.ts: what people hear when a Job stops and starts, a paused and a stopped
//! Job, pauseJob and resumeJob, closeJob and stopTurns, with the helpers of reduce_tests.rs.

use super::*;

fn to_whom(effects: &[Effect]) -> Vec<&str> {
    effects.iter().map(|e| told(e).0).collect()
}

fn merging() -> State {
    apply_request(&running(), &done_request(FOUNDER_ID, "J-0001", "ok")).state
}

// what people hear when a job stops and starts

#[test]
fn notes_tell_the_founder_why_a_job_stopped_so_they_dont_guess() {
    assert_eq!(
        paused_note("J-0001"),
        deliver(FOUNDER_ID, "J-0001 reached its token cap and is paused until the owner adds tokens. Its messages are held and it cannot be finished. Wait, you will be told.")
    );
}

#[test]
fn notes_do_not_wake_someone_stopped_just_to_say_it_runs_again() {
    let s = join_job(&running(), "J-0001", "d1");
    assert_eq!(to_whom(&resumed_notes(&s, "J-0001", |id| id == "d1")), [FOUNDER_ID, "d1"]);
}

#[test]
fn notes_tell_the_founder_and_everyone_on_it_that_it_runs_again_what_was_held_is_gone() {
    let s = join_job(&running(), "J-0001", "d1");
    let notes = resumed_notes(&s, "J-0001", |_| true);
    assert_eq!(to_whom(&notes), [FOUNDER_ID, "p9", "d1"]);
    assert_eq!(told(&notes[0]).1, "J-0001 is running again with more tokens. If its work is finished, call job_finish now.");
    assert_eq!(told(&notes[1]).1, "J-0001 is running again. Anything you sent while it was paused was not delivered: send it again if it still matters.");
    assert!(resumed_notes(&s, "J-9999", |_| true).is_empty());
}

// a paused job

#[test]
fn a_paused_job_holds_a_message_to_someone_on_it_and_tells_the_sender() {
    let s = pause_job(&join_job(&running(), "J-0001", "l1"), "J-0001");
    assert_eq!(
        apply_request(&s, &message(FOUNDER_ID, "Hai", "go")).effects,
        vec![deliver(FOUNDER_ID, "Held: your job J-0001 is paused, so your message to Hai was not sent. Wait for it to be resumed.")]
    );
}

// a stopped job

fn stopped() -> State {
    halt_job(&join_job(&running(), "J-0001", "l1"), "J-0001")
}

#[test]
fn a_stopped_job_comes_from_any_job_still_going_and_from_nothing_else() {
    assert_eq!(stopped().jobs[0].status, JobStatus::Stopped);
    assert_eq!(halt_job(&pause_job(&running(), "J-0001"), "J-0001").jobs[0].status, JobStatus::Stopped);
    assert_eq!(halt_job(&fresh(), "J-0001").jobs[0].status, JobStatus::Stopped);
    assert_eq!(halt_job(&waiting(), "J-0001").jobs[0].status, JobStatus::Stopped);
    for s in [merging(), close_job(&merging(), "J-0001"), closed(), stopped()] {
        assert_eq!(halt_job(&s, "J-0001"), s);
    }
    let r = running();
    assert_eq!(halt_job(&r, "J-9999"), r);
}

#[test]
fn a_stopped_job_takes_no_plan_once_a_job_is_stopped_while_planning() {
    let s = halt_job(&fresh(), "J-0001");
    let r = apply_request(&s, &plan_request(FOUNDER_ID, "J-0001", plan()));
    assert_eq!(r.state, s);
    assert!(told(&r.effects[0]).1.contains("stopped"));
}

#[test]
fn a_stopped_job_tells_the_founder_it_is_over() {
    assert_eq!(
        stopped_note("J-0001"),
        deliver(FOUNDER_ID, "J-0001 was stopped by the owner. It takes no more messages and cannot be finished. Leave it, and do not brief anyone on it.")
    );
}

#[test]
fn a_stopped_job_holds_messages_both_ways_and_tells_the_sender() {
    let s = stopped();
    assert_eq!(
        apply_request(&s, &message(FOUNDER_ID, "Hai", "go")).effects,
        vec![deliver(FOUNDER_ID, "Held: your job J-0001 is stopped, so your message to Hai was not sent. That job will not run again.")]
    );
    assert_eq!(
        apply_request(&s, &message("l1", "Founder", "done")).effects,
        vec![deliver("l1", "Held: your job J-0001 is stopped, so your message to Founder was not sent. That job will not run again.")]
    );
}

#[test]
fn a_stopped_job_does_not_cut_off_someone_who_is_on_a_running_job_too_a_message_names_no_job() {
    let mut s = new_job(&stopped(), "next", 5).0.state;
    s.jobs[1].status = JobStatus::Running;
    s.jobs[1].people = vec!["l1".into()];
    assert_eq!(apply_request(&s, &message(FOUNDER_ID, "Hai", "go")).effects, vec![deliver("l1", "[From Founder] go")]);
}

// and the next job on a floor with no Lead

fn unassigned(floor: &str) -> JobPlan {
    JobPlan { summary: "More".into(), cards: vec![PlanCard { title: "Do it".into(), ..card(floor, None) }], hires: vec![], ..plan() }
}

fn next(s: &State, p: JobPlan) -> State {
    let planned = apply_request(&new_job(s, "next", 5).0.state, &plan_request(FOUNDER_ID, "J-0002", p)).state;
    approve(&planned, "J-0002").state
}

#[test]
fn a_stopped_job_and_the_next_job_on_a_floor_with_no_lead_takes_on_whoever_works_there_alone() {
    let s = next(&halt_job(&running(), "J-0001"), unassigned("b"));
    assert_eq!(s.jobs[1].status, JobStatus::Running);
    assert_eq!(s.jobs[1].people, ["p9"]);
    assert_eq!(apply_request(&s, &message(FOUNDER_ID, "Vy", "go")).effects, vec![deliver("p9", "[From Founder] go")]);
    assert_eq!(apply_request(&s, &message("p9", "Founder", "done")).effects, vec![deliver(FOUNDER_ID, "[From Vy] done")]);
}

#[test]
fn a_stopped_job_and_the_next_job_leaves_a_floor_with_a_lead_to_the_lead() {
    assert_eq!(next(&running(), unassigned("a")).jobs[1].people, ["l1"]);
}

#[test]
fn a_stopped_job_cannot_be_finished_resumed_or_joined() {
    let s = stopped();
    let r = apply_request(&s, &done_request(FOUNDER_ID, "J-0001", "ok"));
    assert_eq!(r.state, s);
    assert!(told(&r.effects[0]).1.contains("stopped"));
    assert_eq!(resume_job(&s, "J-0001", 1000), s);
    assert_eq!(join_job(&s, "J-0001", "d1"), s);
}

// pauseJob and resumeJob

#[test]
fn pause_job_pauses_a_running_job() {
    assert_eq!(pause_job(&running(), "J-0001").jobs[0].status, JobStatus::Paused);
}

#[test]
fn pause_job_leaves_a_job_that_is_not_running_alone() {
    let s = waiting();
    assert_eq!(pause_job(&s, "J-0001"), s);
    let r = running();
    assert_eq!(pause_job(&r, "J-9999"), r);
}

#[test]
fn resume_job_resumes_a_paused_job_with_a_higher_cap() {
    let s = pause_job(&running(), "J-0001");
    let cap = s.jobs[0].token_cap;
    let job = &resume_job(&s, "J-0001", 1000).jobs[0];
    assert_eq!((&job.status, job.token_cap), (&JobStatus::Running, cap + 1000));
}

#[test]
fn resume_job_acts_only_on_a_paused_job() {
    let s = running();
    assert_eq!(resume_job(&s, "J-0001", 1000), s);
    assert_eq!(resume_job(&s, "J-9999", 1000), s);
}

// closeJob

#[test]
fn close_job_moves_a_job_waiting_for_merge_to_done() {
    assert_eq!(close_job(&merging(), "J-0001").jobs[0].status, JobStatus::Done);
}

#[test]
fn close_job_refuses_a_job_that_is_not_waiting_for_merge_or_does_not_exist() {
    let s = running();
    assert_eq!(close_job(&s, "J-0001"), s);
    assert_eq!(close_job(&s, "J-9999"), s);
    let done = close_job(&merging(), "J-0001");
    assert_eq!(close_job(&done, "J-0001"), done);
}

// stopTurns

fn job(id: &str, status: JobStatus, people: &[&str]) -> Job {
    let mut j = new_job(&State { company: Company::empty(), jobs: vec![], roles: vec![] }, id, 1).1;
    j.id = id.into();
    j.status = status;
    j.token_cap = 1;
    j.people = people.iter().map(|p| p.to_string()).collect();
    j
}

fn turns(jobs: Vec<Job>, id: &str) -> Vec<String> {
    let mut t = stop_turns(&State { company: Company::empty(), jobs, roles: vec![] }, id);
    t.sort();
    t
}

#[test]
fn stop_turns_is_everyone_on_the_job_and_the_founder_when_nothing_else_is_going() {
    assert_eq!(turns(vec![job("J1", JobStatus::Stopped, &["a", "b"])], "J1"), ["a", "b", "founder"]);
}

#[test]
fn stop_turns_leaves_out_someone_working_another_job_and_the_founder() {
    assert_eq!(turns(vec![job("J1", JobStatus::Stopped, &["a", "b"]), job("J2", JobStatus::Running, &["b"])], "J1"), ["a"]);
}

#[test]
fn stop_turns_counts_a_finished_job_as_not_going() {
    assert_eq!(turns(vec![job("J1", JobStatus::Stopped, &["a"]), job("J2", JobStatus::Done, &["a"])], "J1"), ["a", "founder"]);
}

#[test]
fn stop_turns_is_nobody_for_an_unknown_job() {
    assert!(turns(vec![], "J9").is_empty());
}
