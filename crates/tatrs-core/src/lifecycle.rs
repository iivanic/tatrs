//! Derived lifecycle: the status a human needs to supervise work.
//!
//! `STATUS` stays binary (`OPEN`/`CLOSED`) because it is part of the
//! compatibility contract (`docs/FORMAT.md` §6). The six states below are
//! computed, never stored: three come from evidence (`OPEN`/`CLOSED`, live
//! leases, dependency edges) and three come from reserved tags that record a
//! human decision. Deriving instead of persisting means no state can go stale
//! and no transition dirties `TASK.md`.
//!
//! This module is pure: callers supply the clock, the last filesystem-visible
//! activity, and the lease. Nothing here reads or writes files.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Serialize;

use crate::claims::Claim;
use crate::huid::Huid;
use crate::ready;
use crate::task::Task;
use crate::time;

/// Declared state: work stopped by decision (usually resource starvation).
pub const PAUSED_TAG: &str = "paused";
/// Declared outcome: closed without being done, reason in the body.
pub const REFUSED_TAG: &str = "refused";
/// Declared follow-up: closed, but the owner still needs to be told.
pub const NOTIFY_TAG: &str = "notify";

/// Default window after which a silent live lease is reported as stalled.
pub const DEFAULT_STALE_AFTER: i64 = 2 * 3_600;

/// What a supervisor should read off a task right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Open, untouched: the ticket exists and nothing has started.
    Received,
    /// A live lease or the `wip` tag: someone is working on it.
    InProgress,
    /// The `paused` tag: stopped on purpose.
    Paused,
    /// An open dependency or the `blocked` tag: cannot proceed yet.
    Waiting,
    /// Closed and not refused.
    Completed,
    /// Closed with the `refused` tag, with a reason in the body.
    Refused,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Received => "received",
            State::InProgress => "in_progress",
            State::Paused => "paused",
            State::Waiting => "waiting",
            State::Completed => "completed",
            State::Refused => "refused",
        }
    }

    /// True while the task still expects work.
    pub fn is_open(self) -> bool {
        matches!(
            self,
            State::Received | State::InProgress | State::Paused | State::Waiting
        )
    }

    /// Board order: what needs a human first.
    pub fn rank(self) -> u8 {
        match self {
            State::Refused => 0,
            State::Paused => 1,
            State::Waiting => 2,
            State::InProgress => 3,
            State::Received => 4,
            State::Completed => 5,
        }
    }
}

/// Things a human should look at, independent of the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Flag {
    /// Closed while `## Acceptance` still has unchecked criteria.
    Unverified,
    /// Closed and tagged `notify`: tell the owner.
    NotifyOwner,
    /// Live lease with no task change for longer than the stale window.
    Stalled,
    /// A lease expired without being released.
    ExpiredClaim,
    /// Tagged `needs-human`: a decision is needed, not just time.
    NeedsHuman,
    /// Declared `paused` while a live lease is held.
    PausedButClaimed,
    /// A lease exists for a closed task.
    ClaimOnClosedTask,
}

impl Flag {
    pub fn as_str(self) -> &'static str {
        match self {
            Flag::Unverified => "unverified",
            Flag::NotifyOwner => "notify-owner",
            Flag::Stalled => "stalled",
            Flag::ExpiredClaim => "expired-claim",
            Flag::NeedsHuman => "needs-human",
            Flag::PausedButClaimed => "paused-but-claimed",
            Flag::ClaimOnClosedTask => "claim-on-closed-task",
        }
    }
}

/// Clock, activity, and staleness policy for one classification pass.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub now: i64,
    /// Last observed change to the task file, in epoch seconds.
    pub last_activity: Option<i64>,
    pub stale_after: i64,
}

impl Context {
    pub fn new(now: i64) -> Self {
        Self {
            now,
            last_activity: None,
            stale_after: DEFAULT_STALE_AFTER,
        }
    }

    pub fn with_activity(mut self, last_activity: Option<i64>) -> Self {
        self.last_activity = last_activity;
        self
    }

    pub fn with_stale_after(mut self, stale_after: i64) -> Self {
        self.stale_after = stale_after;
        self
    }
}

/// The derived status plus the evidence it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lifecycle {
    /// The one state a UI should lead with. Precedence guarantees exactly one.
    pub state: State,
    /// States that also hold but lost the precedence contest, in board order.
    /// A task has exactly one `state`; this keeps the other conditions visible
    /// (for example a paused task whose dependency is still open).
    pub also: Vec<State>,
    /// True when the state rests on a tag rather than on observed evidence, so
    /// a UI can show "declared" differently from "observed".
    pub declared: bool,
    /// Lease owner for `InProgress`; `OWNER` for everything else.
    pub owner: Option<String>,
    pub run: Option<String>,
    /// Epoch seconds when this state started, best effort.
    pub since: Option<i64>,
    pub flags: Vec<Flag>,
    /// Whether `ready`/`claim` would dispatch this task right now.
    pub ready: bool,
}

impl Lifecycle {
    /// True when any flag asks for a human decision.
    pub fn attention(&self) -> bool {
        !self.flags.is_empty()
    }

    pub fn since_iso(&self) -> Option<String> {
        self.since.map(time::format_iso)
    }
}

/// Classify one task. Precedence, documented in `docs/DASHBOARD.md`:
/// `CLOSED` first, then a live lease, then the `paused` tag, then blocking, then
/// `wip`, then "received". Contradictions become flags instead of being hidden.
pub fn classify(task: &Task, all: &[Task], claim: Option<&Claim>, ctx: Context) -> Lifecycle {
    let tags: BTreeSet<&str> = task.tags.iter().map(String::as_str).collect();
    let live = claim.filter(|claim| claim.is_live(ctx.now));

    let owner = live.map(|claim| claim.owner.clone()).or_else(|| {
        task.props
            .get("OWNER")
            .map(str::trim)
            .filter(|owner| !owner.is_empty())
            .map(str::to_owned)
    });
    let run = live.and_then(|claim| claim.run.clone());

    let acceptance = task.acceptance();
    let mut flags = Vec::new();

    if task.is_closed() {
        let refused = tags.contains(REFUSED_TAG);
        if tags.contains(NOTIFY_TAG) {
            flags.push(Flag::NotifyOwner);
        }
        if acceptance.unverified() {
            flags.push(Flag::Unverified);
        }
        if claim.is_some() {
            flags.push(Flag::ClaimOnClosedTask);
        }
        return Lifecycle {
            state: if refused {
                State::Refused
            } else {
                State::Completed
            },
            also: Vec::new(),
            declared: refused,
            owner,
            run,
            since: ctx.last_activity,
            flags,
            ready: false,
        };
    }

    if tags.contains("needs-human") {
        flags.push(Flag::NeedsHuman);
    }

    if live.is_some() {
        if tags.contains(PAUSED_TAG) {
            flags.push(Flag::PausedButClaimed);
        }
        let quiet_since = ctx
            .last_activity
            .or_else(|| live.and_then(|claim| time::parse_iso(&claim.acquired_at)));
        if quiet_since.is_some_and(|last| ctx.now - last > ctx.stale_after) {
            flags.push(Flag::Stalled);
        }
    } else if claim.is_some() {
        flags.push(Flag::ExpiredClaim);
    }

    // Every condition that holds, before precedence collapses them to one
    // primary state. `received` is the fallback and is never listed here.
    let waiting = wait_reason(task, all, &tags);
    let mut conditions = Vec::new();
    if live.is_some() || tags.contains("wip") {
        conditions.push(State::InProgress);
    }
    if tags.contains(PAUSED_TAG) {
        conditions.push(State::Paused);
    }
    if waiting.is_some() {
        conditions.push(State::Waiting);
    }

    let (state, declared) = if live.is_some() {
        (State::InProgress, false)
    } else if tags.contains(PAUSED_TAG) {
        (State::Paused, true)
    } else if let Some(declared) = waiting {
        (State::Waiting, declared)
    } else if tags.contains("wip") {
        (State::InProgress, true)
    } else {
        (State::Received, false)
    };

    let also: Vec<State> = conditions
        .into_iter()
        .filter(|condition| *condition != state)
        .collect();

    let since = live
        .and_then(|claim| time::parse_iso(&claim.acquired_at))
        .or(ctx.last_activity);

    Lifecycle {
        state,
        also,
        declared,
        owner,
        run,
        since,
        flags,
        ready: ready::evaluate(task, all, claim, ctx.now).ready,
    }
}

/// `Some(declared)` when the task cannot proceed: `false` when an open
/// dependency blocks it, `true` when only the `blocked` tag says so.
fn wait_reason(task: &Task, all: &[Task], tags: &BTreeSet<&str>) -> Option<bool> {
    let blocked_by_dependency = task.depends().iter().any(|raw| match Huid::parse(raw) {
        Ok(id) => !all
            .iter()
            .any(|candidate| candidate.id == id && candidate.is_closed()),
        Err(_) => true,
    });

    if blocked_by_dependency {
        Some(false)
    } else if tags.contains("blocked") {
        Some(true)
    } else {
        None
    }
}

/// Classify every task. `claims` is keyed by HUID string; `activity` holds the
/// last observed change per HUID. The result is aligned with `tasks`.
pub fn classify_all(
    tasks: &[Task],
    claims: &BTreeMap<String, Claim>,
    activity: &BTreeMap<String, i64>,
    ctx: Context,
) -> Vec<Lifecycle> {
    tasks
        .iter()
        .map(|task| {
            let per_task = Context {
                last_activity: activity.get(task.id.as_str()).copied(),
                ..ctx
            };
            classify(task, tasks, claims.get(task.id.as_str()), per_task)
        })
        .collect()
}

/// Counts for the board header and for `summary --json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub total: usize,
    pub received: usize,
    pub in_progress: usize,
    pub paused: usize,
    pub waiting: usize,
    pub completed: usize,
    pub refused: usize,
    /// Open tasks that pass every dispatch rule.
    pub ready: usize,
    /// Tasks with at least one flag.
    pub attention: usize,
}

impl Counts {
    pub fn add(&mut self, lifecycle: &Lifecycle) {
        self.total += 1;
        match lifecycle.state {
            State::Received => self.received += 1,
            State::InProgress => self.in_progress += 1,
            State::Paused => self.paused += 1,
            State::Waiting => self.waiting += 1,
            State::Completed => self.completed += 1,
            State::Refused => self.refused += 1,
        }
        if lifecycle.ready {
            self.ready += 1;
        }
        if lifecycle.attention() {
            self.attention += 1;
        }
    }

    /// Everything not yet closed.
    pub fn open(&self) -> usize {
        self.received + self.in_progress + self.paused + self.waiting
    }
}

/// Count a classification pass.
pub fn counts(lifecycles: &[Lifecycle]) -> Counts {
    let mut counts = Counts::default();
    for lifecycle in lifecycles {
        counts.add(lifecycle);
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huid::Huid;
    use crate::task::Task;

    fn task(id: &str, tags: &[&str], status: &str) -> Task {
        let mut task = Task::new(
            Huid::parse(id).unwrap(),
            "T",
            tags.iter().map(|tag| (*tag).to_owned()).collect(),
            10,
        );
        task.status = status.to_owned();
        task
    }

    fn with_body(mut task: Task, body: &str) -> Task {
        task.body = format!("\n{body}\n");
        task
    }

    fn claim(id: &str, owner: &str, acquired: i64, expires: i64) -> Claim {
        Claim {
            task: id.to_owned(),
            owner: owner.to_owned(),
            token: "t".to_owned(),
            acquired_at: time::format_iso(acquired),
            expires_at: time::format_iso(expires),
            host: "h".to_owned(),
            pid: 1,
            run: Some("run-1".to_owned()),
        }
    }

    fn classify_one(task: &Task, claim: Option<&Claim>, ctx: Context) -> Lifecycle {
        let all = vec![task.clone()];
        classify(task, &all, claim, ctx)
    }

    #[test]
    fn untouched_open_task_is_received() {
        let task = task("20261109-120000", &[], "OPEN");
        let lifecycle = classify_one(&task, None, Context::new(0));
        assert_eq!(lifecycle.state, State::Received);
        assert!(!lifecycle.declared);
        assert!(lifecycle.flags.is_empty());
    }

    #[test]
    fn live_claim_is_in_progress_and_carries_the_owner() {
        let task = task("20261109-120000", &["agent"], "OPEN");
        let claim = claim("20261109-120000", "agent-3", 1_000, 4_000);
        let lifecycle = classify_one(
            &task,
            Some(&claim),
            Context::new(2_000).with_activity(Some(1_900)),
        );
        assert_eq!(lifecycle.state, State::InProgress);
        assert!(!lifecycle.declared);
        assert_eq!(lifecycle.owner.as_deref(), Some("agent-3"));
        assert_eq!(lifecycle.run.as_deref(), Some("run-1"));
        assert_eq!(lifecycle.since, Some(1_000));
        assert!(lifecycle.flags.is_empty());
    }

    #[test]
    fn expired_claim_falls_back_and_is_flagged() {
        let task = task("20261109-120000", &["agent"], "OPEN");
        let claim = claim("20261109-120000", "agent-3", 1_000, 2_000);
        let lifecycle = classify_one(&task, Some(&claim), Context::new(3_000));
        assert_eq!(lifecycle.state, State::Received);
        assert_eq!(lifecycle.flags, vec![Flag::ExpiredClaim]);
    }

    #[test]
    fn declared_states_are_marked_as_declared() {
        let paused = task("20261109-120000", &["paused"], "OPEN");
        let lifecycle = classify_one(&paused, None, Context::new(0));
        assert_eq!(lifecycle.state, State::Paused);
        assert!(lifecycle.declared);

        let blocked = task("20261109-120000", &["blocked"], "OPEN");
        let lifecycle = classify_one(&blocked, None, Context::new(0));
        assert_eq!(lifecycle.state, State::Waiting);
        assert!(lifecycle.declared);

        let wip = task("20261109-120000", &["wip"], "OPEN");
        let lifecycle = classify_one(&wip, None, Context::new(0));
        assert_eq!(lifecycle.state, State::InProgress);
        assert!(lifecycle.declared);
    }

    #[test]
    fn open_dependency_is_observed_waiting() {
        let mut dependent = task("20261109-120000", &["agent"], "OPEN");
        dependent.props.put("DEPENDS", "20261109-120001");
        let mut dependency = task("20261109-120001", &["agent"], "OPEN");

        let lifecycle = classify(
            &dependent,
            &[dependent.clone(), dependency.clone()],
            None,
            Context::new(0),
        );
        assert_eq!(lifecycle.state, State::Waiting);
        assert!(!lifecycle.declared);

        dependency.close();
        let lifecycle = classify(
            &dependent,
            &[dependent.clone(), dependency],
            None,
            Context::new(0),
        );
        assert_eq!(lifecycle.state, State::Received);
    }

    #[test]
    fn closed_tasks_are_completed_or_refused() {
        let done = task("20261109-120000", &[], "CLOSED");
        assert_eq!(
            classify_one(&done, None, Context::new(0)).state,
            State::Completed
        );

        let refused = task("20261109-120000", &["refused"], "CLOSED");
        let lifecycle = classify_one(&refused, None, Context::new(0));
        assert_eq!(lifecycle.state, State::Refused);
        assert!(lifecycle.declared);
    }

    #[test]
    fn unverified_completion_is_flagged() {
        let mut closed = task("20261109-120000", &["agent"], "CLOSED");
        closed = with_body(closed, "## Acceptance\n\n- [x] one\n- [ ] two\n");
        let lifecycle = classify_one(&closed, None, Context::new(0));
        assert!(lifecycle.flags.contains(&Flag::Unverified));

        let mut verified = closed.clone();
        verified.body = "\n## Acceptance\n\n- [x] one\n- [x] two\n".to_owned();
        let lifecycle = classify_one(&verified, None, Context::new(0));
        assert!(!lifecycle.flags.contains(&Flag::Unverified));
    }

    #[test]
    fn contradictions_are_flagged_not_hidden() {
        let paused = task("20261109-120000", &["paused"], "OPEN");
        let claim = claim("20261109-120000", "agent-3", 1_000, 4_000);
        let lifecycle = classify_one(&paused, Some(&claim), Context::new(2_000));
        assert_eq!(lifecycle.state, State::InProgress);
        assert!(lifecycle.flags.contains(&Flag::PausedButClaimed));

        let closed = task("20261109-120000", &[], "CLOSED");
        let lifecycle = classify_one(&closed, Some(&claim), Context::new(2_000));
        assert_eq!(lifecycle.state, State::Completed);
        assert!(lifecycle.flags.contains(&Flag::ClaimOnClosedTask));
    }

    #[test]
    fn exactly_one_primary_state_keeps_the_others_visible() {
        // Paused, but a dependency is still open: one primary, both facts kept.
        let mut paused_waiting = task("20261109-120000", &["paused"], "OPEN");
        paused_waiting.props.put("DEPENDS", "20261109-120001");
        let dependency = task("20261109-120001", &[], "OPEN");
        let lifecycle = classify(
            &paused_waiting,
            &[paused_waiting.clone(), dependency],
            None,
            Context::new(0),
        );
        assert_eq!(lifecycle.state, State::Paused);
        assert_eq!(lifecycle.also, vec![State::Waiting]);

        // A live lease wins; the declared pause is still reported.
        let paused = task("20261109-120000", &["paused"], "OPEN");
        let claim = claim("20261109-120000", "agent-3", 1_000, 4_000);
        let lifecycle = classify_one(&paused, Some(&claim), Context::new(2_000));
        assert_eq!(lifecycle.state, State::InProgress);
        assert_eq!(lifecycle.also, vec![State::Paused]);

        // A resting task has nothing to add.
        let plain = task("20261109-120000", &[], "OPEN");
        assert!(classify_one(&plain, None, Context::new(0)).also.is_empty());
    }

    #[test]
    fn silent_lease_is_stalled() {
        let task = task("20261109-120000", &["agent"], "OPEN");
        let claim = claim("20261109-120000", "agent-3", 1_000, 99_000);
        let lifecycle = classify_one(
            &task,
            Some(&claim),
            Context::new(10_000).with_activity(Some(1_000)),
        );
        assert!(lifecycle.flags.contains(&Flag::Stalled));

        let fresh = classify_one(
            &task,
            Some(&claim),
            Context::new(10_000).with_activity(Some(9_000)),
        );
        assert!(!fresh.flags.contains(&Flag::Stalled));
    }

    #[test]
    fn counts_add_up_and_ready_is_a_subset_of_open() {
        let mut ready = task("20261109-120001", &["agent"], "OPEN");
        ready = with_body(ready, "## Acceptance\n\n- [ ] x\n");
        let tasks = vec![
            task("20261109-120000", &[], "OPEN"),
            ready,
            task("20261109-120002", &["refused"], "CLOSED"),
        ];
        let lifecycles = classify_all(&tasks, &BTreeMap::new(), &BTreeMap::new(), Context::new(0));
        let counts = counts(&lifecycles);
        assert_eq!(counts.total, 3);
        assert_eq!(counts.open(), 2);
        assert_eq!(counts.ready, 1);
        assert_eq!(counts.refused, 1);
        assert!(counts.ready <= counts.open());
    }
}
