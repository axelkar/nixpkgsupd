use std::{collections::HashMap, fs, path::Path};

use color_eyre::eyre::{OptionExt, Result, WrapErr};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Cli, serde_int_tag_hack::Version};

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Lockfile {
    V7 {
        #[serde(rename = "version")]
        _version: Version<7>,
        #[serde(rename = "root")]
        root_id: String,
        #[serde(rename = "nodes")]
        raw_nodes: HashMap<String, Value>,
    },
}
impl Lockfile {
    pub fn extract_input(self, input_id: &str) -> Result<LockfileNode> {
        let Self::V7 {
            root_id, raw_nodes, ..
        } = self;
        let raw_node = raw_nodes
            .get(&root_id)
            .and_then(|root_node| {
                let child_id = root_node.get("inputs")?.get(input_id)?.as_str()?;
                raw_nodes.get(child_id)
            })
            .ok_or_eyre("could not locate target node in lockfile")?;

        let node =
            serde_json::from_value(raw_node.clone()).wrap_err("failed to deserialize node")?;

        Ok(node)
    }
}

/// The shape of the one node we actually want to fully decode.
#[derive(Deserialize, Debug)]
pub struct LockfileNode {
    pub locked: Locked,
    pub original: OriginalExtra,
}

/// Description of the version currently used. [`LockfileNode::locked`]
///
/// <https://nix.dev/manual/nix/2.28/command-ref/new-cli/nix3-flake.html#types>
#[derive(Deserialize, Serialize, Debug)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Locked {
    Path {
        path: String,
        /// From flake inputs saved to the Nix registry using NixOS or home-manager
        rev: Option<String>,
        last_modified: u64,
    },
    Tarball {
        url: String,
        // Provided by server
        rev: Option<String>,
        // Provided by server
        last_modified: Option<u64>,
    },
    Git {
        /// The commit time of the revision `rev` as an integer denoting the number of seconds since 1970.
        last_modified: Option<u64>,
        /// You basically never want to use this. If it's specified by the user, it's in
        /// [`Original`].
        #[serde(rename = "ref")]
        ref_: String,
        rev: String,
        shallow: Option<bool>,
        url: String,
    },
    #[serde(untagged)]
    GitService {
        #[serde(rename = "type")]
        type_: GitServiceType,
        owner: String,
        repo: String,
        rev: String,
        /// The commit time of the revision `rev` as an integer denoting the number of seconds since 1970.
        last_modified: Option<u64>,
        host: Option<String>,
    },
    #[serde(untagged)]
    Other {
        #[serde(rename = "type")]
        type_: String,
        rev: Option<String>,
        url: Option<String>,
        last_modified: Option<u64>,
    },
}
impl Locked {
    pub fn rev(&self) -> Option<&str> {
        match self {
            Self::Path { rev, .. } | Self::Tarball { rev, .. } | Self::Other { rev, .. } => {
                rev.as_deref()
            }
            Self::GitService { rev, .. } | Self::Git { rev, .. } => Some(rev),
        }
    }
    pub fn url_no_git(&self) -> Option<&str> {
        match self {
            Self::Tarball { url, .. } => Some(url),
            Self::Path { .. } | Self::Git { .. } | Self::GitService { .. } | Self::Other { .. } => {
                None
            }
        }
    }
    pub const fn last_modified(&self) -> Option<u64> {
        match self {
            Self::Path { last_modified, .. } => Some(*last_modified),
            Self::Tarball { last_modified, .. }
            | Self::Git { last_modified, .. }
            | Self::GitService { last_modified, .. }
            | Self::Other { last_modified, .. } => *last_modified,
        }
    }
}

/// Description of what was parsed from `flake.nix`. [`LockfileNode::original`]
///
/// Git services know whether a rev or ref was specified in `rev-or-ref`.
///
/// <https://nix.dev/manual/nix/2.28/command-ref/new-cli/nix3-flake.html#types>
#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Original {
    /// Performs a lookup of
    ///
    /// Form: `[flake:]<flake-id>(/<rev-or-ref>(/rev)?)?`
    Indirect {
        /// ID in the flake registry
        id: String,
        rev: Option<String>,
        /// Example: `inputs.nixpkgs.url = "nixpkgs/nixos-unstable";`
        #[serde(rename = "ref")]
        ref_: Option<String>,
    },
    Path,
    Tarball {
        // url: String,
    },
    File,
    Git {
        // Either can be without the other
        #[serde(rename = "ref")]
        ref_: Option<String>,
        // rev: Option<String>,
        // shallow: Option<bool>,
        // url: String,
    },
    Mercurial,

    /// Form: `github:<owner>/<repo>(/<rev-or-ref>)?(\?<params>)?`
    ///
    /// `host` param is also supported
    /// `flake` param is NOT supported
    // TODO: extra params!!
    #[serde(untagged)]
    GitService {
        #[serde(rename = "type")]
        _type: GitServiceType,
        //owner: String,
        //repo: String,
        // unused: host: Option<String>,
        // unused: rev: Option<String>,
        #[serde(rename = "ref")]
        ref_: Option<String>,
    },

    #[serde(untagged)]
    Other {
        #[serde(rename = "type")]
        type_: String,
    },
}
impl Original {
    pub fn ref_(&self) -> Option<&str> {
        match self {
            Self::Indirect { ref_, .. }
            | Self::GitService { ref_, .. }
            | Self::Git { ref_, .. } => ref_.as_deref(),
            Self::Path
            | Self::Tarball { .. }
            | Self::File
            | Self::Mercurial
            | Self::Other { .. } => None,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub struct OriginalExtra {
    #[serde(flatten)]
    pub inner: Original,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum GitServiceType {
    GitHub,
    GitLab,
    Sourcehut,
}

pub fn load_lockfile_input(path: &Path, cli: &Cli) -> Result<LockfileNode> {
    let input_id = &cli.input_id;
    let contents = fs::read(path)?;
    let lockfile: Lockfile =
        serde_json::from_slice(&contents).wrap_err("failed to parse top level of lockfile")?;

    let node = lockfile.extract_input(input_id)?;

    Ok(node)
}
