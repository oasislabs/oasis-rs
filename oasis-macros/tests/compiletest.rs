fn run_mode(mode: &'static str) {
    let mut config = compiletest_rs::Config {
        mode: mode.parse().expect("Invalid mode."),
        src_base: std::path::PathBuf::from(format!("tests/{}", mode.replace("-", "_"))),
        target_rustcflags: Some(
            "--edition=2018 \
             -Z unstable-options \
             --cfg feature=\"test\" \
             --extern failure \
             --extern oasis_std \
             --extern oasis_test \
             --extern serde \
             --extern serde_derive \
             --extern serde_cbor"
                .to_string(),
        ),
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
