use std::collections::{BTreeMap, BTreeSet}; // BTree for reproducability

use rustc::hir::intravisit::Visitor;
use rustc_data_structures::sync::Once;

use crate::{
    rpc,
    visitor::{AnalyzedRpcCollector, DefinedTypeCollector, EventCollector, ServiceDefFinder},
};

#[derive(Deserialize)]
struct Lockfile {
    package: Vec<LockfileEntry>,
}

#[derive(Deserialize)]
struct LockfileEntry {
    name: String,
    version: String,
}

pub struct BuildPlugin {
    service_def_finder: ServiceDefFinder,
    iface: Once<rpc::Interface>,
    deps: Once<BTreeMap<String, LockfileEntry>>,
}

impl BuildPlugin {
    pub fn new() -> Self {
        Self {
            service_def_finder: ServiceDefFinder::default(),
            iface: Once::new(),
            deps: Once::new(),
        }
    }

    /// Returns the generated interface.
    /// Only valid after rustc callback has been executed. Panics if called before.
    pub fn try_get(&self) -> Option<&rpc::Interface> {
        self.iface.try_get()
    }

    /// Returns the (name, version) of a dependency.
    fn crate_version<S: AsRef<str>>(&self, crate_name: S) -> String {
        self.deps.init_locking(Self::load_deps);
        let deps = self.deps.get();
        deps.get(crate_name.as_ref())
            .map(|pkg| pkg.version.to_string())
            .unwrap_or_else(|| "*".to_string())
    }

    fn load_deps() -> BTreeMap<String, LockfileEntry> {
        let mf_dir = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` not set"),
        );

        let lockfile_path = mf_dir
            .ancestors()
            .map(|p| p.join("Cargo.lock"))
            .skip_while(|p| !p.is_file())
            .nth(0);

        if let Some(lockfile_path) = lockfile_path {
            let lockfile: Lockfile = toml::from_str(
                &std::fs::read_to_string(lockfile_path).expect("Cargo.lock should exist"),
            )
            .expect("Cargo.lock should exist and be readable");

            lockfile
                .package
                .into_iter()
                .map(|pkg| (pkg.name.replace("-", "_"), pkg))
                .collect()
        } else {
            BTreeMap::default()
        }
    }
}

impl rustc_driver::Callbacks for BuildPlugin {
    fn after_parsing(&mut self, compiler: &rustc_interface::interface::Compiler) -> bool {
        let sess = compiler.session();
        let mut parse = compiler
            .parse()
            .expect("`after_parsing` is only called after parsing")
            .peek_mut();
        syntax::visit::walk_crate(&mut self.service_def_finder, &parse);
        let service_name = self.service_def_finder.service_name();
        if let Some(mut parsed_rpc_collector) = self.service_def_finder.parsed_rpc_collector() {
            syntax::visit::walk_crate(&mut parsed_rpc_collector, &parse);
            let impl_span = parsed_rpc_collector.impl_span();
            let rpcs = match parsed_rpc_collector.into_rpcs() {
                Ok(rpcs) => rpcs,
                Err(errs) => {
                    for err in errs {
                        sess.span_err(err.span(), &format!("{}", err));
                    }
                    return false;
                }
            };
            let (deploy_export, dispatch_tree) = crate::dispatcher_gen::generate(rpcs);
            let deploy_export = match deploy_export {
                Some(deploy_export) => deploy_export,
                None => {
                    sess.span_err(
                        impl_span,
                        &format!("Missing definition of `{}::new`.", service_name.unwrap()),
                    );
                    return false;
                }
            };
            parse.module.items.push(dbg!(deploy_export));
        }
        true
    }

    fn after_analysis(&mut self, compiler: &rustc_interface::interface::Compiler) -> bool {
        let sess = compiler.session();
        let mut global_ctxt = rustc_driver::abort_on_err(compiler.global_ctxt(), sess).peek_mut();

        let service_name = match self.service_def_finder.service_name() {
            Some(service_name) => service_name,
            None => return true, // `#[service]` will complain about missing `derive(Service)`.
        };

        global_ctxt.enter(|tcx| {
            let mut rpc_collector = AnalyzedRpcCollector::new(tcx, service_name);
            tcx.hir().krate().visit_all_item_likes(&mut rpc_collector);

            let defined_types = rpc_collector.rpcs().iter().flat_map(|(_, decl)| {
                let mut def_ty_collector = DefinedTypeCollector::new(tcx);
                def_ty_collector.visit_fn_decl(decl);
                def_ty_collector.adt_defs()
            });

            let mut event_collector = EventCollector::new(tcx);
            tcx.hir()
                .krate()
                .visit_all_item_likes(&mut event_collector.as_deep_visitor());

            let all_adt_defs = defined_types.map(|def| (def, false /* is_import */)).chain(
                event_collector
                    .adt_defs()
                    .into_iter()
                    .map(|def| (def, true)),
            );

            let mut imports = BTreeSet::default();
            let mut adt_defs = BTreeSet::default();
            for (def, is_event) in all_adt_defs.into_iter() {
                if def.did.is_local() {
                    adt_defs.insert((def, is_event));
                } else {
                    let crate_name = tcx.original_crate_name(def.did.krate);
                    imports.insert((crate_name, self.crate_version(crate_name.as_str())));
                }
            }

            let iface = match rpc::Interface::convert(
                tcx,
                service_name,
                imports,
                adt_defs,
                self.service_def_finder.event_indices(),
                rpc_collector.rpcs(),
            ) {
                Ok(iface) => iface,
                Err(errs) => {
                    for err in errs {
                        sess.span_err(err.span(), &format!("{}", err));
                    }
                    return;
                }
            };

            self.iface.set(iface);
        });

        true
    }
}
