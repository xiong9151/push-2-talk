fn main() {
    tauri_build::build();

    // Feature-gated vendored SpeexDSP AEC (echo cancellation only).
    // Gated on CARGO_FEATURE_AEC so a C compile failure never breaks the
    // default build — drop `aec` from `default` in Cargo.toml to disable.
    if std::env::var("CARGO_FEATURE_AEC").is_ok() {
        let vendor = std::path::Path::new("vendor/speexdsp");
        cc::Build::new()
            .files([
                vendor.join("mdf.c"),
                vendor.join("fftwrap.c"),
                vendor.join("smallft.c"),
            ])
            .include(vendor)
            .define("HAVE_CONFIG_H", None)
            .define("FLOATING_POINT", None)
            .define("USE_SMALLFT", None)
            // MSVC C-dialect warnings are noise here; never treat as error.
            .warnings(false)
            .compile("speexdsp");
        println!("cargo:rerun-if-changed=vendor/speexdsp");
    }
}
