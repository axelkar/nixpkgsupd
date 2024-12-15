use color_eyre::{
    eyre::{Context, Result},
    owo_colors::OwoColorize,
};

pub fn set_flake_input_url(
    new_flake_ref: &str,
    old_contents: &str,
    cli: &crate::Cli,
) -> Result<String> {
    let input_url_path = &format!("inputs.{}.url", cli.flake_id);

    let new_contents = nix_editor::write::write(
        old_contents,
        input_url_path,
        &format!("{:?}", new_flake_ref),
    )
    .wrap_err("Invalid flake.nix")?;

    // Print the diff
    let diff = diff::lines(&old_contents, &new_contents);
    let diff = reduce_diff_context(&diff, cli.diff_context);
    for line in diff {
        match line {
            diff::Result::Left(line) => println!("{}", format_args!("-{line}").red()),
            diff::Result::Both(line, _) => println!(" {line}"),
            diff::Result::Right(line) => println!("{}", format_args!("+{line}").green()),
        }
    }

    let escaped_flake_id = regex::escape(&cli.flake_id);
    let regex = regex::Regex::new(&format!(
        r"#[ \t\n\r]*(inputs\.)?{}(\.url)?[ \t\n\r]*=",
        escaped_flake_id
    ))?;
    if regex.is_match(old_contents) {
        eprintln!(
            "{}",
            "Note: Found a comment defining the input. Use the editor to apply this.".yellow()
        );
    }

    Ok(new_contents)
}

fn reduce_diff_context<T: PartialEq>(
    input: &[diff::Result<T>],
    context: usize,
) -> Vec<&diff::Result<T>> {
    let mut result = Vec::new();

    let mut diff_indices = Vec::new();
    for (idx, res) in input.iter().enumerate() {
        if matches!(res, diff::Result::Left(_) | diff::Result::Right(_)) {
            diff_indices.push(idx);
        }
    }

    let len = input.len();

    for &diff_idx in &diff_indices {
        // Determine the range: `Both` lines before and after the diff
        let start = diff_idx.saturating_sub(context);
        let end = (diff_idx + context + 1).min(len);

        for diff in input.iter().take(end).skip(start) {
            // Avoid duplicates
            if !result.contains(&diff) {
                result.push(diff);
            }
        }
    }

    result
}
