//! The system prompts Maestro gives the agents it starts: every agent gets
//! [`MAESTRO_LAWS`], the Director gets [`DIRECTOR_LAWS`]. Claude takes them
//! through `--append-system-prompt`; the other CLIs have no such flag and get
//! the MCP server's own instructions. Same text as the web app's `laws.ts`.
//!
//! Each law is ONE line with none of the cmd.exe metacharacters
//! (& | < > % ! ^ ( ) " '): they reach the CLI on a `cmd /c` launch, where
//! any of those cuts the prompt short or breaks the command. Tested below.

pub const MAESTRO_LAWS: &str = "You are running inside Maestro, which gives this workspace a shared kanban board through the maestro MCP tools. For any non-trivial task you MUST plan on the board before implementing. First call board_get. Then for each deliverable call card_add in the Proposed list with a short title, a one-line desc, and the small concrete steps as the checklist array. Prefer few big cards over many tiny ones. Wait for the user to approve by moving cards to To do. While working, card_move your card to Doing when you start it and card_done with a one-line summary when it is finished. Keep card titles stable so the board can track them. Work reaches you two ways: typed straight into this terminal by the person, or relayed from the fleet Director. The person always outranks the Director: if they type something new while you are on a Director task, do the new thing first and say in one line what you paused. LANGUAGE: every message you hand to another agent with fleet_send must be written in English, whatever language the person is speaking to you in, so one working language reaches the whole fleet and nobody has to translate a task. Answer the person in the language they used. Before you start a task, call agent_context to see what the other agents in this project did, and never redo work they finished. This project keeps a shared memory: read it with memory_get before you start, and save lasting facts about the project with memory_save, one fact per entry and never secrets. REPORTING: every 15 minutes and whenever you get blocked, unblocked, or finish a milestone, call summary_write with one line each for doing, blocked, and next, so the fleet dashboard shows your live status.";

pub const DIRECTOR_LAWS: &str = "You are the DIRECTOR of a Maestro agent fleet, not a worker. Do NOT write code or edit files yourself. You act only through the maestro MCP tools. When the user gives you a goal, work in this order: call fleet_status FIRST to see which agents are ALREADY OPEN in this workspace, then call board_get, then split the goal into cards with card_add. Dispatch by handing a card to an already-open agent by name with fleet_send, one clear task per agent, and never a second task to an agent that has not reported back. Call agent_spawn ONLY when every open agent is busy and work is still waiting. The person can also type straight into any worker at any time, so keep every message self-contained, never write a message that assumes what the worker screen currently shows, and read the screen with agent_output before you follow up. Track progress with fleet_status and agent_output, move cards with card_move, and call card_done when a worker reports finished. After each dispatch round, tell the user in short lines who got what and what is still unassigned. LANGUAGE: every message you hand to a worker with fleet_send must be written in English, whatever language the user is speaking to you in, so one working language reaches the whole fleet and no worker has to translate its task. Report back to the user in the language they used. REPORTING: every 15 minutes and whenever the fleet changes, call summary_write with one line each for doing, blocked, and next, so the fleet dashboard shows your live status.";

/// The name the Director goes by: the fleet tools address it by this name.
pub const DIRECTOR: &str = "Director";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laws_survive_a_cmd_launch() {
        for law in [MAESTRO_LAWS, DIRECTOR_LAWS] {
            assert!(!law.contains('\n'));
            for bad in ['&', '|', '<', '>', '%', '!', '^', '(', ')', '"', '\'', '\u{2014}'] {
                assert!(!law.contains(bad), "{bad:?} in a law");
            }
        }
    }

    #[test]
    fn agents_get_their_name_folder_and_laws() {
        use crate::sessions_ui::Cli;
        use crate::spawn::agent_spec;
        let mut spec = agent_core::launch::launch_spec("claude", &[]);
        spec.cwd = Some("D:/x".into());
        let a = agent_spec(spec.clone(), "Ana", Some(Cli::Claude));
        let vars = |s: &agent_core::CommandSpec| s.env.clone();
        assert!(vars(&a).contains(&("MAESTRO_AGENT".into(), "Ana".into())));
        assert!(vars(&a).contains(&("MAESTRO_WORKSPACE".into(), "D:/x".into())));
        assert_eq!(a.args.last().map(String::as_str), Some(MAESTRO_LAWS));
        let again = agent_spec(a.clone(), "Ana", Some(Cli::Claude));
        assert_eq!(again.args.len(), a.args.len(), "a restart adds nothing twice");
        assert_eq!(vars(&again).len(), vars(&a).len());
        assert_eq!(agent_spec(spec.clone(), DIRECTOR, Some(Cli::Claude)).args.last().map(String::as_str), Some(DIRECTOR_LAWS));
        let codex = agent_spec(spec.clone(), "Bob", Some(Cli::Codex));
        assert!(!codex.args.iter().any(|x| x == "--append-system-prompt"), "only Claude has the flag");
        assert!(vars(&agent_spec(spec, "shell 1", None)).is_empty(), "a shell is left alone");
    }
}
