use std::path::PathBuf;

fn run_mode(mode: &'static str) {
    // Our serde conflicts with the serde in the sysroot.
    let libserde_path =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug/deps"))
            .unwrap()
            .find_map(|maybe_dirent| {
                maybe_dirent.ok().and_then(|dirent| {
                    if dirent
                        .file_name()
                        .into_string()
                        .unwrap()
                        .starts_with("libserde")
                    {
                        Some(dirent.path())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| PathBuf::from("serde"));

    let mut config = compiletest_rs::Config {
        mode: mode.parse().expect("Invalid mode."),
        src_base: PathBuf::from(format!("tests/{}", mode.replace("-", "_"))),
        target_rustcflags: Some(format!(
            "--edition=2018 \
             -Z unstable-options \
             --cfg feature=\"mantle-build-test\" \
             --extern mantle \
             --extern mantle_test \
             --extern {} \
             --extern serde_derive \
             --extern serde_cbor",
            libserde_path.display()
        )),
        rustc_path: PathBuf::from("mantle-build"),
        ..Default::default()
    }
    .tempdir();

    config.link_deps();
    config.clean_rmeta();

    compiletest_rs::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("ui");
}
