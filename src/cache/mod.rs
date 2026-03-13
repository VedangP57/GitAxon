//! Cache module for local caching and indexing.

use serde::{Deserialize, Serialize};

use crate::errors::{GitfastError, GitfastResult};

/// Repository record stored in the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    /// Unique identifier.
    pub id: String,
    /// Absolute path to the repository.
    pub path: String,
    /// Display name.
    pub name: String,
    /// Last opened timestamp (Unix seconds).
    pub last_opened: i64,
    /// Whether the repository is marked as favorite.
    pub is_favorite: bool,
}

/// Commit node for graph traversal and display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitNode {
    /// Full commit hash.
    pub hash: String,
    /// Short hash (first 7-12 chars).
    pub short_hash: String,
    /// Commit message.
    pub message: String,
    /// Author display name.
    pub author_name: String,
    /// Author email.
    pub author_email: String,
    /// Commit timestamp (Unix seconds).
    pub timestamp: i64,
    /// Parent commit hashes.
    pub parent_hashes: Vec<String>,
}

/// SQLite-backed cache for repositories and commits.
pub struct Cache {
    /// Database connection.
    conn: rusqlite::Connection,
}

fn path_to_id(path: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn to_db_err(e: rusqlite::Error) -> GitfastError {
    GitfastError::DatabaseError(e.to_string())
}

impl Cache {
    /// Opens or creates a SQLite database at the given path and initializes the schema.
    pub fn new(db_path: &str) -> GitfastResult<Self> {
        let conn = rusqlite::Connection::open(db_path).map_err(to_db_err)?;
        let cache = Self { conn };
        cache.init_schema()?;
        Ok(cache)
    }

    /// Creates database tables if they do not exist.
    fn init_schema(&self) -> GitfastResult<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS repositories (
                    id TEXT PRIMARY KEY,
                    path TEXT UNIQUE,
                    name TEXT,
                    last_opened INTEGER,
                    is_favorite INTEGER DEFAULT 0
                );
                CREATE TABLE IF NOT EXISTS cached_commits (
                    repo_id TEXT,
                    hash TEXT,
                    short_hash TEXT,
                    message TEXT,
                    author TEXT,
                    timestamp INTEGER,
                    parent_hashes TEXT,
                    PRIMARY KEY (repo_id, hash)
                );
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value TEXT
                );
                ",
            )
            .map_err(to_db_err)?;
        Ok(())
    }

    /// Adds a repository to the cache and returns its generated ID.
    /// If the path already exists, updates last_opened and name instead of failing.
    pub fn add_repository(&self, path: &str, name: &str) -> GitfastResult<String> {
        let id = path_to_id(path);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| GitfastError::DatabaseError(format!("invalid system time: {}", e)))?
            .as_secs() as i64;

        self.conn
            .execute(
                "INSERT INTO repositories (id, path, name, last_opened, is_favorite) 
                 VALUES (?1, ?2, ?3, ?4, 0)
                 ON CONFLICT(path) DO UPDATE SET last_opened = excluded.last_opened, name = excluded.name",
                rusqlite::params![&id, path, name, now],
            )
            .map_err(to_db_err)?;
        Ok(id)
    }

    /// Returns repositories ordered by last opened (most recent first).
    pub fn get_recent_repositories(&self, limit: usize) -> GitfastResult<Vec<RepoRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, name, last_opened, is_favorite FROM repositories ORDER BY last_opened DESC LIMIT ?1",
        ).map_err(to_db_err)?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(RepoRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                last_opened: row.get(3)?,
                is_favorite: row.get::<_, i64>(4)? != 0,
            })
        }).map_err(to_db_err)?;

        let mut repos = Vec::new();
        for row in rows {
            repos.push(row.map_err(to_db_err)?);
        }
        Ok(repos)
    }

    /// Bulk inserts commits into the cache. Uses INSERT OR REPLACE.
    pub fn save_commits(&self, repo_id: &str, commits: &[CommitNode]) -> GitfastResult<()> {
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO cached_commits (repo_id, hash, short_hash, message, author, timestamp, parent_hashes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        ).map_err(to_db_err)?;

        for commit in commits {
            let author = format!("{} <{}>", commit.author_name, commit.author_email);
            let parent_hashes = commit.parent_hashes.join(",");
            stmt.execute(rusqlite::params![
                repo_id,
                &commit.hash,
                &commit.short_hash,
                &commit.message,
                author,
                commit.timestamp,
                parent_hashes,
            ])
            .map_err(to_db_err)?;
        }
        Ok(())
    }

    /// Returns paginated commits from the cache for a repository.
    pub fn get_cached_commits(
        &self,
        repo_id: &str,
        limit: usize,
        offset: usize,
    ) -> GitfastResult<Vec<CommitNode>> {
        let mut stmt = self.conn
            .prepare(
                "SELECT hash, short_hash, message, author, timestamp, parent_hashes FROM cached_commits WHERE repo_id = ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3",
            )
            .map_err(to_db_err)?;
        let rows = stmt
            .query_map(rusqlite::params![repo_id, limit as i64, offset as i64], |row| {
                let hash: String = row.get(0)?;
                let short_hash: String = row.get(1)?;
                let message: String = row.get(2)?;
                let author: String = row.get(3)?;
                let timestamp: i64 = row.get(4)?;
                let parent_hashes_str: String = row.get(5)?;
                let parent_hashes: Vec<String> = if parent_hashes_str.is_empty() {
                    Vec::new()
                } else {
                    parent_hashes_str.split(',').map(String::from).collect()
                };

                let (author_name, author_email) = parse_author(&author);
                Ok(CommitNode {
                    hash,
                    short_hash,
                    message,
                    author_name,
                    author_email,
                    timestamp,
                    parent_hashes,
                })
            })
            .map_err(to_db_err)?;

        let mut commits = Vec::new();
        for row in rows {
            commits.push(row.map_err(to_db_err)?);
        }
        Ok(commits)
    }
}

fn parse_author(author: &str) -> (String, String) {
    if let Some(open) = author.find(" <") {
        if let Some(close) = author[open + 2..].find('>') {
            let name = author[..open].trim().to_string();
            let email = author[open + 2..open + 2 + close].to_string();
            return (name, email);
        }
    }
    (author.to_string(), String::new())
}
