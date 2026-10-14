//! Windows only: the suite's icon and version information in craft-suite.exe, so it shows in
//! Explorer, the taskbar, the Start menu and Alt-Tab. On other systems this does nothing; a
//! missing resource compiler is a warning unless CRAFT_SUITE_REQUIRE_WINRES is set (CI).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../assets/craft-suite.ico");
    println!("cargo:rerun-if-env-changed=CRAFT_SUITE_REQUIRE_WINRES");
    println!("cargo:rerun-if-env-changed=CRAFT_SUITE_RELEASES_URL");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../assets/craft-suite.ico")
        .set("ProductName", "Craft Suite")
        .set("FileDescription", "Craft Suite: WordCraft, DeckCraft and GridCraft in one window")
        .set("CompanyName", "Md. Hafijur Rahman")
        .set("Comments", "Created by Md. Hafijur Rahman. Source: WordCraft, DeckCraft and GridCraft by ArtCraft.")
        .set("LegalCopyright", "Craft Suite by Md. Hafijur Rahman; apps by ArtCraft. MIT OR Apache-2.0")
        .set("OriginalFilename", "craft-suite.exe")
        .set("InternalName", "craft-suite");
    if let Err(e) = res.compile() {
        if std::env::var_os("CRAFT_SUITE_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=craft-suite.exe built without icon/version resources: {e}");
    }
}
