use crate::backup::run_backup;
use crate::github::{self, Filters};
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ghsync",
    about = "Back up all GitHub repos (personal + org)",
    after_help = r#"EXAMPLES:
    ghsync --dry-run                          List all repos
    ghsync --dest ~/backup                    Mirror-clone everything
    ghsync --org tdacorp --org tdaorg         Only these orgs
    ghsync --orgs-only --no-forks             All orgs, skip forks
    ghsync --personal-only                    Only personal repos
    ghsync --match "tda-*"                    Repos matching glob
    ghsync --exclude "poc-*" --no-archived    Skip POCs and archived
    ghsync --visibility private               Only private repos
    ghsync --list-orgs                        Show orgs and exit"#
)]
struct Cli {
    /// Back up specific org(s) only (repeatable)
    #[arg(long, value_name = "NAME", help_heading = "Scope")]
    org: Vec<String>,

    /// Back up org repos only, skip personal
    #[arg(long, conflicts_with_all = ["personal_only"], help_heading = "Scope")]
    orgs_only: bool,

    /// Back up personal repos only, skip orgs
    #[arg(long, conflicts_with_all = ["orgs_only", "org"], help_heading = "Scope")]
    personal_only: bool,

    /// List orgs and exit
    #[arg(long, help_heading = "Scope")]
    list_orgs: bool,

    /// Exclude forked repos
    #[arg(long, conflicts_with = "forks_only", help_heading = "Filters")]
    no_forks: bool,

    /// Only forked repos
    #[arg(long, conflicts_with = "no_forks", help_heading = "Filters")]
    forks_only: bool,

    /// Exclude archived repos
    #[arg(long, conflicts_with = "archived_only", help_heading = "Filters")]
    no_archived: bool,

    /// Only archived repos
    #[arg(long, conflicts_with = "no_archived", help_heading = "Filters")]
    archived_only: bool,

    /// Filter by visibility
    #[arg(long, value_parser = ["public", "private", "internal"], help_heading = "Filters")]
    visibility: Option<String>,

    /// Only repos matching glob pattern (repeatable)
    #[arg(long = "match", value_name = "GLOB", help_heading = "Filters")]
    patterns: Vec<String>,

    /// Exclude repos matching glob pattern (repeatable)
    #[arg(long, value_name = "GLOB", help_heading = "Filters")]
    exclude: Vec<String>,

    /// Destination directory
    #[arg(long, default_value = ".", help_heading = "Clone Options")]
    dest: PathBuf,

    /// Use regular clone instead of --mirror
    #[arg(long, help_heading = "Clone Options")]
    no_mirror: bool,

    /// Parallel workers
    #[arg(long, default_value_t = 4, help_heading = "Clone Options")]
    jobs: usize,

    /// List repos without cloning
    #[arg(long)]
    dry_run: bool,
}

pub fn run() -> Result<()> {
    let args = Cli::parse();

    github::check_gh()?;
    let username = github::get_username()?;
    println!("Authenticated as: {username}");

    let orgs = github::get_orgs()?;

    if args.list_orgs {
        println!("\nOrgs ({}):", orgs.len());
        let mut sorted = orgs.clone();
        sorted.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        for o in &sorted {
            let count = github::list_repos(o)?.len();
            println!("  {o} ({count} repos)");
        }
        return Ok(());
    }

    let filters = Filters {
        org: &args.org,
        orgs_only: args.orgs_only,
        personal_only: args.personal_only,
        no_forks: args.no_forks,
        forks_only: args.forks_only,
        no_archived: args.no_archived,
        archived_only: args.archived_only,
        visibility: args.visibility.as_deref(),
        patterns: &args.patterns,
        exclude: &args.exclude,
    };
    let repos = github::discover_repos(&filters, &username, &orgs)?;

    if repos.is_empty() {
        println!("No repos matched.");
        return Ok(());
    }

    if args.dry_run {
        println!("\n--- Dry run ---");
        let total = repos.len();
        for (i, r) in repos.iter().enumerate() {
            let mut tags = Vec::new();
            if r.is_fork {
                tags.push("fork".to_string());
            }
            if r.is_archived {
                tags.push("archived".to_string());
            }
            let vis = r.visibility.to_lowercase();
            if !vis.is_empty() {
                tags.push(vis);
            }
            let suffix = if tags.is_empty() {
                String::new()
            } else {
                format!("  ({})", tags.join(", "))
            };
            println!("  [{}/{}] {}{}", i + 1, total, r.name_with_owner, suffix);
        }
        println!("\nTotal: {total} repos");
        return Ok(());
    }

    let mirror = !args.no_mirror;
    run_backup(&repos, &args.dest, mirror, args.jobs)?;

    Ok(())
}
