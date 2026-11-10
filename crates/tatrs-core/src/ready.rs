//! Dispatch readiness: the single place that decides whether an agent may start
//! a task. `ready` lists these tasks; `claim` refuses anything that is not one.

use crate::claims::Claim;
use crate::huid::Huid;
use crate::task::Task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    pub ready: bool,
    pub blockers: Vec<String>,
}

/// Evaluate a task against the dispatch rules from `docs/DECISIONS.md` D-001/D-002.
pub fn evaluate(task: &Task, all: &[Task], claim: Option<&Claim>, now: i64) -> Readiness {
    let mut blockers = Vec::new();

    if task.is_closed() {
        blockers.push("task is closed".to_owned());
        return Readiness {
            ready: false,
            blockers,
        };
    }

    if !task.is_agent_task() {
        blockers.push("missing `agent` tag".to_owned());
    }

    let acceptance = task.acceptance();
    if !acceptance.satisfied() {
        blockers.push(if acceptance.present {
            "`## Acceptance` has no criterion with text".to_owned()
        } else {
            "no `## Acceptance` section".to_owned()
        });
    }

    for tag in ["wip", "blocked", "paused"] {
        if task.tags.iter().any(|candidate| candidate == tag) {
            blockers.push(format!("tagged `{tag}`"));
        }
    }

    for raw in task.depends() {
        match Huid::parse(&raw) {
            Ok(dependency) => match all.iter().find(|candidate| candidate.id == dependency) {
                Some(dependency) if dependency.is_closed() => {}
                Some(_) => blockers.push(format!("dependency {dependency} is open")),
                None => blockers.push(format!("missing dependency {dependency}")),
            },
            Err(_) => blockers.push(format!("invalid `DEPENDS` entry `{raw}`")),
        }
    }

    if let Some(claim) = claim.filter(|claim| claim.is_live(now)) {
        blockers.push(format!(
            "claimed by {} until {}",
            claim.owner, claim.expires_at
        ));
    }

    Readiness {
        ready: blockers.is_empty(),
        blockers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Task;

    fn sample(tags: &[&str]) -> Task {
        Task::new(
            Huid::parse("20261109-120000").unwrap(),
            "T",
            tags.iter().map(|tag| (*tag).to_owned()).collect(),
            10,
        )
    }

    fn with_body(mut task: Task, body: &str) -> Task {
        task.body = format!("\n{body}\n");
        task
    }

    #[test]
    fn human_tasks_are_not_ready() {
        let task = with_body(sample(&["bug"]), "## Acceptance\n\n- [ ] fixed\n");
        let readiness = evaluate(&task, std::slice::from_ref(&task), None, 0);
        assert!(!readiness.ready);
        assert!(readiness.blockers.iter().any(|b| b.contains("agent")));
    }

    #[test]
    fn agent_tasks_need_criteria() {
        let task = with_body(sample(&["agent"]), "## Acceptance\n\n- [ ] fixed\n");
        assert!(evaluate(&task, std::slice::from_ref(&task), None, 0).ready);

        let empty = with_body(sample(&["agent"]), "## Acceptance\n\n- [ ]\n");
        assert!(!evaluate(&empty, std::slice::from_ref(&empty), None, 0).ready);
    }

    #[test]
    fn wip_and_blocked_tags_stop_dispatch() {
        let task = with_body(sample(&["agent", "wip"]), "## Acceptance\n\n- [ ] fixed\n");
        assert!(!evaluate(&task, std::slice::from_ref(&task), None, 0).ready);

        let paused = with_body(
            sample(&["agent", "paused"]),
            "## Acceptance\n\n- [ ] fixed\n",
        );
        assert!(!evaluate(&paused, std::slice::from_ref(&paused), None, 0).ready);
    }

    #[test]
    fn dependencies_must_be_closed_and_present() {
        let mut dependent = with_body(sample(&["agent"]), "## Acceptance\n\n- [ ] fixed\n");
        dependent
            .props
            .put("DEPENDS", "20261109-120001, 20261109-120002");

        let open_dep = with_body(sample(&["agent"]), "");
        let mut open_dep = open_dep;
        open_dep.id = Huid::parse("20261109-120001").unwrap();

        let readiness = evaluate(&dependent, &[dependent.clone(), open_dep], None, 0);
        assert!(!readiness.ready);
        assert!(readiness.blockers.iter().any(|b| b.contains("is open")));
        assert!(readiness.blockers.iter().any(|b| b.contains("missing")));

        let mut closed_dep = sample(&["agent"]);
        closed_dep.id = Huid::parse("20261109-120001").unwrap();
        closed_dep.close();
        let mut closed_dep2 = sample(&["agent"]);
        closed_dep2.id = Huid::parse("20261109-120002").unwrap();
        closed_dep2.close();
        let readiness = evaluate(
            &dependent,
            &[dependent.clone(), closed_dep, closed_dep2],
            None,
            0,
        );
        assert!(readiness.ready, "{:?}", readiness.blockers);
    }

    #[test]
    fn live_claims_block_dispatch() {
        let task = with_body(sample(&["agent"]), "## Acceptance\n\n- [ ] fixed\n");
        let claim = Claim {
            task: task.id.to_string(),
            owner: "agent-3".to_owned(),
            token: "t".to_owned(),
            acquired_at: crate::time::format_iso(0),
            expires_at: crate::time::format_iso(3_600),
            host: "h".to_owned(),
            pid: 1,
            run: None,
        };
        let readiness = evaluate(&task, std::slice::from_ref(&task), Some(&claim), 0);
        assert!(!readiness.ready);
        assert!(readiness.blockers.iter().any(|b| b.contains("claimed")));

        // Expired claims do not block.
        let readiness = evaluate(&task, std::slice::from_ref(&task), Some(&claim), 4_000);
        assert!(readiness.ready);
    }
}
