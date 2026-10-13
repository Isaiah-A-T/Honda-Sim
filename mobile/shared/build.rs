//! The phone app shows the desktop app's own tune, parts, cooling and other panels (native/app/src/gui.rs), so
//! every feature added there shows up on phones too. Rust only allows that file's `//!` header at the top of a
//! file, so it's copied here without it and `include!`d into `gui` (src/lib.rs), where the phone layout
//! (src/gui/phone.rs) sits beside it as a child module.

fn main() {
    let src = "../../native/app/src/gui.rs";
    println!("cargo:rerun-if-changed={src}");
    let text = std::fs::read_to_string(src).expect("native/app/src/gui.rs");
    let body: String = text.lines().filter(|l| !l.trim_start().starts_with("//!")).map(|l| format!("{l}\n")).collect();
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("desktop_gui.rs");
    std::fs::write(out, body).unwrap();
}
