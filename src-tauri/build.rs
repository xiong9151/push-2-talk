fn main() {
    tauri_build::build();

    // Vendored RNNoise C library (noise reduction).
    // Compiles unconditionally — the Rust FFI wrapper replaces the `nnnoiseless` crate.
    {
        let vendor = std::path::Path::new("vendor/rnnoise");
        cc::Build::new()
            .include(vendor)
            .define("HAVE_CONFIG_H", None)
            // MSVC (cl.exe) 默认按 C89 编译，pitch.c 用了 C99 变长数组(VLA)，
            // 需要显式指定 C11 或支持 VLA 的 dialect
            .flag_if_supported("/std:c11")
            .flag_if_supported("-std=c11")
            .warnings(false)
            .files([
                vendor.join("denoise.c"),
                vendor.join("rnn.c"),
                vendor.join("rnn_data.c"),
                // rnn_reader.c is not needed — we use the built-in model.
                vendor.join("pitch.c"),
                vendor.join("kiss_fft.c"),
                vendor.join("celt_lpc.c"),
            ])
            .compile("rnnoise");
        println!("cargo:rerun-if-changed=vendor/rnnoise");
    }

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
