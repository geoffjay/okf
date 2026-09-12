//! Workspace developer tasks.
//!
//! ```text
//! cargo xtask tailwind   # compile crates/okf-web/assets/tailwind.css -> site.css
//! ```
//!
//! The `tailwind` task compiles the site's Tailwind v4 source with the Tailwind
//! standalone CLI — a self-contained executable that needs no Node. The binary
//! is resolved in order: `$TAILWIND_BIN`, then a cached download under
//! `target/tailwind/`, fetched from GitHub on first use (`$TAILWIND_VERSION`
//! pins a release tag; otherwise the latest is used). The generated `site.css`
//! is committed, so `okf site` never runs this tool — only contributors editing
//! the stylesheet do.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        Some("tailwind") => match run_tailwind() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("xtask: {e}");
                ExitCode::FAILURE
            }
        },
        other => {
            eprintln!(
                "usage: cargo xtask <task>\n\ntasks:\n  tailwind   compile the okf-web stylesheet"
            );
            if let Some(task) = other {
                eprintln!("\nunknown task: {task}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Compiles `crates/okf-web/assets/tailwind.css` into the committed `site.css`.
fn run_tailwind() -> Result<(), String> {
    let root = workspace_root();
    let assets = root.join("crates").join("okf-web").join("assets");
    let input = assets.join("tailwind.css");
    let output = assets.join("site.css");
    let bin = resolve_binary(&root)?;

    println!(
        "xtask: compiling {} -> {}",
        input.display(),
        output.display()
    );
    let status = Command::new(&bin)
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--minify")
        // Run from the assets dir so any relative resolution stays local.
        .current_dir(&assets)
        .status()
        .map_err(|e| format!("failed to run {}: {e}", bin.display()))?;
    if !status.success() {
        return Err(format!("tailwind exited with {status}"));
    }

    let bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    println!("xtask: wrote {} ({bytes} bytes)", output.display());
    Ok(())
}

/// The workspace root — the parent of this crate's directory.
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

/// Resolves the Tailwind standalone binary, downloading it on first use.
fn resolve_binary(root: &Path) -> Result<PathBuf, String> {
    if let Some(bin) = env::var_os("TAILWIND_BIN") {
        return Ok(PathBuf::from(bin));
    }

    let asset = tailwind_asset()?;
    let (url, tag) = match env::var("TAILWIND_VERSION") {
        Ok(v) if !v.is_empty() => (
            format!(
                "https://github.com/tailwindlabs/tailwindcss/releases/download/{v}/tailwindcss-{asset}"
            ),
            v,
        ),
        _ => (
            format!(
                "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-{asset}"
            ),
            "latest".to_string(),
        ),
    };

    let cache = root.join("target").join("tailwind");
    let bin = cache.join(format!("tailwindcss-{tag}-{asset}"));
    if bin.exists() {
        return Ok(bin);
    }

    std::fs::create_dir_all(&cache).map_err(|e| format!("create {}: {e}", cache.display()))?;
    println!("xtask: downloading {url}");
    download(&url, &bin)?;
    make_executable(&bin)?;
    Ok(bin)
}

/// The Tailwind standalone asset name for the host platform.
fn tailwind_asset() -> Result<&'static str, String> {
    Ok(match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => "macos-arm64",
        ("macos", "x86_64") => "macos-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("windows", "x86_64") => "windows-x64.exe",
        (os, arch) => {
            return Err(format!(
                "no Tailwind standalone build for {os}/{arch}; set $TAILWIND_BIN to a local binary"
            ));
        }
    })
}

/// Downloads `url` to `dest` with `curl`.
fn download(url: &str, dest: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .arg("--fail")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("--output")
        .arg(dest)
        .arg(url)
        .status()
        .map_err(|e| format!("failed to run curl (is it installed?): {e}"))?;
    if !status.success() {
        return Err(format!("curl failed for {url} ({status})"));
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}
