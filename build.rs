//! The self-contained MinGW linker shipped with Rust's `windows-gnu` toolchain lacks `libimm32.a`,
//! which miniquad links for IME support. Generate the import library with the bundled `rust-lld`
//! so the project builds without a separate MinGW or Visual Studio install.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const IMM32_EXPORTS: &[&str] = &[
    "ImmAssociateContext",
    "ImmAssociateContextEx",
    "ImmCreateContext",
    "ImmGetCompositionStringW",
    "ImmGetContext",
    "ImmGetOpenStatus",
    "ImmReleaseContext",
    "ImmSetCandidateWindow",
    "ImmSetCompositionWindow",
    "ImmSetOpenStatus",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "gnu" {
        return;
    }

    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let Some(sysroot) = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
    else {
        return;
    };
    let host = env::var("HOST").unwrap_or_default();
    let lld = PathBuf::from(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin/rust-lld.exe");
    if !lld.exists() {
        // Fall back to whatever MinGW installation provides libimm32.a.
        return;
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let def = out.join("imm32.def");
    fs::write(&def, format!("LIBRARY imm32.dll\nEXPORTS\n{}\n", IMM32_EXPORTS.join("\n"))).unwrap();

    let status = Command::new(&lld)
        .args(["-flavor", "link", "-lib", "-machine:x64"])
        .arg(format!("-def:{}", def.display()))
        .arg(format!("-out:{}", out.join("libimm32.a").display()))
        .status();
    if matches!(status, Ok(s) if s.success()) {
        println!("cargo:rustc-link-search=native={}", out.display());
    }
}
