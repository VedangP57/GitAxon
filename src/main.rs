//! Gitfast-core CLI.

use clap::{Parser, Subcommand};
use gitfast_core::branches;
use gitfast_core::diff;
use gitfast_core::graph;
use gitfast_core::remotes;
use gitfast_core::staging;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gitfast")]
#[command(about = "Fast git operations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show commit graph with lane assignments
    Graph {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Show commit log
    Log {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Show diff
    Diff {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Show diff for specific commit
        #[arg(long)]
        commit: Option<String>,
    },
    /// Stage files
    Stage {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Stage specific file
        #[arg(long)]
        file: Option<String>,
        /// Stage all modified and untracked
        #[arg(long)]
        all: bool,
    },
    /// Unstage files
    Unstage {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Unstage specific file
        #[arg(long)]
        file: Option<String>,
        /// Unstage all
        #[arg(long)]
        all: bool,
    },
    /// List branches
    Branches {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Fetch from remote
    Fetch {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, default_value = "origin")]
        remote: String,
    },
    /// Push to remote
    Push {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, default_value = "origin")]
        remote: String,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        force: bool,
    },
    /// Pull from remote
    Pull {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, default_value = "origin")]
        remote: String,
        #[arg(long)]
        branch: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Graph { repo } => run_graph(repo).await,
        Commands::Log { repo } => run_log(repo).await,
        Commands::Diff { repo, commit } => run_diff(repo, commit).await,
        Commands::Stage { repo, file, all } => run_stage(repo, file, all).await,
        Commands::Unstage { repo, file, all } => run_unstage(repo, file, all).await,
        Commands::Branches { repo } => run_branches(repo).await,
        Commands::Fetch { repo, remote } => run_fetch(repo, remote).await,
        Commands::Push {
            repo,
            remote,
            branch,
            force,
        } => run_push(repo, remote, branch, force).await,
        Commands::Pull { repo, remote, branch } => run_pull(repo, remote, branch).await,
    }
}

async fn run_graph(repo: PathBuf) {
    let path = repo.to_string_lossy();
    match graph::get_laned_commits_json(path.as_ref(), 100, 0).await {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn run_log(repo: PathBuf) {
    let path = repo.to_string_lossy();
    match graph::get_commits_json(path.as_ref(), 100, 0).await {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn run_diff(repo: PathBuf, commit: Option<String>) {
    let path = repo.to_string_lossy();
    let result = match commit {
        Some(hash) => diff::diff_commit_json(path.as_ref(), &hash).await,
        None => diff::diff_working_tree_json(path.as_ref()).await,
    };
    match result {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn run_stage(repo: PathBuf, file: Option<String>, all: bool) {
    let path = repo.to_string_lossy();
    let result = match (file, all) {
        (Some(f), _) => staging::stage_file(path.as_ref(), &f).await,
        (None, true) => staging::stage_all(path.as_ref()).await,
        (None, false) => {
            eprintln!("Error: specify --file <path> or --all");
            return;
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }
}

async fn run_unstage(repo: PathBuf, file: Option<String>, all: bool) {
    let path = repo.to_string_lossy();
    let result = match (file, all) {
        (Some(f), _) => staging::unstage_file(path.as_ref(), &f).await,
        (None, true) => staging::unstage_all(path.as_ref()).await,
        (None, false) => {
            eprintln!("Error: specify --file <path> or --all");
            return;
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }
}

async fn run_branches(repo: PathBuf) {
    let path = repo.to_string_lossy();
    match branches::list_branches_json(path.as_ref()).await {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn run_fetch(repo: PathBuf, remote: String) {
    let path = repo.to_string_lossy();
    match remotes::fetch(path.as_ref(), &remote).await {
        Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn run_push(repo: PathBuf, remote: String, branch: String, force: bool) {
    let path = repo.to_string_lossy();
    match remotes::push(path.as_ref(), &remote, &branch, force).await {
        Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn run_pull(repo: PathBuf, remote: String, branch: String) {
    let path = repo.to_string_lossy();
    match remotes::pull(path.as_ref(), &remote, &branch).await {
        Ok(msg) => println!("{}", msg),
        Err(e) => eprintln!("Error: {}", e),
    }
}
