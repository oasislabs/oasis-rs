use crate::idl::*;

pub fn walk_type_def<V: IdlVisitor>(visitor: &mut V, def: &TypeDef) {
    use TypeDef::*;
    match def {
        Struct { fields, .. } => {
            fields.iter().for_each(|f| visitor.visit_type(&f.ty));
        }
        Enum { variants, .. } => {
            variants.iter().for_each(|v| match &v.fields {
                Some(EnumFields::Named(fields)) => {
                    fields.iter().for_each(|f| visitor.visit_type(&f.ty));
                }
                Some(EnumFields::Tuple(tys)) => {
                    tys.iter().for_each(|ty| visitor.visit_type(ty));
                }
                _ => {}
            });
        }
        Event { fields, .. } => {
            fields.iter().for_each(|f| visitor.visit_type(&f.ty));
        }
    }
}

pub fn walk_type<V: IdlVisitor>(visitor: &mut V, ty: &Type) {
    use Type::*;
    match ty {
        Tuple(tys) => {
            tys.iter().for_each(|ty| visitor.visit_type(&ty));
        }
        Array(ty, _) | List(ty) | Set(ty) | Optional(ty) => {
            visitor.visit_type(&ty);
        }
        Map(ty0, ty1) | Result(ty0, ty1) => {
            visitor.visit_type(&ty0);
            visitor.visit_type(&ty1);
        }
        _ => {}
    }
}

pub trait IdlVisitor: Sized {
    fn visit_type_def(&mut self, def: &TypeDef) {
        walk_type_def(self, def);
    }

    fn visit_type(&mut self, ty: &Type) {
        walk_type(self, ty);
    }
}
