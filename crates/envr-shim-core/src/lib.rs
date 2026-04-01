//! Shim routing: map `node` / `python` / `java` / … to a concrete executable using project config, then global `current`.

mod resolve;

pub use resolve::{
    CoreCommand, ResolvedShim, ShimContext, normalize_invoked_basename, pick_version_home,
    resolve_core_shim,
};
