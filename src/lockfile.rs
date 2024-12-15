use std::{borrow::Cow, path::Path};

use color_eyre::eyre::{bail, Context, OptionExt, Result};
use fs_err as fs;
use sonic_rs::JsonValueTrait;

use crate::{
    flake_ref::git_hosting_svc_fmt,
    json_helpers::{get_opt_json, get_three_pointers, get_two_pointers},
    Cli,
};

pub struct AnalyzedLockfile<'cli> {
    pub new_flake_ref: Option<Cow<'cli, str>>,
    pub allow_update: bool,
    pub local_rev: String,
}

pub fn analyze_lockfile<'cli>(
    path: &Path,
    global_rev: &str,
    cli: &'cli Cli,
) -> Result<Option<AnalyzedLockfile<'cli>>> {
    let lockfile_contents = fs::read(&path)?;

    let (nodes, root, version) =
        get_three_pointers(&*lockfile_contents, ["nodes"], ["root"], ["version"])?;

    match version.as_u64().ok_or_eyre("Invalid lockfile")? {
        7 => {}
        num => bail!("Unsupported version {num}"),
    }

    let root_node_id = root.as_str().ok_or_eyre("Invalid lockfile")?;

    let target_node_id = sonic_rs::get(nodes.as_raw_str(), [root_node_id, "inputs", &cli.flake_id])
        .wrap_err("Missing target")?;
    let target_node_id = target_node_id.as_str().ok_or_eyre("Invalid lockfile")?;

    let local_rev = sonic_rs::get(nodes.as_raw_str(), [target_node_id, "locked", "rev"])
        .wrap_err("Invalid lockfile")?;
    let local_rev = local_rev.as_str().ok_or_eyre("Invalid lockfile")?;

    if local_rev == global_rev {
        return Ok(None);
    }

    let original = sonic_rs::get(nodes.as_raw_str(), [target_node_id, "original"])
        .wrap_err("Invalid lockfile")?;
    let original_type =
        sonic_rs::get(original.as_raw_str(), ["type"]).wrap_err("Invalid lockfile")?;
    let original_type = original_type.as_str().ok_or_eyre("Invalid lockfile")?;

    Ok(Some(match original_type {
        "indirect" => {
            // Input is either nonexistent or set to a registry flake id
            let rev = get_opt_json(original.as_raw_str(), ["rev"]).wrap_err("Invalid lockfile")?;
            let ref_ = get_opt_json(original.as_raw_str(), ["ref"]).wrap_err("Invalid lockfile")?;

            AnalyzedLockfile {
                new_flake_ref: cli.set_flake_ref.as_deref().map(Cow::Borrowed),
                allow_update: rev.is_none() && ref_.is_none(),
                local_rev: local_rev.to_owned(),
            }
        }
        "path" | "tarball" | "file" => {
            // Not supported
            bail!("Type {original_type} is not supported")
        }
        "git" => {
            bail!("Git is not yet implemented")
        }
        "mercurial" => {
            // Not yet implemented
            bail!("Mercurial is not yet implemented")
        }
        "github" | "gitlab" | "sourcehut" => {
            let (owner, repo) = get_two_pointers(original.as_raw_str(), ["owner"], ["repo"])
                .wrap_err("Invalid lockfile")?;
            let owner = owner.as_str().ok_or_eyre("Invalid lockfile")?;
            let repo = repo.as_str().ok_or_eyre("Invalid lockfile")?;

            AnalyzedLockfile {
                new_flake_ref: Some(Cow::Owned(git_hosting_svc_fmt(
                    original_type,
                    owner,
                    repo,
                    Some(global_rev),
                    None,
                ))),
                allow_update: false,
                local_rev: local_rev.to_owned(),
            }
        }
        _ => bail!("Invalid lockfile"),
    }))
}
