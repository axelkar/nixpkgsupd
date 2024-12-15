/// Formats the flake ref for a Git hosting service
pub fn git_hosting_svc_fmt(
    type_: &str,
    owner: &str,
    repo: &str,
    rev_or_ref: Option<&str>,
    params: Option<&str>,
) -> String {
    let mut s = format!("{type_}:{owner}/{repo}");
    if let Some(rev_or_ref) = rev_or_ref {
        s += "/";
        s += rev_or_ref;
    }
    if let Some(params) = params {
        s += "?";
        s += params;
    }
    s
}
