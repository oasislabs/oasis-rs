use std::collections::{BTreeMap, BTreeSet}; // BTree for reproducability

use rustc::{hir::intravisit::Visitor, util::nodemap::FxHashMap};
use rustc_data_structures::sync::Once;
use syntax_pos::symbol::Symbol;

use crate::{
    rpc,
    visitor::{
        AnalyzedRpcCollector, DefinedTypeCollector, EventCollector, ParsedRpcCollector,
        ServiceDefFinder,
    },
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
    service_name: Once<Symbol>,
    event_indexed_fields: FxHashMap<Symbol, Vec<Symbol>>, // event_name -> field_name
    iface: Once<rpc::Interface>,
    deps: Once<BTreeMap<String, LockfileEntry>>,
}

impl Default for BuildPlugin {
    fn default() -> Self {
        Self {
            service_name: Once::new(),
            event_indexed_fields: Default::default(),
            iface: Once::new(),
            deps: Once::new(),
        }
    }
}

impl BuildPlugin {
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
        let gen_dir = compiler
            .output_dir()
            .as_ref()
            .map(std::path::PathBuf::clone)
            .unwrap_or_else(std::env::temp_dir)
            .join("mantle_generated");
        std::fs::create_dir_all(&gen_dir)
            .unwrap_or_else(|_| panic!("Could not create dir: `{}`", gen_dir.display()));

        let crate_name_query = compiler
            .crate_name()
            .expect("Could not determine crate name");
        let crate_name = crate_name_query.take();
        crate_name_query.give(crate_name.clone());

        let sess = compiler.session();
        let mut parse = compiler
            .parse()
            .expect("`after_parsing` is only called after parsing")
            .peek_mut();

        let mut service_def_finder = ServiceDefFinder::default();
        syntax::visit::walk_crate(&mut service_def_finder, &parse);

        let (services, event_indexed_fields) = service_def_finder.get();
        self.event_indexed_fields = event_indexed_fields;

        let main_service = match services.as_slice() {
            [] => return false,
            [main_service] => main_service,
            _ => {
                sess.span_err(
                    services[1].span,
                    "Multiple invocations of `mantle::service!`. Second occurrence here:",
                );
                return false;
            }
        };
        let service_name = main_service.name;
        self.service_name.set(service_name);

        let mut parsed_rpc_collector = ParsedRpcCollector::new(service_name);
        syntax::visit::walk_crate(&mut parsed_rpc_collector, &parse);

        let struct_span = parsed_rpc_collector.struct_span();

        let rpcs = match parsed_rpc_collector.into_rpcs() {
            Ok(rpcs) => rpcs,
            Err(errs) => {
                for err in errs {
                    sess.span_err(err.span(), &format!("{}", err));
                }
                return false;
            }
        };
        let (ctor, rpcs): (Vec<_>, Vec<_>) = rpcs
            .into_iter()
            .partition(|(name, _)| *name == syntax_pos::symbol::Symbol::intern("new"));
        let ctor_sig = match ctor.as_slice() {
            [] => {
                sess.span_err(
                    struct_span,
                    &format!("Missing definition of `{}::new`.", service_name),
                );
                return false;
            }
            [(_, sig)] => sig,
            _ => return true, // Multiply defined `new` function. Let the compiler catch this.
        };

        crate::dispatcher_gen::generate_and_insert(
            &mut parse,
            &gen_dir,
            &crate_name,
            service_name,
            ctor_sig,
            rpcs,
        );

        true
    }

    fn after_analysis(&mut self, compiler: &rustc_interface::interface::Compiler) -> bool {
        let sess = compiler.session();
        let mut global_ctxt = rustc_driver::abort_on_err(compiler.global_ctxt(), sess).peek_mut();

        let service_name = match self.service_name.try_get() {
            Some(service_name) => service_name,
            None => return false,
        };

        global_ctxt.enter(|tcx| {
            let mut rpc_collector = AnalyzedRpcCollector::new(tcx, *service_name);
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
                *service_name,
                imports,
                adt_defs,
                &self.event_indexed_fields,
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
