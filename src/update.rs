use std::{
    io::{Write, stderr, stdin},
    ops::ControlFlow,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use color_eyre::{
    Result,
    eyre::{Context, OptionExt, bail},
};
use fs_err as fs;
use owo_colors::{OwoColorize, colors::xterm};

use crate::{
    Flake, UpdateArgs, flake_nix::print_diff, flake_nix::replace_flake_input_url,
    lockfile::analyze_lockfile, print_flake_info,
};

/// Runs the given command and returns whether it was successful.
pub fn run_cmd(program: &str, args: &[&str], dir: &Path) -> Result<bool> {
    let _guard = crate::sigint_guard::SigintGuard::new();

    Ok(Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()?
        .success())
}

pub fn update_flake(
    flake: &Flake,
    cli: &crate::Cli,
    target: &crate::MatchTarget,
    flake_index: usize,
    flakes_count: usize,
    update_args: &UpdateArgs,
) -> Result<()> {
    let flake_nix = flake.directory.join("flake.nix");
    if !flake_nix.exists() {
        bail!("flake.nix does not exist")
    }

    let target_flake_ref = target.flake_ref_url();

    loop {
        println!();
        let analyzed_lockfile = analyze_lockfile(&flake.lockfile_path, target, cli)?;
        let lock_matches_target = print_flake_info(flake, target, &analyzed_lockfile)?;

        let current_flake_nix = fs::read_to_string(&flake_nix)?;

        let new_flake_nix =
            replace_flake_input_url(target_flake_ref, &current_flake_nix, flake.id)?;

        print_diff(&current_flake_nix, &new_flake_nix, update_args);

        let escaped_flake_id = regex::escape(flake.id);
        let regex = regex::Regex::new(&format!(
            r"#[ \t\n\r]*(inputs\.)?{escaped_flake_id}(\.url)?[ \t\n\r]*="
        ))?;
        if regex.is_match(&current_flake_nix) {
            eprintln!(
                "{} {} {}",
                "Found a comment defining the input. Use".yellow(),
                PromptCommand::LaunchEditor.cyan(),
                "to remove it before applying the diff.".yellow()
            );
        }

        let changes_exist = new_flake_nix != current_flake_nix;

        if !changes_exist && !lock_matches_target {
            eprintln!("{} {} {} {} {}", "The `flake.nix` is up to date but the locked version doesn't match the target. Try".yellow(), PromptCommand::Lock.cyan(), "or".yellow(), PromptCommand::RefreshDirenv.cyan(), "to update the lockfile".yellow());
        }

        if lock_matches_target {
            eprintln!("{} {} {} {} {}", "The locked version matches the target but the gcroots may not be up to date. You can try".yellow(), PromptCommand::DeleteGcroots.cyan(), "or".yellow(), PromptCommand::RefreshDirenv.cyan(), "to clean up the gcroots.".yellow());
        }

        eprint!(
            "{}",
            format_args!(
                "({}/{}) [{}{},{},{},{},{},{},{},{}?] ",
                flake_index + 1,
                flakes_count,
                changes_exist.then_some("a,").unwrap_or_default(),
                PromptCommand::NextFlake,
                PromptCommand::LaunchEditor,
                PromptCommand::LaunchShell,
                PromptCommand::RunNixFlakeUpdate,
                PromptCommand::DeleteGcroots,
                PromptCommand::Lock,
                PromptCommand::RefreshDirenv,
                flake.in_git_repo().then_some("commit,").unwrap_or_default(),
            )
            .blue()
        );

        let cmd_string = read_line()?;
        let cmd_string = cmd_string.trim();

        let cmd = PromptCommand::from_str(cmd_string).unwrap_or_else(|_| {
            if !cmd_string.is_empty() {
                eprintln!(
                    "{}",
                    format_args!("Unknown command: {}", cmd_string.red()).red()
                );
            }
            PromptCommand::PrintHelp
        });

        let flow = execute_prompt_cmd(update_args, flake, &flake_nix, &new_flake_nix, cmd)?;

        match flow {
            ControlFlow::Break(()) => break,
            ControlFlow::Continue(()) => {}
        }
    }

    Ok(())
}

#[expect(clippy::too_many_lines, reason = "Really can't shorten this any more")]
fn execute_prompt_cmd(
    update_args: &UpdateArgs,
    flake: &Flake,
    flake_nix: &PathBuf,
    new_flake_nix: &str,
    cmd: PromptCommand,
) -> Result<ControlFlow<()>> {
    let check_dry_run_here = matches!(
        cmd,
        PromptCommand::ApplyDiff
            | PromptCommand::RunNixFlakeUpdate
            | PromptCommand::DeleteGcroots
            | PromptCommand::Lock
    );
    if check_dry_run_here && !update_args.allow_write {
        eprintln!("{}", "Dry run, not modifying files".yellow());
        return Ok(ControlFlow::Continue(()));
    }

    match cmd {
        PromptCommand::ApplyDiff => {
            fs::write(flake_nix, new_flake_nix)?;

            eprintln!(
                "{} {} {}",
                "You should execute one of the following:".yellow(),
                PromptCommand::Lock.cyan(),
                PromptCommand::RefreshDirenv.cyan(),
            );
        }
        PromptCommand::NextFlake => {
            eprintln!("{}", "Going to the next flake".green());
            return Ok(ControlFlow::Break(()));
        }
        PromptCommand::LaunchEditor => {
            let status = Command::new(
                std::env::var_os("EDITOR").ok_or_eyre("EDITOR environment variable missing")?,
            )
            .current_dir(&flake.directory)
            .arg(flake_nix)
            .status()?;

            if !status.success() {
                eprintln!("{}", "Editor exited with nonzero exit code".red());
            }

            eprintln!(
                "{} {} {}",
                "You have been returned to the prompt. Select".green(),
                PromptCommand::Lock.cyan(),
                "or similar if you have applied edits manually.".green()
            );
        }
        PromptCommand::LaunchShell => {
            const PROMPTEXTRA_ADDITION: &str = concat!(env!("CARGO_PKG_NAME"), " shell ");

            let mut cmd = Command::new(
                std::env::var_os("SHELL").ok_or_eyre("SHELL environment variable missing")?,
            );

            if let Some(mut env) = std::env::var_os("PROMPTEXTRA") {
                env.push(" ");
                env.push(PROMPTEXTRA_ADDITION);
                cmd.env("PROMPTEXTRA", env);
            } else {
                cmd.env("PROMPTEXTRA", PROMPTEXTRA_ADDITION);
            }

            let status = cmd.current_dir(&flake.directory).status()?;

            if !status.success() {
                eprintln!("{}", "Shell exited with nonzero exit code".red());
            }

            eprintln!(
                "{} {} {}",
                "You have been returned to the prompt. Select".green(),
                PromptCommand::Lock.cyan(),
                "or similar if you have applied edits manually.".green()
            );
        }
        PromptCommand::RunNixFlakeUpdate => {
            if !run_cmd("nix", &["flake", "update", flake.id], &flake.directory)? {
                eprintln!(
                    "{}",
                    "Failed to update indirect input. Try another method.".red()
                );
                return Ok(ControlFlow::Continue(()));
            }

            if flake.has_direnv_gc_roots {
                refresh_direnv(update_args, flake)?;
            }
            if flake.in_git_repo() {
                git_commit_changes(update_args, flake)?;
            }
        }
        PromptCommand::DeleteGcroots => {
            eprintln!("Deleting garbage collector root.");
            for gcroot in &flake.gcroots {
                fs::remove_file(gcroot).wrap_err("Failed to remove garbage collector root")?;
            }
        }
        PromptCommand::Lock => {
            if !run_cmd("nix", &["flake", "lock"], &flake.directory)? {
                eprintln!("Failed to recreate lockfile. Try manually editing flake.nix.");
                return Ok(ControlFlow::Continue(()));
            }

            if flake.has_direnv_gc_roots {
                refresh_direnv(update_args, flake)?;
            }
            if flake.in_git_repo() {
                git_commit_changes(update_args, flake)?;
            }
        }
        PromptCommand::RefreshDirenv => {
            refresh_direnv(update_args, flake)?;
        }
        PromptCommand::Commit => {
            git_commit_changes(update_args, flake)?;
        }
        PromptCommand::PrintHelp => {
            for cmd in PromptCommand::ALL {
                eprintln!(
                    "{:<6} {} {}",
                    cmd.cyan(),
                    "-".fg::<xterm::Gray>(),
                    cmd.description()
                );
            }
        }
    }
    Ok(ControlFlow::Continue(()))
}

#[derive(Clone, Copy, strum::EnumString, strum::Display)]
enum PromptCommand {
    #[strum(serialize = "a")]
    ApplyDiff,
    #[strum(serialize = "n")]
    NextFlake,
    #[strum(serialize = "e")]
    LaunchEditor,
    #[strum(serialize = "sh")]
    LaunchShell,
    #[strum(serialize = "up")]
    RunNixFlakeUpdate,
    #[strum(serialize = "dg")]
    DeleteGcroots,
    #[strum(serialize = "lock")]
    Lock,
    #[strum(serialize = "direnv")]
    RefreshDirenv,
    #[strum(serialize = "commit")]
    Commit,
    #[strum(serialize = "?")]
    PrintHelp,
}
impl PromptCommand {
    const ALL: &[Self] = &[
        Self::ApplyDiff,
        Self::NextFlake,
        Self::LaunchEditor,
        Self::LaunchShell,
        Self::RunNixFlakeUpdate,
        Self::DeleteGcroots,
        Self::Lock,
        Self::RefreshDirenv,
        Self::Commit,
        Self::PrintHelp,
    ];
    const fn description(self) -> &'static str {
        match self {
            Self::ApplyDiff => "Applies the change",
            Self::NextFlake => "Proceeds to the next flake",
            Self::LaunchEditor => "Edits `flake.nix` using `$EDITOR`",
            Self::LaunchShell => "Launches `$SHELL` in the flake's directory",
            Self::RunNixFlakeUpdate => "Runs `nix flake update <input id>",
            Self::DeleteGcroots => "Deletes garbage collector roots like build results and direnv",
            Self::Lock => "Runs `nix flake lock`",
            Self::RefreshDirenv => "Refreshes direnv",
            Self::Commit => "Makes a Git commit with `flake.nix` and `flake.lock`",
            Self::PrintHelp => "Prints help",
        }
    }
}

fn refresh_direnv(update_args: &UpdateArgs, flake: &Flake) -> Result<()> {
    eprint!("{}", "Refresh direnv? [y,N] ".blue());
    let buf = read_line()?;
    if buf.trim() == "y" {
        if update_args.allow_write {
            if !run_cmd("direnv", &["exec", ".", "true"], &flake.directory)? {
                // FIXME: This never even happens...
                // `direnv: nix-direnv: Evaluating current devShell failed. Falling back to previous environment!` and exit code 0
                eprintln!("{}", "Failed to reload direnv.".red());
            }
        } else {
            eprintln!("{}", "Dry run, not modifying files".yellow());
        }
    }
    Ok(())
}

fn git_commit_changes(
    update_args: &UpdateArgs,
    flake: &Flake<'_>,
) -> Result<(), color_eyre::eyre::Error> {
    let is_empty = !run_cmd("git", &["log", "-0"], &flake.directory)?;
    let stage_is_dirty = !run_cmd(
        "git",
        &["diff", "--quiet", "--cached", "--exit-code"],
        &flake.directory,
    )?;
    eprint!(
        "{} {} {} {} {} ",
        "Commit".blue(),
        "flake.nix".cyan().bold(),
        "and".blue(),
        "flake.lock".cyan().bold(),
        "into Git?".blue()
    );
    if is_empty {
        eprint!("{} ", "(No commits yet)".yellow());
    }
    if stage_is_dirty {
        eprint!("{} ", "(Stage is dirty)".yellow());
    }

    let commit_msg = format!("chore: bump flake input {}", flake.id);
    eprint!(
        "\n{} {} {} ",
        "Commit message:".blue(),
        commit_msg.cyan().bold(),
        "[y,N]".blue(),
    );

    let buf = read_line()?;
    if buf.trim() == "y" {
        if update_args.allow_write {
            if run_cmd("git", &["add", "flake.nix", "flake.lock"], &flake.directory)? {
                if !run_cmd("git", &["commit", "-m", &commit_msg], &flake.directory)? {
                    eprintln!("{}", "Failed to commit.".red());
                }
            } else {
                eprintln!("{}", "Failed to stage files.".red());
            }
        } else {
            eprintln!("{}", "Dry run, not modifying files".yellow());
        }
    }
    Ok(())
}

fn read_line() -> Result<String> {
    stderr().flush()?;
    let mut buf = String::new();
    stdin().read_line(&mut buf)?;
    Ok(buf)
}
