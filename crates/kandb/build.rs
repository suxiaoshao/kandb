#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    #[cfg(target_os = "windows")]
    {
        let default_icon =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon/app-icon.ico");
        let icon_path =
            std::env::var("KANDB_ICON_PATH").unwrap_or_else(|_| default_icon.display().to_string());
        let icon = std::path::Path::new(&icon_path);

        println!("cargo:rerun-if-env-changed=KANDB_ICON_PATH");
        println!("cargo:rerun-if-changed={}", icon.display());

        let mut res = winresource::WindowsResource::new();
        if let Ok(toolkit_path) = std::env::var("KANDB_RC_TOOLKIT_PATH") {
            res.set_toolkit_path(toolkit_path.as_str());
        }

        if icon.exists()
            && icon
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
        {
            res.set_icon(icon.to_str().unwrap());
        } else if !icon.exists() {
            println!(
                "cargo:warning=kanDB icon not found at '{}'; building without app icon",
                icon.display()
            );
        } else {
            println!(
                "cargo:warning=kanDB icon must be .ico for Windows resources (got '{}'); building without app icon",
                icon.display()
            );
        }

        res.set("FileDescription", "kanDB");
        res.set("ProductName", "kanDB");

        if let Err(err) = res.compile() {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
