use envr_domain::runtime::RuntimeProvider;
use std::path::PathBuf;

fn attach_runtime_root<T>(
    runtime_root: &Option<PathBuf>,
    new: impl FnOnce() -> T,
    with_root: impl FnOnce(T, PathBuf) -> T,
) -> T {
    match runtime_root {
        None => new(),
        Some(r) => with_root(new(), r.clone()),
    }
}

pub fn default_provider_boxes(runtime_root: Option<PathBuf>) -> Vec<Box<dyn RuntimeProvider>> {
    let mut providers: Vec<Box<dyn RuntimeProvider>> = Vec::new();

    #[cfg(feature = "runtime-node")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_node::NodeRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-python")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_python::PythonRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-java")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_java::JavaRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-kotlin")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_kotlin::KotlinRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-scala")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_scala::ScalaRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-clojure")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_clojure::ClojureRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-groovy")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_groovy::GroovyRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-terraform")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_terraform::TerraformRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-v")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_v::VRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-odin")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_odin::OdinRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-purescript")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_purescript::PurescriptRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-elm")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_elm::ElmRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-gleam")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_gleam::GleamRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-racket")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_racket::RacketRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-dart")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_dart::DartRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-flutter")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_flutter::FlutterRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-go")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_go::GoRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-rust")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_rust::RustRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-ruby")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_ruby::RubyRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-elixir")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_elixir::ElixirRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-erlang")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_erlang::ErlangRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-php")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_php::PhpRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-deno")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_deno::DenoRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-bun")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_bun::BunRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-dotnet")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_dotnet::DotnetRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-zig")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_zig::ZigRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-julia")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_julia::JuliaRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-janet")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_janet::JanetRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-c3")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_c3::C3RuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-babashka")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_babashka::BabashkaRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-sbcl")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_sbcl::SbclRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-haxe")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_haxe::HaxeRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-lua")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_lua::LuaRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-nim")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_nim::NimRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-crystal")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_crystal::CrystalRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-perl")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_perl::PerlRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-unison")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_unison::UnisonRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-rlang")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_rlang::RlangRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));
    #[cfg(feature = "runtime-luau")]
    providers.push(Box::new(attach_runtime_root(
        &runtime_root,
        envr_runtime_luau::LuauRuntimeProvider::new,
        |p, r| p.with_runtime_root(r),
    )));

    providers
}
