use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Repo {
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
    #[serde(rename = "sshUrl")]
    pub ssh_url: String,
    #[serde(rename = "isFork")]
    pub is_fork: bool,
    #[serde(rename = "isArchived")]
    pub is_archived: bool,
    pub visibility: String,
}

pub struct Filters<'a> {
    pub org: &'a [String],
    pub orgs_only: bool,
    pub personal_only: bool,
    pub no_forks: bool,
    pub forks_only: bool,
    pub no_archived: bool,
    pub archived_only: bool,
    pub visibility: Option<&'a str>,
    pub patterns: &'a [String],
    pub exclude: &'a [String],
}

pub fn run_cmd(cmd: &[&str]) -> Result<String> {
    let output = Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .with_context(|| format!("failed to execute: {}", cmd[0]))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("command failed: {}\n{}", cmd.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn check_gh() -> Result<()> {
    match Command::new("gh").args(["auth", "status"]).output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => bail!("gh CLI is not authenticated.\nRun: gh auth login"),
        Err(_) => bail!("gh CLI is not installed.\nInstall: https://cli.github.com/"),
    }
}

pub fn get_username() -> Result<String> {
    run_cmd(&["gh", "api", "/user", "--jq", ".login"])
}

pub fn get_orgs() -> Result<Vec<String>> {
    let output = run_cmd(&["gh", "api", "/user/orgs", "--paginate", "--jq", ".[].login"])?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

pub fn list_repos(owner: &str) -> Result<Vec<Repo>> {
    let output = run_cmd(&[
        "gh",
        "repo",
        "list",
        owner,
        "--limit",
        "500",
        "--json",
        "nameWithOwner,sshUrl,isFork,isArchived,visibility",
    ])?;
    let repos: Vec<Repo> = serde_json::from_str(&output)?;
    Ok(repos)
}

pub fn discover_repos(filters: &Filters, username: &str, orgs: &[String]) -> Result<Vec<Repo>> {
    let owners: Vec<String> = if !filters.org.is_empty() {
        let invalid: Vec<&String> = filters
            .org
            .iter()
            .filter(|o| !orgs.iter().any(|x| x.eq_ignore_ascii_case(o)))
            .collect();
        if !invalid.is_empty() {
            let names: Vec<&str> = invalid.iter().map(|s| s.as_str()).collect();
            let org_names: Vec<&str> = orgs.iter().map(|s| s.as_str()).collect();
            bail!(
                "not a member of org(s): {}\nYour orgs: {}",
                names.join(", "),
                org_names.join(", ")
            );
        }
        orgs.iter()
            .filter(|o| filters.org.iter().any(|x| x.eq_ignore_ascii_case(o)))
            .cloned()
            .collect()
    } else if filters.orgs_only {
        orgs.to_vec()
    } else if filters.personal_only {
        vec![username.to_string()]
    } else {
        let mut v = vec![username.to_string()];
        v.extend(orgs.iter().cloned());
        v
    };

    println!(
        "Scanning: {} ({} owner(s))",
        owners.join(", "),
        owners.len()
    );

    let mut seen = HashMap::new();
    for owner in &owners {
        let repos = list_repos(owner)?;
        for repo in repos {
            seen.entry(repo.name_with_owner.clone())
                .or_insert(repo);
        }
    }

    let mut repos: Vec<Repo> = seen.into_values().collect();

    if filters.no_forks {
        let before = repos.len();
        repos.retain(|r| !r.is_fork);
        let excluded = before - repos.len();
        if excluded > 0 {
            println!("Excluded {excluded} fork(s)");
        }
    }

    if filters.forks_only {
        repos.retain(|r| r.is_fork);
    }

    if filters.no_archived {
        let before = repos.len();
        repos.retain(|r| !r.is_archived);
        let excluded = before - repos.len();
        if excluded > 0 {
            println!("Excluded {excluded} archived repo(s)");
        }
    }

    if filters.archived_only {
        repos.retain(|r| r.is_archived);
    }

    if let Some(vis) = filters.visibility {
        repos.retain(|r| r.visibility.eq_ignore_ascii_case(vis));
    }

    if !filters.patterns.is_empty() {
        repos.retain(|r| {
            let name = r.name_with_owner.split_once('/').map(|(_, n)| n).unwrap_or("");
            filters.patterns.iter().any(|p| glob_match(p, name))
        });
    }

    if !filters.exclude.is_empty() {
        repos.retain(|r| {
            let name = r.name_with_owner.split_once('/').map(|(_, n)| n).unwrap_or("");
            !filters.exclude.iter().any(|p| glob_match(p, name))
        });
    }

    repos.sort_by(|a, b| {
        a.name_with_owner
            .to_lowercase()
            .cmp(&b.name_with_owner.to_lowercase())
    });
    println!("Found {} repo(s)", repos.len());
    Ok(repos)
}

fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_bytes(pattern.to_lowercase().as_bytes(), text.to_lowercase().as_bytes())
}

fn glob_match_bytes(p: &[u8], t: &[u8]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            glob_match_bytes(&p[1..], t) || (!t.is_empty() && glob_match_bytes(p, &t[1..]))
        }
        (Some(b'?'), Some(_)) => glob_match_bytes(&p[1..], &t[1..]),
        (Some(a), Some(b)) if a == b => glob_match_bytes(&p[1..], &t[1..]),
        _ => false,
    }
}
