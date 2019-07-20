use std::{io::Write, path::Path};

use syntax::{
    ast::{Arg, Block, Crate, Item, ItemKind, MethodSig, StmtKind},
    print::pprust,
    ptr::P,
};
use syntax_pos::symbol::Symbol;

use crate::{
    visitor::syntax::{ParsedRpc, ParsedRpcKind},
    BuildContext,
};

use super::ServiceDefinition;

pub fn insert(build_ctx: &BuildContext, krate: &mut Crate, service_def: &ServiceDefinition) {}
