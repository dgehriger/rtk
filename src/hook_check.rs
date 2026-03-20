use std::path::PathBuf;

const CURRENT_HOOK_VERSION: u8 = 2;
const WARN_INTERVAL_SECS: u64 = 24 * 3600;

/// Hook status for diagnostics and `rtk gain`.
#[derive(Debug, PartialEq, Clone)]
pub enum HookStatus {
    /// Hook is installed and up to date.
    Ok,
    /// Hook exists but is outdated or unreadable.
    Outdated,
    /// No hook file found (but Claude Code is installed).
    Missing,
}

/// Return the current hook status without printing anything.
/// Returns `Ok` if no Claude Code is detected (not applicable).
pub fn status() -> HookStatus {
    // Don't warn users who don't have Claude Code installed
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return HookStatus::Ok,
    };
    if !home.join(".claude").exists() {
        return HookStatus::Ok;
    }

    let Some(hook_path) = hook_installed_path() else {
        return HookStatus::Missing;
    };
    let Ok(content) = std::fs::read_to_string(&hook_path) else {
        return HookStatus::Outdated; // exists but unreadable — treat as needs-update
    };
    if parse_hook_version(&content) >= CURRENT_HOOK_VERSION {
        HookStatus::Ok
    } else {
        HookStatus::Outdated
    }
}

/// Check if the installed hook is missing or outdated, warn once per day.
pub fn maybe_warn() {
    // Don't block startup — fail silently on any error
    let _ = check_and_warn();
}

/// Single source of truth: delegates to `status()` then rate-limits the warning.
fn check_and_warn() -> Option<()> {
    let warning = match status() {
        HookStatus::Ok => return Some(()),
        HookStatus::Missing => {
            "[rtk] /!\\ No hook installed — run `rtk init -g` for automatic token savings"
        }
        HookStatus::Outdated => "[rtk] /!\\ Hook outdated — run `rtk init -g` to update",
    };

    // Rate limit: warn once per day
    let marker = warn_marker_path()?;
    if let Ok(meta) = std::fs::metadata(&marker) {
        if let Ok(modified) = meta.modified() {
            if modified.elapsed().map(|e| e.as_secs()).unwrap_or(u64::MAX) < WARN_INTERVAL_SECS {
                return Some(());
            }
        }
    }

    eprintln!("{}", warning);

    // Touch marker after warning is printed.
    // Write a non-empty payload so Windows updates the mtime (writing
    // 0 bytes to an already-empty file does not update mtime on NTFS).
    let _ = std::fs::create_dir_all(marker.parent()?);
    let _ = std::fs::write(&marker, b"1");

    Some(())
}

pub fn parse_hook_version(content: &str) -> u8 {
    // JSON hook files (e.g. rtk-rewrite.json) are always current — they invoke
    // `rtk hook` which is built into the binary, so there's no version skew.
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') {
        return CURRENT_HOOK_VERSION;
    }

    // Shell hook: version tag must be in the first 5 lines (shebang + header convention)
    for line in content.lines().take(5) {
        if let Some(rest) = line.strip_prefix("# rtk-hook-version:") {
            if let Ok(v) = rest.trim().parse::<u8>() {
                return v;
            }
        }
    }
    0 // No version tag = version 0 (outdated)
}

fn hook_installed_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Claude Code hook (Unix shell script)
    let claude_hook = home.join(".claude").join("hooks").join("rtk-rewrite.sh");
    if claude_hook.exists() {
        return Some(claude_hook);
    }

    // Claude Code hook (Windows: `rtk hook` command in settings.json)
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if content.contains("rtk hook") || content.contains("rtk-rewrite") {
                return Some(settings_path);
            }
        }
    }

    // VS Code Copilot / Copilot CLI hook (cross-platform JSON config).
    // Check both the current working directory and the home-level .github/hooks.
    let copilot_hook_name = std::path::Path::new(".github")
        .join("hooks")
        .join("rtk-rewrite.json");
    if copilot_hook_name.exists() {
        return Some(copilot_hook_name.to_path_buf());
    }
    let home_copilot = home.join(&copilot_hook_name);
    if home_copilot.exists() {
        return Some(home_copilot);
    }

    None
}

fn warn_marker_path() -> Option<PathBuf> {
    let data_dir = dirs::data_local_dir()?.join("rtk");
    Some(data_dir.join(".hook_warn_last"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hook_version_present() {
        let content = "#!/usr/bin/env bash\n# rtk-hook-version: 2\n# some comment\n";
        assert_eq!(parse_hook_version(content), 2);
    }

    #[test]
    fn test_parse_hook_version_missing() {
        let content = "#!/usr/bin/env bash\n# old hook without version\n";
        assert_eq!(parse_hook_version(content), 0);
    }

    #[test]
    fn test_parse_hook_version_future() {
        let content = "#!/usr/bin/env bash\n# rtk-hook-version: 5\n";
        assert_eq!(parse_hook_version(content), 5);
    }

    #[test]
    fn test_parse_hook_version_no_tag() {
        assert_eq!(parse_hook_version("no version here"), 0);
        assert_eq!(parse_hook_version(""), 0);
    }

    #[test]
    fn test_parse_hook_version_json_file() {
        let content = r#"{ "hooks": { "PreToolUse": [{ "type": "command", "command": "rtk hook" }] } }"#;
        assert_eq!(parse_hook_version(content), CURRENT_HOOK_VERSION);
    }

    #[test]
    fn test_hook_status_enum() {
        assert_ne!(HookStatus::Ok, HookStatus::Missing);
        assert_ne!(HookStatus::Outdated, HookStatus::Missing);
        assert_eq!(HookStatus::Ok, HookStatus::Ok);
        // Clone works
        let s = HookStatus::Missing;
        assert_eq!(s.clone(), HookStatus::Missing);
    }

    #[test]
    fn test_status_returns_valid_variant() {
        // Skip on machines without Claude Code or without hook
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return,
        };
        if !home
            .join(".claude")
            .join("hooks")
            .join("rtk-rewrite.sh")
            .exists()
        {
            // No hook — status should be Missing (if .claude exists) or Ok (if not)
            let s = status();
            if home.join(".claude").exists() {
                assert_eq!(s, HookStatus::Missing);
            } else {
                assert_eq!(s, HookStatus::Ok);
            }
            return;
        }
        let s = status();
        assert!(
            s == HookStatus::Ok || s == HookStatus::Outdated,
            "Expected Ok or Outdated when hook exists, got {:?}",
            s
        );
    }
}
