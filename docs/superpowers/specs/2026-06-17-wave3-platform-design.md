# Wave 3 — Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Make GitAxon self-updating and work with SSH remotes — the two biggest distribution and compatibility gaps.

**Architecture:** Auto-updater uses Tauri's first-party plugin. SSH auth hooks into `git2`'s existing credential callback system — no new UI required for the happy path.

**Tech Stack:** Tauri 2 · `tauri-plugin-updater` · Rust `git2` SSH agent callback · SvelteKit 5

---

## Feature 1 — Auto-Updater

### What
On every app startup, GitAxon silently checks GitHub Releases for a newer version. If one exists, a non-blocking toast appears: **"v0.2.0 available — Update now"**. Clicking it downloads and installs the update, then relaunches the app.

### How it works
Tauri's `tauri-plugin-updater` fetches a `latest.json` manifest published alongside each release. If the version is newer than the running build, it offers the update.

### CI changes (`release.yml`)
After building both DMGs, generate and upload `latest.json`:
```json
{
  "version": "0.2.0",
  "notes": "See release notes on GitHub",
  "pub_date": "2026-06-17T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "url": "https://github.com/VedangP57/GitAxon/releases/download/v0.2.0/GitAxon_0.2.0_aarch64.dmg",
      "signature": ""
    },
    "darwin-x86_64": {
      "url": "https://github.com/VedangP57/GitAxon/releases/download/v0.2.0/GitAxon_0.2.0_x64.dmg",
      "signature": ""
    }
  }
}
```

### Rust backend
- Add `tauri-plugin-updater` to `Cargo.toml` and register in `src-tauri/src/lib.rs`.
- On app startup, spawn a background task: fetch manifest → compare versions → if newer, emit `update-available` event with version string.

### Frontend
- `ui/src/lib/store.ts` — listen for `update-available` Tauri event, set `updateAvailable` store.
- `ui/src/components/AppShell.svelte` — show toast when `$updateAvailable` is set. Toast has "Update now" button that calls `invoke('install_update')`.

### tauri.conf.json change
```json
"plugins": {
  "updater": {
    "endpoints": ["https://github.com/VedangP57/GitAxon/releases/latest/download/latest.json"],
    "dialog": false,
    "pubkey": ""
  }
}
```

---

## Feature 2 — SSH Auth Support

### What
When pushing/fetching/cloning over SSH remotes (`git@github.com:...`), `git2` currently fails with an auth error because no credential callback is set. This adds an SSH agent callback so that keys already loaded into the OS SSH agent (`ssh-add`) are used automatically.

### Approach
Every Rust function that calls `git2::Remote::fetch()` or `push()` currently passes `None` for callbacks. Replace with a `RemoteCallbacks` that tries (in order):
1. SSH agent (`credentials_ssh_key_from_agent`)
2. Default SSH key paths (`~/.ssh/id_ed25519`, `~/.ssh/id_rsa`)
3. Returns `git2::Error` → surfaces as a toast in the UI

No new UI is needed for the happy path — if `ssh-agent` is running and has the key, it just works. For the unhappy path (no agent, wrong passphrase), the error toast already exists.

### Files changed
- `src/remotes/mod.rs` — `build_callbacks()` helper that returns `RemoteCallbacks` with SSH agent + key fallback. Used by `fetch_remote`, `push_branch`, `clone_repo`.
- `src/branches/mod.rs` — same callback passed to any `fetch` calls there.

### SSH key fallback order
```rust
fn build_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut cb = RemoteCallbacks::new();
    cb.credentials(|_url, username, allowed| {
        let user = username.unwrap_or("git");
        // 1. Try SSH agent
        if allowed.contains(CredentialType::SSH_KEY) {
            if let Ok(cred) = Cred::ssh_key_from_agent(user) {
                return Ok(cred);
            }
        }
        // 2. Try default key paths
        let home = dirs::home_dir().unwrap_or_default();
        for key in &["id_ed25519", "id_rsa", "id_ecdsa"] {
            let path = home.join(".ssh").join(key);
            if path.exists() {
                if let Ok(cred) = Cred::ssh_key(user, None, &path, None) {
                    return Ok(cred);
                }
            }
        }
        Err(git2::Error::from_str("No SSH credentials found. Run ssh-add first."))
    });
    cb
}
```
