#[test]
fn compile_test() {
    let mut config = compiletest_rs::Config {
        mode: compiletest_rs::common::Mode::Ui,
        src_base: std::path::PathBuf::from("tests/ui"),
        target_rustcflags: Some(
            "--edition=2018 \
             -Z unstable-options \
             --extern oasis_std \
             --extern serde_cbor"
                .to_string(),
        ),
        ..Default::default()
    };

    config.link_deps();
    config.clean_rmeta();

    compiletest_rs::run_tests(&config);
}
