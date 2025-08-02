mod flake_nix;
mod lockfile;
mod serde_int_tag_hack;
mod sigint_guard;
mod update;

use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use clap::{Args, Parser, Subcommand, builder::ArgPredicate};
use color_eyre::{
    Result,
    eyre::{Context, OptionExt, bail},
};
use fs_err as fs;
use iddqd::{IdHashItem, IdHashMap, id_hash_map::Entry as IdHashMapEntry};
use owo_colors::{OwoColorize, colors::xterm};
use serde::Deserialize;

use crate::lockfile::{AnalyzedLockfile, Locked, LockfileNode, Original, analyze_lockfile};

struct Flake<'cli> {
    // Currently just the flake ID passed in.
    /// Key in `inputs`
    id: &'cli str,
    /// Parent of `flake.lock`
    directory: PathBuf,
    /// Paths of the gcroots. Below `directory`
    gcroots: Vec<PathBuf>,
    /// Whether the flake has build result gcroots
    has_build_result: bool,
    /// Whether the flake has direnv gcroots
    has_direnv_gc_roots: bool,
    /// Path of `flake.lock`
    lockfile_path: PathBuf,
}

impl Flake<'_> {
    pub fn in_git_repo(&self) -> bool {
        self.directory
            .ancestors()
            .any(|path| path.join(".git").is_dir())
    }
}

impl IdHashItem for Flake<'_> {
    type Key<'a>
        = &'a Path
    where
        Self: 'a;

    fn key(&self) -> Self::Key<'_> {
        &self.directory
    }
    iddqd::id_upcast!();
}

fn filter_gcroot<'cli>(
    entry: &fs::DirEntry,
    flakes: &mut IdHashMap<Flake<'cli>>,
    flake_id: &'cli str,
) -> Result<()> {
    let gcroot = fs::read_link(entry.path())?;
    if !gcroot.exists() {
        return Ok(());
    }

    let Some((directory, is_direnv, is_build_result)) = {
        gcroot
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == ".direnv"))
            .and_then(|direnv_path| direnv_path.parent())
            .map(|path| (path, true, false))
    }
    .or_else(|| {
        gcroot
            .file_name()
            .is_some_and(|name| name == "result" || name.as_encoded_bytes().starts_with(b"result-"))
            .then(|| gcroot.parent())
            .flatten()
            .map(|path| (path, false, true))
    }) else {
        return Ok(());
    };

    match flakes.entry(directory) {
        IdHashMapEntry::Occupied(mut occupied) => {
            let mut existing = occupied.get_mut();
            existing.gcroots.push(gcroot.clone());
            existing.has_direnv_gc_roots |= is_direnv;
            existing.has_build_result |= is_build_result;
        }
        IdHashMapEntry::Vacant(vacant) => {
            let lockfile_path = directory.join("flake.lock");
            if !lockfile_path.exists() {
                return Ok(());
            }

            vacant.insert(Flake {
                id: flake_id,
                directory: directory.to_owned(),
                gcroots: vec![gcroot.clone()],
                has_direnv_gc_roots: is_direnv,
                has_build_result: is_build_result,
                lockfile_path,
            });
        }
    }

    Ok(())
}

/// `nix flake metadata --json` output
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixFlakeMetadata {
    //description: Option<String>,
    //fingerprint: String,
    // lastModified = locked.lastModified?
    locked: Locked,
    locks: lockfile::Lockfile,
    //original: Original,
    //original_url: String,
    /// Equal to `original` except when `original` is indirect.
    resolved: Original,
    resolved_url: String,
    // unused: url: String,
}

enum MatchTarget {
    /// Target a flake's flake ref
    FlakeMetadata(NixFlakeMetadata),
    /// Target a flake's input's flake ref
    FlakeInput {
        input: LockfileNode,
        flake_ref_url: String,
    },
}

impl MatchTarget {
    /// Returns the `locked` key.
    const fn locked(&self) -> &Locked {
        match self {
            Self::FlakeMetadata(metadata) => &metadata.locked,
            Self::FlakeInput { input, .. } => &input.locked,
        }
    }
    /// Returns the `original` key.
    const fn original(&self) -> &Original {
        match self {
            Self::FlakeMetadata(metadata) => &metadata.resolved,
            Self::FlakeInput { input, .. } => &input.original.inner,
        }
    }
    /// Returns the URL-like flake ref with `indirect` flakes resolved for [`MatchTarget::FlakeMetadata`].
    fn flake_ref_url(&self) -> &str {
        match self {
            Self::FlakeMetadata(metadata) => &metadata.resolved_url,
            Self::FlakeInput { flake_ref_url, .. } => flake_ref_url,
        }
    }
}

fn process_flake(
    flake: &Flake,
    cli: &Cli,
    target: &MatchTarget,
    flake_index: usize,
    flakes_count: usize,
) -> Result<()> {
    match &cli.command {
        CliCommand::List => {
            let analyzed_lockfile = analyze_lockfile(&flake.lockfile_path, target, cli)?;

            print_flake_info(flake, target, &analyzed_lockfile)?;
        }
        CliCommand::Update(update_args) => {
            update::update_flake(flake, cli, target, flake_index, flakes_count, update_args)?;
        }
    }

    Ok(())
}

fn print_flake_info(
    flake: &Flake<'_>,
    target: &MatchTarget,
    analyzed_lockfile: &AnalyzedLockfile,
) -> Result<bool> {
    print!("{}", flake.directory.display().fg::<xterm::Gray>(),);
    if flake.has_direnv_gc_roots {
        print!("{}", " (direnv)".green());
    }
    if flake.has_build_result {
        print!("{}", " (build result)".green());
    }
    print!("{}", ":".fg::<xterm::Gray>(),);

    let mut printed = false;

    let ref_matches_target = analyzed_lockfile
        .original
        .ref_()
        .is_some_and(|ref_| Some(ref_) == target.original().ref_());
    if let Some(ref_) = analyzed_lockfile.original.ref_() {
        let matches = target.original().ref_() == Some(ref_);
        if matches {
            if !printed {
                print!(" {}", ref_.green());
            }
        } else {
            print!(" {}", ref_.red());
        }
        printed = true;
    }

    let rev_matches_target = analyzed_lockfile
        .locked
        .rev()
        .is_some_and(|rev| Some(rev) == target.locked().rev());
    if let Some(rev) = analyzed_lockfile.locked.rev() {
        let matches = target.locked().rev() == Some(rev);
        if matches {
            if !printed {
                print!(" {}", rev.green());
            }
        } else {
            print!(" {}", rev.red());
        }
        printed = true;
    }

    let url_matches_target = analyzed_lockfile
        .locked
        .url_no_git()
        .is_some_and(|url| Some(url) == target.locked().url_no_git());
    if let Some(url) = analyzed_lockfile.locked.url_no_git() {
        let matches = target.locked().url_no_git() == Some(url);
        if matches {
            if !printed {
                print!(" {}", url.green());
            }
        } else {
            print!(" {}", url.red());
        }
    }

    let timestamp_matches_target = match analyzed_lockfile.locked.last_modified() {
        None => false,
        Some(last_modified) => {
            let last_modified = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(last_modified))
                .ok_or_eyre("Invalid last_modified; would overflow")?;
            print!(
                " {} {}",
                "last updated".fg::<xterm::Gray>(),
                chrono_humanize::HumanTime::from(last_modified).cyan(),
            );
            false // TODO!!!
        }
    };
    println!();

    // TODO: warn on indirect flakes!!

    let matches_target = (ref_matches_target && timestamp_matches_target)
        || rev_matches_target
        || url_matches_target;
    Ok(matches_target)
}

/// Nix garbage collector root flake updater
///
/// Looks for Nix garbage collector roots in `/nix/var/nix/gcroots/auto` and filters them for
/// `.direnv/**`, `result` and `result-*`.
///
/// Then allows the user to execute operations on the found flakes interactively.
#[derive(Parser)]
#[command(author, version)]
struct Cli {
    /// The name of the input to look for in flakes.
    #[arg(long, default_value = "nixpkgs")]
    input_id: String,

    /// Target flake reference.
    ///
    /// This will be resolved using `nix flake metadata`.
    ///
    /// Use a hash symbol to reference an input of a flake. For example: `./my-nixos-config#nixpkgs`.
    ///
    /// Defaults to `github:NixOS/nixpkgs/nixos-unstable` when `input-id` is set to `nixpkgs`.
    #[arg(long, default_value_if("input_id", ArgPredicate::Equals("nixpkgs".into()), "github:NixOS/nixpkgs/nixos-unstable"))]
    target: String,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Lists the flakes and does not apply any operations on them.
    List,
    /// Updates Nix flake inputs based on a target.
    ///
    /// Updating only works when the new `nix` command is enabled.
    Update(UpdateArgs),
}

#[derive(Args)]
struct UpdateArgs {
    /// Allows writing to files. This flag being unset means a dry run.
    #[arg(long)]
    allow_write: bool,
    /// The number of lines to give as context in the diff.
    #[arg(long, default_value_t = 3)]
    diff_context: usize,
    // TODO: target vs flake-ref vs source??
    // TODO: also support non-gcroot mode with more sources or destinations or targets or flakes!!!
    // TODO: also support taking flakes by recursively finding flake.nix's
}

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .theme(if std::io::stderr().is_terminal() {
            color_eyre::config::Theme::dark()
        } else {
            // Don't attempt color
            color_eyre::config::Theme::new()
        })
        .install()?;

    let cli = Cli::parse();

    if let CliCommand::Update(UpdateArgs {
        allow_write: false, ..
    }) = cli.command
    {
        println!(
            "{}{}",
            "Note: This is a dry run. To modify files and run commands, run again with "
                .yellow()
                .bold(),
            "--allow-write".cyan().bold()
        );
    }

    let target = if let Some((flake_ref, input_id)) = cli.target.rsplit_once('#') {
        let metadata = get_flake_ref_metadata(flake_ref)
            .wrap_err("Failed to get metadata of flake reference")?;
        let input = metadata
            .locks
            .extract_input(input_id)
            .wrap_err("Failed to extract input of flake reference")?;
        MatchTarget::FlakeInput {
            flake_ref_url: get_flake_ref_url(&input)
                .wrap_err("Failed to convert flake reference to URL-like format")?,
            input,
        }
    } else {
        MatchTarget::FlakeMetadata(
            get_flake_ref_metadata(&cli.target)
                .wrap_err("Failed to get metadata of flake reference")?,
        )
    };

    print!("{} {}", cli.input_id.cyan(), "target:".fg::<xterm::Gray>(),);

    if let Some(ref_) = target.original().ref_() {
        print!(" {}", ref_.green());
    } else if let Some(rev) = target.locked().rev() {
        print!(" {}", rev.green());
    } else if let Some(url) = target.locked().url_no_git() {
        print!(" {}", url.green());
    }

    if let Some(last_modified) = target.locked().last_modified() {
        let last_modified = SystemTime::UNIX_EPOCH + Duration::from_secs(last_modified);
        print!(
            " {} {}",
            "last updated".fg::<xterm::Gray>(),
            chrono_humanize::HumanTime::from(last_modified).cyan(),
        );
    }

    println!();

    let mut flakes = IdHashMap::new();

    for entry in fs::read_dir("/nix/var/nix/gcroots/auto")? {
        let entry = entry?;

        if let Err(err) = filter_gcroot(&entry, &mut flakes, &cli.input_id)
            .wrap_err_with(|| format!("Failed to filter gcroot {}", entry.path().display()))
        {
            eprintln!("{err:?}");
        }
    }

    let flakes_count = flakes.len();
    for (flake_index, flake) in flakes.into_iter().enumerate() {
        if let Err(err) = process_flake(&flake, &cli, &target, flake_index, flakes_count)
            .wrap_err_with(|| format!("Failed to process flake {}", flake.directory.display()))
        {
            eprintln!("{err:?}");
        }
    }

    Ok(())
}

fn get_flake_ref_metadata(flake_ref: &str) -> Result<NixFlakeMetadata> {
    let output = {
        let _guard = crate::sigint_guard::SigintGuard::new();

        Command::new("nix")
            .args(["flake", "metadata", "--json", "--", flake_ref])
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()?
    };

    if !output.status.success() {
        bail!("Command failed with {}", output.status);
    }

    serde_json::from_slice(&output.stdout).wrap_err("Failed to parse output")
}

fn get_flake_ref_url(input: &LockfileNode) -> Result<String> {
    let json = serde_json::to_string(&input.original)?;
    let output = {
        // `--argstr` doesn't work at all with `nix eval`
        Command::new("nix-instantiate")
            .args([
                "--eval",
                "--expr",
                "{ json }: builtins.flakeRefToString (builtins.fromJSON json)",
                "--raw",
                "--argstr",
                "json",
                &json,
            ])
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()?
    };

    if !output.status.success() {
        bail!("Command failed with {}", output.status);
    }

    Ok(String::from_utf8(output.stdout)?)
}
