mod flake_nix;
mod flake_ref;
mod json_helpers;
mod lockfile;
mod registry;

use std::{
    collections::HashSet,
    io::{stderr, stdin, IsTerminal, Write},
    path::{Path, PathBuf},
    process::Command,
};

use clap::{builder::ArgPredicate, Parser};
use color_eyre::{
    eyre::{bail, Context, OptionExt},
    Result,
};
use flake_nix::set_flake_input_url;
use fs_err as fs;
use lockfile::{analyze_lockfile, AnalyzedLockfile};
use owo_colors::{colors::xterm, OwoColorize};
use registry::get_rev_from_registry;

fn process_gcroot(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    cli: &Cli,
    global_rev: &str,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let Some((directory, is_direnv)) = {
        path.ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == ".direnv"))
            .and_then(|direnv_path| direnv_path.parent())
            .map(|path| (path, true))
    }
    .or_else(|| {
        path.file_name()
            .is_some_and(|name| name == "result")
            .then(|| path.parent())
            .flatten()
            .map(|path| (path, false))
    }) else {
        return Ok(());
    };

    let run_cmd = |program: &str, args: &[&str]| {
        color_eyre::eyre::Ok(
            Command::new(program)
                .args(args)
                .current_dir(directory)
                .status()?
                .success(),
        )
    };

    let lockfile_path = directory.join("flake.lock");
    if !lockfile_path.exists() || visited.contains(directory) {
        return Ok(());
    }

    visited.insert(directory.to_owned());

    let Some(AnalyzedLockfile {
        new_flake_ref,
        allow_update,
        local_rev,
    }) = analyze_lockfile(&lockfile_path, global_rev, cli)?
    else {
        return Ok(());
    };

    println!(
        "{}{}",
        format_args!("{}: ", directory.display()).fg::<xterm::Gray>(),
        local_rev.fg::<xterm::Red>()
    );

    let flake_nix = directory.join("flake.nix");
    if !flake_nix.exists() {
        bail!("flake.nix does not exist")
    }
    let flake_nix_contents = fs::read_to_string(&flake_nix)?;

    let mut flake_nix_new_contents = new_flake_ref
        .as_ref()
        .map(|new_flake_ref| set_flake_input_url(new_flake_ref, &flake_nix_contents, cli))
        .transpose()?;

    loop {
        if allow_update {
            eprintln!("{}", "Note: The indirect reference can be updated".yellow());
        }

        eprint!(
            "{}",
            format_args!(
                "Write this? [{}n,e,{}?] ",
                flake_nix_new_contents
                    .is_some()
                    .then_some("y,")
                    .unwrap_or_default(),
                allow_update.then_some("u,").unwrap_or_default()
            )
            .blue()
        );
        stderr().flush()?;
        let mut buf = String::new();
        stdin().read_line(&mut buf)?;

        match (buf.trim(), &flake_nix_new_contents) {
            ("y", Some(contents)) => {
                if !cli.allow_write {
                    break;
                }
                fs::write(&flake_nix, contents)?;

                if !run_cmd("nix", &["flake", "lock"])? {
                    eprintln!("Failed to recreate lockfile. Try editing flake.nix.");
                    flake_nix_new_contents = new_flake_ref
                        .as_ref()
                        .map(|new_flake_ref| {
                            set_flake_input_url(new_flake_ref, &flake_nix_contents, cli)
                        })
                        .transpose()?;
                    continue;
                }
            }
            ("n", _) => {
                eprintln!("{}", "Skipping this change".red());
                break;
            }
            ("e", _) => {
                if !cli.allow_write {
                    break;
                }
                let status = Command::new(
                    std::env::var_os("EDITOR").ok_or_eyre("EDITOR environment variable missing")?,
                )
                .arg(&flake_nix)
                .status()?;
                if !status.success() {
                    eprintln!("{}", "Editor exited with nonzero exit code".red());
                }

                flake_nix_new_contents = new_flake_ref
                    .as_ref()
                    .map(|new_flake_ref| {
                        set_flake_input_url(new_flake_ref, &flake_nix_contents, cli)
                    })
                    .transpose()?;

                continue;
            }
            ("u", _) if allow_update => {
                if !cli.allow_write {
                    break;
                }
                if !run_cmd("nix", &["flake", "update", &cli.flake_id])? {
                    eprintln!(
                        "{}",
                        "Failed to update indirect input. Try another method.".red()
                    );
                    continue;
                }
            }
            _ => {
                if new_flake_ref.is_some() {
                    eprintln!("y - Write change and run `nix flake lock`");
                }
                eprintln!("n - Skip change");
                eprintln!("e - Edit the file using $EDITOR. Make sure to copy the change first");
                if allow_update {
                    eprintln!(
                        "u - Update an indirect reference by running `nix flake update {}`",
                        cli.flake_id
                    );
                    eprintln!("    This is only supported by indirect references without rev or ref specifiers");
                }
                eprintln!("? - Print help");
                continue;
            }
        }

        if is_direnv {
            eprint!("{}", "Update direnv? [y,n] ".blue());
            stderr().flush()?;
            let mut buf = String::new();
            stdin().read_line(&mut buf)?;

            match buf.trim() {
                "y" if cli.allow_write => {
                    if !run_cmd("direnv", &["exec", ".", "true"])? {
                        eprintln!("{}", "Failed to reload direnv.".red());
                        continue;
                    }
                }
                _ => {}
            }
        }

        if directory.ancestors().any(|path| path.join(".git").is_dir()) {
            let is_empty = !run_cmd("git", &["log", "-0"])?;
            let stage_is_dirty = !run_cmd("git", &["diff", "--quiet", "--cached", "--exit-code"])?;

            eprint!(
                "{}{}{}{}{}",
                "Commit ".blue(),
                "flake.nix".blue().bold(),
                " and ".blue(),
                "flake.lock".blue().bold(),
                " into Git? [y,n] ".blue()
            );

            if is_empty {
                eprint!("{}", "(No commits yet) ".yellow());
            }

            if stage_is_dirty {
                eprint!("{}", "(Stage is dirty) ".yellow());
            }

            stderr().flush()?;
            let mut buf = String::new();
            stdin().read_line(&mut buf)?;

            match buf.trim() {
                "y" if cli.allow_write => {
                    if run_cmd("git", &["add", "flake.nix", "flake.lock"])? {
                        if !run_cmd(
                            "git",
                            &[
                                "commit",
                                "-m",
                                &format!("chore: bump flake input {}", cli.flake_id),
                            ],
                        )? {
                            eprintln!("{}", "Failed to commit.".red());
                        }
                    } else {
                        eprintln!("{}", "Failed to stage files.".red());
                    }
                }
                _ => {}
            }
        }

        break;
    }

    eprintln!();
    Ok(())
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The name of the flake in the Nix flake registry and as an input
    #[arg(short, long, default_value = "nixpkgs")]
    flake_id: String,
    /// Sets a flake reference for an indirect
    ///
    /// The rev/ref is optional and will be automatically fetched from the registry
    ///
    /// Defaults to `github:NixOS/nixpkgs` when `flake_id` is set to `nixpkgs`
    #[arg(short='r', long, default_value_if("flake_id", ArgPredicate::Equals("nixpkgs".into()), "github:NixOS/nixpkgs"))]
    set_flake_ref: Option<String>,
    /// Write to the files
    #[arg(long)]
    allow_write: bool,
    /// The number of lines to give as context in the idff.
    #[arg(long, default_value_t = 3)]
    diff_context: usize,
}

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .theme(if !std::io::stderr().is_terminal() {
            // Don't attempt color
            color_eyre::config::Theme::new()
        } else {
            color_eyre::config::Theme::dark()
        })
        .install()?;

    let mut cli = Cli::parse();

    if !cli.allow_write {
        println!(
            "{}{}",
            "Note: This is a dry run. To modify files and run commands, run again with "
                .yellow()
                .bold(),
            "--allow-write".cyan().bold()
        );
    }

    let rev =
        get_rev_from_registry(&cli.flake_id).wrap_err("Failed to get rev from Nix registry")?;

    if let Some(set_flake_ref) = &mut cli.set_flake_ref {
        if !set_flake_ref.starts_with("github:")
            && !set_flake_ref.starts_with("gitlab:")
            && !set_flake_ref.starts_with("sourcehut:")
        {
            bail!("Unsupported set_flake_ref type")
        }

        let set_has_rev = set_flake_ref.bytes().filter(|ch| *ch == b'/').count() >= 2;
        if !set_has_rev {
            set_flake_ref.push('/');
            set_flake_ref.push_str(&rev);
        }
    }

    println!(
        "{}{}",
        "Global rev: ".fg::<xterm::Gray>(),
        rev.fg::<xterm::Lime>()
    );

    let mut visited = HashSet::new();

    for entry in fs::read_dir("/nix/var/nix/gcroots/auto")? {
        let path = match (|| {
            let entry = entry?;
            color_eyre::eyre::Ok(fs::read_link(entry.path())?)
        })() {
            Ok(path) => path,
            Err(err) => {
                let err = err.wrap_err("Failed to process gcroot");
                eprintln!("{err:?}");
                continue;
            }
        };

        if let Err(err) = process_gcroot(&path, &mut visited, &cli, &rev)
            .wrap_err_with(|| format!("Failed to process gcroot {}", path.display()))
        {
            eprintln!("{err:?}");
        }
    }

    Ok(())
}
