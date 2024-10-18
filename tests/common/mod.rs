use scopeguard::{guard, ScopeGuard};
use std::path::PathBuf;

pub fn use_cmake(name: &str) -> ScopeGuard<(), impl FnOnce(())> {
    let path = std::env::var("PATH").expect("Path not set?");
    let cmake_path = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("common")
        .join(name);
    assert!(cmake_path.exists());
    assert!(cmake_path.is_dir());

    std::env::set_var("PATH", format!("{}:{}", cmake_path.to_str().unwrap(), path));

    guard((), |_| {
        std::env::set_var("PATH", path);
    })
}

pub fn set_outdir() -> ScopeGuard<(), impl FnOnce(())> {
    std::env::set_var("OUT_DIR", std::env::temp_dir());

    guard((), |_| {
        std::env::remove_var("OUT_DIR");
    })
}
