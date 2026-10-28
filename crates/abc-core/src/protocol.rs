//! The protocol between abc and abcd: newline-delimited JSON over a Unix
//! socket, one message per line.
//!
//! The first line each way is a [`Hello`]. Its shape never changes, so any two
//! builds can tell that they differ. Then the client sends [`Request`]s, each
//! answered by a [`Response`] with the same id, and abcd pushes [`Event`]s:
//! changes to connections that subscribed, and a search's results to the
//! connection that asked, ahead of its response.

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::model::{FoundFile, Repo};

/// Bumped on any change to the messages after `hello`.
pub const PROTOCOL: u32 = 1;

/// The first line each way: `{"hello": {...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloLine {
    pub hello: Hello,
}

/// Which protocol and build each side is. abcd adds its process and profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub build: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// A request from a client: `{"id": 7, "method": "repo.add", "params": {...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub call: Call,
}

/// What a request asks for, and the result it's answered with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Call {
    /// Answered with [`ServerStatus`].
    #[serde(rename = "server.status")]
    ServerStatus,
    /// Answered with `{}`, then abcd exits.
    #[serde(rename = "server.shutdown")]
    ServerShutdown,
    /// Answered with `{}`; then this connection gets change events.
    #[serde(rename = "subscribe")]
    Subscribe,
    /// Answered with [`Repos`].
    #[serde(rename = "repo.list")]
    RepoList,
    /// What `path`, a folder or git URL, would be as a repository named
    /// `name`. Answered with a [`RepoCheck`](crate::model::RepoCheck).
    #[serde(rename = "repo.check")]
    RepoCheck { path: String, name: String },
    /// Adds the repository at `path`, cloning a git URL first, under `name`
    /// or, without one, the folder's or repository's. Answered with [`Added`].
    #[serde(rename = "repo.add")]
    RepoAdd { path: String, name: Option<String> },
    /// Forgets a repository; its files stay. Answered with [`Repos`].
    #[serde(rename = "repo.remove")]
    RepoRemove { name: String },
    /// The worktree's files, as the explorer lists them. Answered with [`Files`].
    #[serde(rename = "worktree.files")]
    WorktreeFiles { worktree: PathBuf },
    /// A client opened the worktree, for "2h ago" and `tab`. Answered with `{}`.
    #[serde(rename = "worktree.visit")]
    WorktreeVisit { worktree: PathBuf },
    /// A file in a worktree. Answered with [`FileContents`](crate::model::FileContents).
    #[serde(rename = "file.read")]
    FileRead { worktree: PathBuf, path: String },
    /// Finds `query` in the worktree's files. Results come as
    /// `search.found` events; the response, [`SearchDone`], comes last. A new
    /// search from the same connection cancels the last.
    #[serde(rename = "search")]
    Search {
        worktree: PathBuf,
        query: String,
        max_lines: usize,
    },
}

/// The answer to a request: `{"id": 7, "result": ...}` or `{"id": 7, "error": {...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Result(serde_json::Value),
    Error(Failure),
}

impl Response {
    pub fn ok(id: u64, result: &impl Serialize) -> Self {
        let result = serde_json::to_value(result).expect("results serialize");
        Self {
            id,
            outcome: Outcome::Result(result),
        }
    }

    pub fn error(id: u64, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            id,
            outcome: Outcome::Error(Failure {
                code,
                message: message.into(),
            }),
        }
    }

    /// The result as `T`, or why there isn't one.
    pub fn into_result<T: DeserializeOwned>(self) -> Result<T, Failure> {
        match self.outcome {
            Outcome::Result(value) => serde_json::from_value(value).map_err(|e| Failure {
                code: ErrorCode::BadResponse,
                message: e.to_string(),
            }),
            Outcome::Error(failure) => Err(failure),
        }
    }
}

/// Why a request failed: a code to act on and a message for people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request couldn't be read, or its parameters make no sense.
    BadRequest,
    /// No such repository, worktree or file.
    NotFound,
    /// Can't add: not a git repository, already added, a name in use.
    CantAdd,
    /// git or the filesystem failed.
    Io,
    /// The answer didn't have the expected shape (raised by the client).
    BadResponse,
    /// A code from a newer abcd.
    #[serde(other)]
    Other,
}

/// A message abcd pushes: `{"event": "repos.changed", "data": {...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum Event {
    /// The repositories changed: all of them, as they are now.
    #[serde(rename = "repos.changed")]
    ReposChanged { repos: Vec<Repo> },
    /// Files a search found, a few at a time; `search` is its request's id.
    #[serde(rename = "search.found")]
    SearchFound { search: u64, files: Vec<FoundFile> },
}

/// Any line abcd sends after its hello.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerLine {
    Response(Response),
    Event(Event),
}

/// The result of `server.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerStatus {
    pub pid: u32,
    pub build: String,
    pub profile: String,
    /// Seconds since the Unix epoch when abcd started.
    pub started: u64,
    /// Connections open now, this one included.
    pub clients: usize,
}

/// Every repository, as they are now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repos {
    pub repos: Vec<Repo>,
}

/// The result of `repo.add`: the name it was added under, and every repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Added {
    pub name: String,
    pub repos: Vec<Repo>,
}

/// Relative paths, `/`-separated, in no particular order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Files {
    pub files: Vec<String>,
}

/// The result of `search`, after its last `search.found`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchDone {
    /// More lines matched than `max_lines`.
    pub capped: bool,
}

/// The result of requests that answer with nothing: `{}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Done {}

/// `value` as one line of JSON, newline included.
pub fn encode(value: &impl Serialize) -> String {
    let mut line = serde_json::to_string(value).expect("messages serialize");
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `value` encodes to `golden` and decodes back to itself.
    fn golden<T>(value: T, golden: serde_json::Value)
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        assert_eq!(serde_json::to_value(&value).unwrap(), golden);
        assert_eq!(serde_json::from_value::<T>(golden).unwrap(), value);
    }

    #[test]
    fn hello_keeps_its_shape() {
        let client = Hello {
            protocol: 1,
            build: "0.1.0+3f9a2c1e".into(),
            pid: None,
            profile: None,
        };
        golden(
            HelloLine { hello: client },
            json!({"hello": {"protocol": 1, "build": "0.1.0+3f9a2c1e"}}),
        );
        let server = Hello {
            protocol: 1,
            build: "0.1.0+3f9a2c1e".into(),
            pid: Some(4242),
            profile: Some("dev".into()),
        };
        golden(
            HelloLine { hello: server },
            json!({"hello": {"protocol": 1, "build": "0.1.0+3f9a2c1e", "pid": 4242, "profile": "dev"}}),
        );
        let newer: HelloLine = serde_json::from_value(
            json!({"hello": {"protocol": 9, "build": "9.0.0+00000000", "later": true}}),
        )
        .unwrap();
        assert_eq!(newer.hello.protocol, 9, "a newer hello still reads");
    }

    #[test]
    fn requests_name_their_method_and_params() {
        golden(
            Request {
                id: 7,
                call: Call::RepoAdd {
                    path: "~/code/x".into(),
                    name: None,
                },
            },
            json!({"id": 7, "method": "repo.add", "params": {"path": "~/code/x", "name": null}}),
        );
        golden(
            Request {
                id: 1,
                call: Call::ServerStatus,
            },
            json!({"id": 1, "method": "server.status"}),
        );
        golden(
            Request {
                id: 3,
                call: Call::Search {
                    worktree: "/w".into(),
                    query: "gstin".into(),
                    max_lines: 1000,
                },
            },
            json!({"id": 3, "method": "search", "params": {"worktree": "/w", "query": "gstin", "max_lines": 1000}}),
        );
    }

    #[test]
    fn responses_carry_a_result_or_an_error() {
        let ok = Response::ok(7, &SearchDone { capped: true });
        golden(ok.clone(), json!({"id": 7, "result": {"capped": true}}));
        assert_eq!(
            ok.into_result::<SearchDone>(),
            Ok(SearchDone { capped: true })
        );
        let failed = Response::error(8, ErrorCode::CantAdd, "not a git repository");
        golden(
            failed.clone(),
            json!({"id": 8, "error": {"code": "cant_add", "message": "not a git repository"}}),
        );
        assert_eq!(
            failed.into_result::<Repos>().unwrap_err().message,
            "not a git repository"
        );
        let newer: Failure =
            serde_json::from_value(json!({"code": "rate_limited", "message": "later"})).unwrap();
        assert_eq!(newer.code, ErrorCode::Other);
    }

    #[test]
    fn lines_from_abcd_are_responses_or_events() {
        let event = json!({"event": "search.found", "data": {"search": 3, "files": []}});
        assert_eq!(
            serde_json::from_value::<ServerLine>(event).unwrap(),
            ServerLine::Event(Event::SearchFound {
                search: 3,
                files: vec![]
            })
        );
        let response = json!({"id": 3, "result": {}});
        assert!(matches!(
            serde_json::from_value::<ServerLine>(response).unwrap(),
            ServerLine::Response(Response { id: 3, .. })
        ));
        assert_eq!(encode(&Done {}), "{}\n");
    }
}
