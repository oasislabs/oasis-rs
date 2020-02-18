use std::path::PathBuf;

macro_rules! deps_dir {
    () => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug/deps")
    };
}

fn find_deps(names: &[&str]) -> Vec<PathBuf> {
    let libs = std::fs::read_dir(deps_dir!())
        .unwrap()
        .filter_map(|de| {
            let de = de.unwrap();
            let p = de.path();
            let fname = p.file_name().unwrap().to_str().unwrap();
            match fname.split('-').collect::<Vec<_>>().as_slice() {
                [lib_name, disambiguator] if disambiguator.ends_with(".rlib") => {
                    Some(((*lib_name).to_string(), de.path()))
                }
                _ => None,
            }
        })
        .collect::<std::collections::HashMap<String, PathBuf>>();
    names
        .iter()
        .map(|name| {
            libs.get(&format!("lib{}", name.replace("-", "_")))
                .cloned()
                .unwrap_or_else(|| PathBuf::from(name))
        })
        .collect()
}

fn run_mode(mode: &'static str) {
    let deps = &["borsh", "oasis_std", "oasis_macros", "oasis_types", "tests"];
    let externs = deps
        .iter()
        .zip(find_deps(deps).iter())
        .map(|(dep, p)| format!("--extern {}={}", dep, p.display()))
        .collect::<Vec<_>>()
        .join(" ");

    let rustflags = format!(
        concat!(
            "--edition=2018 --cfg feature=\"oasis-build-compiletest\" --crate-type dylib {} -L",
            deps_dir!()
        ),
        externs
    );
    let config = compiletest_rs::Config {
        mode: mode.parse().expect("Invalid mode."),
        src_base: PathBuf::from(format!("tests/{}", mode.replace("-", "_"))),
        target_rustcflags: Some(rustflags),
        rustc_path: PathBuf::from("oasis-build"),
        ..Default::default()
    }
    .tempdir();

    compiletest_rs::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("ui");
}
