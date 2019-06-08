use std::path::PathBuf;

fn run_mode(mode: &'static str) {
    let mut config = compiletest_rs::Config {
        mode: mode.parse().expect("Invalid mode."),
        src_base: PathBuf::from(format!("tests/{}", mode.replace("-", "_"))),
        target_rustcflags: Some(
            "--edition=2018 \
             -Z unstable-options \
             --cfg feature=\"mantle-build-test\" \
             --extern mantle \
             --extern mantle_test \
             --extern ../target/debug/deps/libserde-2ff96db1b7ad1ae8.rlib \
             --extern serde_derive \
             --extern serde_cbor"
                .to_string(),
        ),
        rustc_path: PathBuf::from("../target/debug/mantle-build"),
        ..Default::default()
    };

    config.link_deps();
    config.clean_rmeta();

    compiletest_rs::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("run-pass");
    run_mode("ui");
}
