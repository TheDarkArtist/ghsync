use crate::github::{Repo, run_cmd};
use anyhow::Result;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub enum Status {
    Cloned,
    Updated,
    Failed,
}

impl Status {
    pub fn icon(&self) -> &'static str {
        match self {
            Status::Cloned => "+",
            Status::Updated => "~",
            Status::Failed => "!",
        }
    }
}

pub struct BackupResult {
    pub nwo: String,
    pub status: Status,
    pub error: Option<String>,
}

fn backup_repo(repo: &Repo, dest: &Path, mirror: bool) -> BackupResult {
    let nwo = repo.name_with_owner.as_str();
    let (owner, name) = nwo.split_once('/').unwrap_or(("", nwo));
    let repo_dir = dest.join(owner).join(name);
    let repo_dir_str = repo_dir.display().to_string();

    let result: std::result::Result<Status, String> = if repo_dir.exists() {
        let cmd = if mirror {
            vec!["git", "-C", repo_dir_str.as_str(), "remote", "update"]
        } else {
            vec!["git", "-C", repo_dir_str.as_str(), "fetch", "--all"]
        };
        run_cmd(&cmd)
            .map(|_| Status::Updated)
            .map_err(|e| e.to_string())
    } else {
        if let Some(parent) = repo_dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut args = vec![
            "gh",
            "repo",
            "clone",
            nwo,
            repo_dir_str.as_str(),
            "--",
        ];
        if mirror {
            args.push("--mirror");
        }
        run_cmd(&args)
            .map(|_| Status::Cloned)
            .map_err(|e| e.to_string())
    };

    match result {
        Ok(status) => BackupResult {
            nwo: nwo.to_string(),
            status,
            error: None,
        },
        Err(e) => BackupResult {
            nwo: nwo.to_string(),
            status: Status::Failed,
            error: Some(e),
        },
    }
}

struct Progress {
    count: usize,
    results: Vec<BackupResult>,
}

pub fn run_backup(repos: &[Repo], dest: &Path, mirror: bool, jobs: usize) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let dest = std::fs::canonicalize(dest)?;
    let mode = if mirror { "mirror" } else { "regular" };
    println!(
        "\nBacking up to: {} (mode: {mode}, workers: {jobs})",
        dest.display()
    );

    let total = repos.len();
    let next_idx = AtomicUsize::new(0);
    let progress = Mutex::new(Progress {
        count: 0,
        results: Vec::with_capacity(total),
    });

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let idx = next_idx.fetch_add(1, Ordering::SeqCst);
                if idx >= total {
                    break;
                }
                let result = backup_repo(&repos[idx], &dest, mirror);
                let mut p = progress.lock().unwrap();
                p.count += 1;
                let i = p.count;
                println!("  [{i}/{total}] [{}] {}", result.status.icon(), result.nwo);
                if let Some(ref e) = result.error {
                    if let Some(first_line) = e.lines().next() {
                        println!("           {first_line}");
                    }
                }
                p.results.push(result);
            });
        }
    });

    let results = progress.into_inner().unwrap().results;
    let cloned = results
        .iter()
        .filter(|r| matches!(r.status, Status::Cloned))
        .count();
    let updated = results
        .iter()
        .filter(|r| matches!(r.status, Status::Updated))
        .count();
    let failed: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.status, Status::Failed))
        .collect();

    println!("\n--- Summary ---");
    println!("  Cloned:  {cloned}");
    println!("  Updated: {updated}");
    println!("  Failed:  {}", failed.len());

    if !failed.is_empty() {
        println!("\nFailed repos:");
        for r in &failed {
            println!("  - {}", r.nwo);
        }
        std::process::exit(1);
    }

    Ok(())
}
