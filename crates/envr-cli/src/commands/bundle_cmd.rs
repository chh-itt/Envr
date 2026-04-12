//! `envr bundle` — portable offline bundle create/apply.

use crate::cli::{BundleCmd, GlobalArgs};
use crate::commands::common;
use crate::output;

use envr_config::project_config::{
    load_project_config_profile, PROJECT_CONFIG_FILE, PROJECT_CONFIG_LOCAL_FILE,
};
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, VersionSpec, parse_runtime_kind};
use envr_error::EnvrError;
use envr_error::EnvrResult;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use zip::write::FileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

pub fn run(g: &GlobalArgs, cmd: BundleCmd) -> i32 {
    match cmd {
        BundleCmd::Create {
            output,
            path,
            profile,
            include_indexes,
            include_shims,
            full,
            no_current,
        } => create(
            g,
            output,
            path,
            profile,
            include_indexes,
            include_shims,
            full,
            no_current,
        ),
        BundleCmd::Apply {
            file,
            runtime_root,
            index_cache_dir,
        } => apply(g, file, runtime_root, index_cache_dir),
    }
}

fn default_bundle_zip_path() -> PathBuf {
    let secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::env::current_dir()
        .map(|cwd| cwd.join(format!("envr-bundle-{secs}.zip")))
        .unwrap_or_else(|_| PathBuf::from(format!("envr-bundle-{secs}.zip")))
}

fn create(
    g: &GlobalArgs,
    output_path: Option<PathBuf>,
    working_dir: PathBuf,
    profile: Option<String>,
    include_indexes: bool,
    include_shims: bool,
    full: bool,
    no_current: bool,
) -> i32 {
    if full && no_current {
        return common::print_envr_error(
            g,
            EnvrError::Validation("`--full` cannot be combined with `--no-current`".to_string()),
        );
    }

    let runtime_root = match common::effective_runtime_root() {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let bundle_zip = output_path.unwrap_or_else(default_bundle_zip_path);
    if let Some(parent) = bundle_zip.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return common::print_envr_error(g, EnvrError::from(e));
        }
    }

    // Project config discovery (optional).
    let loaded = match load_project_config_profile(&working_dir, profile.as_deref()) {
        Ok(v) => v,
        Err(e) => return common::print_envr_error(g, e),
    };
    let (cfg_loc, pinned_specs): (Option<PathBuf>, Vec<(String, String)>) = match loaded {
        Some((cfg, loc)) => {
            let mut pins: Vec<(String, String)> = Vec::new();
            for (k, rc) in cfg.runtimes {
                if let Some(v) = rc.version {
                    let kt = k.trim().to_string();
                    let vt = v.trim().to_string();
                    if !kt.is_empty() && !vt.is_empty() {
                        pins.push((kt, vt));
                    }
                }
            }
            pins.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            pins.dedup();
            (Some(loc.dir), pins)
        }
        None => (None, Vec::new()),
    };

    let service = match RuntimeService::with_runtime_root(runtime_root.clone()) {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, e),
    };

    let mut included_versions: Vec<(RuntimeKind, String)> = Vec::new();
    for (k, spec) in &pinned_specs {
        let kind = match parse_runtime_kind(k) {
            Ok(v) => v,
            Err(e) => return common::print_envr_error(g, e),
        };
        let resolved = match service.resolve(kind, &VersionSpec(spec.clone())) {
            Ok(r) => r.version.0,
            Err(e) => return common::print_envr_error(g, e),
        };
        included_versions.push((kind, resolved));
    }
    included_versions.sort_by(|a, b| {
        crate::commands::common::kind_label(a.0)
            .cmp(crate::commands::common::kind_label(b.0))
            .then(a.1.cmp(&b.1))
    });
    included_versions.dedup();

    let mut global_current: Vec<(String, String)> = Vec::new();
    if !no_current {
        for kind in [
            RuntimeKind::Node,
            RuntimeKind::Python,
            RuntimeKind::Java,
            RuntimeKind::Go,
            RuntimeKind::Rust,
            RuntimeKind::Php,
            RuntimeKind::Deno,
            RuntimeKind::Bun,
        ] {
            if let Ok(Some(v)) = service.current(kind) {
                global_current.push((crate::commands::common::kind_label(kind).to_string(), v.0));
            }
        }
    }
    // Ensure current versions are included in payload.
    for (k, v) in &global_current {
        let kind = match parse_runtime_kind(k) {
            Ok(v) => v,
            Err(e) => return common::print_envr_error(g, e),
        };
        included_versions.push((kind, v.clone()));
    }
    included_versions.sort_by(|a, b| {
        crate::commands::common::kind_label(a.0)
            .cmp(crate::commands::common::kind_label(b.0))
            .then(a.1.cmp(&b.1))
    });
    included_versions.dedup();

    let index_cache_dir = {
        let platform = match envr_platform::paths::current_platform_paths() {
            Ok(p) => p,
            Err(e) => return common::print_envr_error(g, e),
        };
        envr_platform::paths::index_cache_dir_from_platform(&platform)
    };

    let included_versions_manifest: Vec<(String, String)> = included_versions
        .iter()
        .map(|(k, v)| (crate::commands::common::kind_label(*k).to_string(), v.clone()))
        .collect();

    let manifest = serde_json::json!({
        "format": 1,
        "created_at_unix_secs": SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs()).unwrap_or(0),
        "runtime_root_hint": runtime_root.to_string_lossy(),
        "working_dir": working_dir.to_string_lossy(),
        "profile": profile,
        "include_indexes": include_indexes,
        "include_shims": include_shims,
        "full": full,
        "no_current": no_current,
        "project_dir": cfg_loc.as_ref().map(|p| p.to_string_lossy().to_string()),
        "project_pins": pinned_specs,
        "global_current": global_current,
        "included_versions": included_versions_manifest,
    });
    let manifest_json = match serde_json::to_string_pretty(&manifest) {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, EnvrError::Runtime(format!("manifest: {e}"))),
    };

    match write_bundle_zip(
        &bundle_zip,
        &runtime_root,
        full,
        &included_versions,
        cfg_loc.as_deref(),
        &working_dir,
        profile.as_deref(),
        include_indexes.then_some(index_cache_dir.as_path()),
        include_shims,
        &manifest_json,
    ) {
        Ok(()) => {
            let data = serde_json::json!({ "path": bundle_zip.to_string_lossy() });
            output::emit_ok(g, "bundle_created", data, || {
                if !g.quiet {
                    println!("{}", bundle_zip.display());
                }
            })
        }
        Err(e) => common::print_envr_error(g, e),
    }
}

fn apply(
    g: &GlobalArgs,
    file: PathBuf,
    runtime_root_override: Option<String>,
    index_cache_dir_override: Option<PathBuf>,
) -> i32 {
    if !file.is_file() {
        return common::print_envr_error(
            g,
            EnvrError::Validation(format!("bundle file not found: {}", file.display())),
        );
    }

    let runtime_root = match runtime_root_override.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty())
    {
        Some(p) => PathBuf::from(p),
        None => match common::effective_runtime_root() {
            Ok(r) => r,
            Err(e) => return common::print_envr_error(g, e),
        },
    };

    let index_cache_dir = match index_cache_dir_override {
        Some(d) => d,
        None => {
            let platform = match envr_platform::paths::current_platform_paths() {
                Ok(p) => p,
                Err(e) => return common::print_envr_error(g, e),
            };
            envr_platform::paths::index_cache_dir_from_platform(&platform)
        }
    };

    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(e) => return common::print_envr_error(g, EnvrError::from(e)),
    };

    if let Err(e) = extract_bundle_zip(&file, tmp.path()) {
        return common::print_envr_error(g, e);
    }

    let manifest_path = tmp
        .path()
        .join("envr-bundle")
        .join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).ok();

    // Copy runtimes
    let src_runtimes = tmp.path().join("envr-bundle").join("runtime_root").join("runtimes");
    if src_runtimes.is_dir() {
        let dst = runtime_root.join("runtimes");
        if let Err(e) = copy_dir_merge(&src_runtimes, &dst) {
            return common::print_envr_error(g, e);
        }
    }

    // Copy indexes
    let src_indexes = tmp
        .path()
        .join("envr-bundle")
        .join("index_cache")
        .join("indexes");
    if src_indexes.is_dir() {
        if let Err(e) = copy_dir_merge(&src_indexes, &index_cache_dir) {
            return common::print_envr_error(g, e);
        }
    }

    // Copy shims
    let src_shims = tmp.path().join("envr-bundle").join("runtime_root").join("shims");
    if src_shims.is_dir() {
        let dst = runtime_root.join("shims");
        if let Err(e) = copy_dir_merge(&src_shims, &dst) {
            return common::print_envr_error(g, e);
        }
    }

    // Restore global current pointers based on manifest (cross-platform safe).
    if let Some(m) = manifest {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&m) {
            if let Some(list) = v.get("global_current").and_then(|x| x.as_array()) {
                if let Ok(svc) = RuntimeService::with_runtime_root(runtime_root.clone()) {
                    for item in list {
                        let kind_str = item.get(0).and_then(|x| x.as_str()).unwrap_or("");
                        let ver = item.get(1).and_then(|x| x.as_str()).unwrap_or("");
                        if kind_str.is_empty() || ver.is_empty() {
                            continue;
                        }
                        if let Ok(kind) = parse_runtime_kind(kind_str) {
                            let _ = svc.set_current(kind, &RuntimeVersion(ver.to_string()));
                        }
                    }
                }
            }
        }
    }

    let data = serde_json::json!({
        "runtime_root": runtime_root.to_string_lossy(),
        "index_cache_dir": index_cache_dir.to_string_lossy(),
    });
    output::emit_ok(g, "bundle_applied", data, || {
        if !g.quiet {
            println!("{}", runtime_root.display());
        }
    })
}

fn write_bundle_zip(
    zip_path: &Path,
    runtime_root: &Path,
    full: bool,
    included_versions: &[(RuntimeKind, String)],
    project_dir: Option<&Path>,
    working_dir: &Path,
    profile: Option<&str>,
    index_cache_dir: Option<&Path>,
    include_shims: bool,
    manifest_json: &str,
) -> EnvrResult<()> {
    let file = File::create(zip_path).map_err(EnvrError::from)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions<'_, ()> = FileOptions::default();

    // Manifest
    zip.start_file("envr-bundle/manifest.json", opts)
        .map_err(|e| EnvrError::Runtime(format!("zip manifest.json: {e}")))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(EnvrError::from)?;

    // Project config files (if found)
    if let Some(dir) = project_dir {
        let base = dir.join(PROJECT_CONFIG_FILE);
        let local = dir.join(PROJECT_CONFIG_LOCAL_FILE);
        if base.is_file() {
            add_file_to_zip(&mut zip, opts, &base, "envr-bundle/project/.envr.toml")?;
        }
        if local.is_file() {
            add_file_to_zip(
                &mut zip,
                opts,
                &local,
                "envr-bundle/project/.envr.local.toml",
            )?;
        }
        // Also store where we found it.
        let loc_json = serde_json::json!({
            "project_dir": dir.to_string_lossy(),
            "working_dir": working_dir.to_string_lossy(),
            "profile": profile,
        });
        let loc_text = serde_json::to_string_pretty(&loc_json)
            .map_err(|e| EnvrError::Runtime(format!("serialize bundle location: {e}")))?;
        zip.start_file("envr-bundle/project/location.json", opts)
            .map_err(|e| EnvrError::Runtime(format!("zip location.json: {e}")))?;
        zip.write_all(loc_text.as_bytes()).map_err(EnvrError::from)?;
    }

    // Runtimes
    let runtimes_dir = runtime_root.join("runtimes");
    if full {
        if runtimes_dir.is_dir() {
            add_dir_to_zip(&mut zip, opts, &runtimes_dir, "envr-bundle/runtime_root/runtimes")?;
        }
    } else {
        // Precise: include only required version directories.
        for (kind, ver) in included_versions {
            let key = crate::commands::common::kind_label(*kind);
            let home = runtimes_dir.join(key).join("versions").join(ver);
            if home.is_dir() {
                let dest = format!("envr-bundle/runtime_root/runtimes/{key}/versions/{ver}");
                add_dir_to_zip(&mut zip, opts, &home, &dest)?;
            }
        }
    }

    // Index cache (offline indexes)
    if let Some(idx) = index_cache_dir {
        if idx.is_dir() {
            add_dir_to_zip(&mut zip, opts, idx, "envr-bundle/index_cache/indexes")?;
        }
    }

    // Shims (optional)
    if include_shims {
        let shims_dir = runtime_root.join("shims");
        if shims_dir.is_dir() {
            add_dir_to_zip(&mut zip, opts, &shims_dir, "envr-bundle/runtime_root/shims")?;
        }
    }

    zip.finish()
        .map_err(|e| EnvrError::Runtime(format!("zip finish: {e}")))?;
    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    opts: FileOptions<'_, ()>,
    src: &Path,
    dest_name: &str,
) -> EnvrResult<()> {
    let mut body = Vec::new();
    File::open(src)
        .map_err(EnvrError::from)?
        .read_to_end(&mut body)
        .map_err(EnvrError::from)?;
    zip.start_file(dest_name, opts)
        .map_err(|e| EnvrError::Runtime(format!("zip {dest_name}: {e}")))?;
    zip.write_all(&body).map_err(EnvrError::from)?;
    Ok(())
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    opts: FileOptions<'_, ()>,
    src_dir: &Path,
    dest_prefix: &str,
) -> EnvrResult<()> {
    fn rec(
        zip: &mut ZipWriter<File>,
        opts: FileOptions<'_, ()>,
        base: &Path,
        cur: &Path,
        dest_prefix: &str,
    ) -> Result<(), EnvrError> {
        for ent in fs::read_dir(cur).map_err(EnvrError::from)? {
            let ent = ent.map_err(EnvrError::from)?;
            let p = ent.path();
            let rel = p.strip_prefix(base).unwrap_or(&p);
            let rel_str = rel
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_string();
            let zip_name = format!("{dest_prefix}/{rel_str}");
            if p.is_dir() {
                rec(zip, opts, base, &p, dest_prefix)?;
            } else if p.is_file() {
                let mut body = Vec::new();
                File::open(&p)
                    .map_err(EnvrError::from)?
                    .read_to_end(&mut body)
                    .map_err(EnvrError::from)?;
                zip.start_file(zip_name, opts)
                    .map_err(|e| EnvrError::Runtime(format!("zip file: {e}")))?;
                zip.write_all(&body).map_err(EnvrError::from)?;
            }
        }
        Ok(())
    }
    rec(zip, opts, src_dir, src_dir, dest_prefix)
}

fn extract_bundle_zip(zip_path: &Path, dest: &Path) -> Result<(), EnvrError> {
    let file = File::open(zip_path).map_err(EnvrError::from)?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| EnvrError::Runtime(format!("open bundle zip: {e}")))?;
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| EnvrError::Runtime(format!("read zip entry: {e}")))?;
        let name = f.name().to_string();
        if name.contains("..") || name.starts_with('/') || name.contains('\\') {
            return Err(EnvrError::Validation(format!("unsafe zip entry: {name}")));
        }
        let out_path = dest.join(&name);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let mut out = File::create(&out_path).map_err(EnvrError::from)?;
        std::io::copy(&mut f, &mut out).map_err(EnvrError::from)?;
    }
    Ok(())
}

fn copy_dir_merge(src: &Path, dst: &Path) -> EnvrResult<()> {
    if src.is_file() {
        return Err(EnvrError::Validation(format!(
            "expected directory, got file: {}",
            src.display()
        )));
    }
    fs::create_dir_all(dst).map_err(EnvrError::from)?;
    for ent in fs::read_dir(src).map_err(EnvrError::from)? {
        let ent = ent.map_err(EnvrError::from)?;
        let p = ent.path();
        let name = ent.file_name();
        let dstp = dst.join(name);
        if p.is_dir() {
            copy_dir_merge(&p, &dstp)?;
        } else if p.is_file() {
            if let Some(parent) = dstp.parent() {
                fs::create_dir_all(parent).map_err(EnvrError::from)?;
            }
            fs::copy(&p, &dstp).map_err(EnvrError::from)?;
        }
    }
    Ok(())
}

