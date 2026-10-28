//! Pull request status from GitHub, through the gh CLI, which already holds
//! your login: abc stores no token. Running gh is separate from reading what
//! it prints, so the reading is tested on its own.

use abc_core::model::{Checks, PrState, PullRequest, Review};
use serde::Deserialize;

/// The fields `gh pr list` is asked for.
pub const FIELDS: &str = "number,title,state,isDraft,headRefName,statusCheckRollup,reviewDecision,url,updatedAt,headRepositoryOwner";

/// A pull request, and the branch and owner it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchPr {
    pub branch: String,
    /// Who owns the repository the branch is in: the base's owner, or a fork's.
    pub owner: Option<String>,
    /// When it last changed, as GitHub writes it (ISO 8601, so it sorts).
    pub updated: String,
    pub pr: PullRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Listed {
    number: u32,
    title: String,
    url: String,
    state: String,
    #[serde(default)]
    is_draft: bool,
    head_ref_name: String,
    #[serde(default)]
    status_check_rollup: Vec<Check>,
    #[serde(default)]
    review_decision: Option<String>,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    head_repository_owner: Option<Owner>,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
}

/// A check run (`status`, `conclusion`) or a commit status (`state`).
#[derive(Deserialize)]
struct Check {
    status: Option<String>,
    conclusion: Option<String>,
    state: Option<String>,
}

/// Reads `gh pr list --json` output.
pub fn parse_prs(json: &[u8]) -> Result<Vec<BranchPr>, String> {
    let listed: Vec<Listed> = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    Ok(listed
        .into_iter()
        .map(|l| {
            let state = match (l.state.as_str(), l.is_draft) {
                ("MERGED", _) => PrState::Merged,
                ("CLOSED", _) => PrState::Closed,
                (_, true) => PrState::Draft,
                _ => PrState::Open,
            };
            let review = match l.review_decision.as_deref() {
                Some("APPROVED") => Some(Review::Approved),
                Some("CHANGES_REQUESTED") => Some(Review::ChangesRequested),
                Some("REVIEW_REQUIRED") => Some(Review::Required),
                _ => None,
            };
            BranchPr {
                branch: l.head_ref_name,
                owner: l.head_repository_owner.map(|o| o.login),
                updated: l.updated_at,
                pr: PullRequest {
                    number: l.number,
                    title: l.title,
                    url: l.url,
                    state,
                    checks: rollup(&l.status_check_rollup),
                    review,
                },
            }
        })
        .collect())
}

/// One verdict for all of a pull request's checks: failing if any failed,
/// else pending if any haven't finished, else passing; none without checks.
fn rollup(checks: &[Check]) -> Option<Checks> {
    if checks.is_empty() {
        return None;
    }
    let failed = |c: &Check| {
        let verdict = c.conclusion.as_deref().or(c.state.as_deref()).unwrap_or("");
        matches!(
            verdict,
            "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        )
    };
    let pending = |c: &Check| match (&c.status, &c.state) {
        (Some(status), _) => status != "COMPLETED",
        (None, Some(state)) => matches!(state.as_str(), "PENDING" | "EXPECTED"),
        (None, None) => false,
    };
    if checks.iter().any(failed) {
        Some(Checks::Failing)
    } else if checks.iter().any(pending) {
        Some(Checks::Pending)
    } else {
        Some(Checks::Passing)
    }
}

/// The pull request for `branch`: the most recently updated, preferring
/// ones from `own`er's repository over forks that use the same branch name.
pub fn pick<'a>(prs: &'a [BranchPr], branch: &str, own: Option<&str>) -> Option<&'a PullRequest> {
    prs.iter()
        .filter(|p| p.branch == branch)
        .max_by_key(|p| {
            (
                own.is_some() && p.owner.as_deref() == own,
                p.updated.as_str(),
            )
        })
        .map(|p| &p.pr)
}

/// The login of the account gh is signed in with on github.com, from
/// `gh auth status --json hosts`.
pub fn parse_login(json: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Status {
        hosts: std::collections::HashMap<String, Vec<Account>>,
    }
    #[derive(Deserialize)]
    struct Account {
        state: String,
        #[serde(default)]
        active: bool,
        login: String,
    }
    let status: Status = serde_json::from_slice(json).ok()?;
    status
        .hosts
        .get("github.com")?
        .iter()
        .find(|a| a.active && a.state == "success")
        .map(|a| a.login.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTED: &str = r#"[
      {"number": 142, "title": "GST mismatch check", "url": "https://github.com/privue/p/pull/142",
       "state": "OPEN", "isDraft": false, "headRefName": "feat/gst-check",
       "reviewDecision": "APPROVED", "updatedAt": "2026-10-28T09:00:00Z",
       "headRepositoryOwner": {"id": "x", "login": "privue"},
       "statusCheckRollup": [
         {"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"},
         {"__typename": "StatusContext", "state": "SUCCESS"}]},
      {"number": 139, "title": "Round GST", "url": "u", "state": "OPEN", "isDraft": true,
       "headRefName": "fix/round", "reviewDecision": "", "updatedAt": "2026-10-27T09:00:00Z",
       "headRepositoryOwner": {"login": "privue"},
       "statusCheckRollup": [
         {"__typename": "CheckRun", "status": "IN_PROGRESS", "conclusion": ""},
         {"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "FAILURE"}]},
      {"number": 150, "title": "Fork's own", "url": "u", "state": "OPEN", "isDraft": false,
       "headRefName": "feat/gst-check", "updatedAt": "2026-10-28T10:00:00Z",
       "headRepositoryOwner": {"login": "someone"}, "statusCheckRollup": []},
      {"number": 141, "title": "Ledger", "url": "u", "state": "MERGED", "isDraft": false,
       "headRefName": "fix/ledger", "updatedAt": "2026-10-20T09:00:00Z",
       "statusCheckRollup": [{"__typename": "StatusContext", "state": "PENDING"}]}
    ]"#;

    #[test]
    fn pull_requests_are_read_with_their_checks_and_review() {
        let prs = parse_prs(LISTED.as_bytes()).unwrap();
        assert_eq!(prs.len(), 4);
        let check = &prs[0].pr;
        assert_eq!(
            (check.state, check.checks, check.review),
            (PrState::Open, Some(Checks::Passing), Some(Review::Approved))
        );
        let round = &prs[1].pr;
        assert_eq!(
            (round.state, round.checks, round.review),
            (PrState::Draft, Some(Checks::Failing), None),
            "a failure outweighs one still running"
        );
        assert_eq!(prs[2].pr.checks, None, "no checks");
        assert_eq!(
            (prs[3].pr.state, prs[3].pr.checks),
            (PrState::Merged, Some(Checks::Pending))
        );
        assert!(parse_prs(b"not json").is_err());
    }

    #[test]
    fn a_branch_takes_its_own_repositorys_pull_request_over_a_forks() {
        let prs = parse_prs(LISTED.as_bytes()).unwrap();
        assert_eq!(
            pick(&prs, "feat/gst-check", Some("privue")).unwrap().number,
            142
        );
        assert_eq!(
            pick(&prs, "feat/gst-check", None).unwrap().number,
            150,
            "without an owner, the most recently updated"
        );
        assert!(pick(&prs, "main", Some("privue")).is_none());
    }

    #[test]
    fn the_signed_in_login_is_read() {
        let status = r#"{"hosts": {"github.com": [
            {"state": "success", "active": false, "host": "github.com", "login": "old"},
            {"state": "success", "active": true, "host": "github.com", "login": "snehilvj"}]}}"#;
        assert_eq!(parse_login(status.as_bytes()).as_deref(), Some("snehilvj"));
        let failed =
            r#"{"hosts": {"github.com": [{"state": "error", "active": true, "login": "x"}]}}"#;
        assert_eq!(parse_login(failed.as_bytes()), None);
        assert_eq!(parse_login(br#"{"hosts": {}}"#), None);
    }
}
