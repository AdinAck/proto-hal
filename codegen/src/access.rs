use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use darling::{util::SpannedValue, FromMeta};
use syn::{Ident, Meta};
use tiva::Validator;

use crate::{
    structures::{entitlement, schema::Schema},
    utils::{get_schema_from_set, PathArray},
};

#[derive(Debug, Clone, Default, FromMeta)]
#[darling(default)]
pub struct AccessArgs {
    pub schema: Option<Ident>,
    pub entitlements: PathArray,
    pub effect: Option<Meta>,
}

#[derive(Debug, Clone)]
pub struct Read {
    pub args: SpannedValue<AccessArgs>,
    pub schema: Schema,
    pub entitlements: HashSet<entitlement::Unrefined>,
    pub effects: (),
}

#[derive(Debug, Clone)]
pub struct Write {
    pub args: SpannedValue<AccessArgs>,
    pub schema: Schema,
    pub entitlements: HashSet<entitlement::Unrefined>,
    pub effects: (),
}

#[derive(Debug, Clone)]
pub struct AccessSpec<R, W> {
    read: R,
    write: W,
}

#[derive(Debug, Clone)]
pub struct Access<R, W> {
    spec: AccessSpec<R, W>,
}

impl<R, W> Deref for Access<R, W> {
    type Target = AccessSpec<R, W>;

    fn deref(&self) -> &Self::Target {
        &self.spec
    }
}

pub type Unrefined = Access<Option<Read>, Option<Write>>;

impl<W> Access<Option<Read>, W> {
    pub fn refine_read(self) -> Result<Access<Read, W>, Self> {
        let spec = self.spec;

        let Some(read) = spec.read else {
            Err(Self { spec })?
        };

        Ok(Access {
            spec: AccessSpec {
                read,
                write: spec.write,
            },
        })
    }
}
impl<R> Access<R, Option<Write>> {
    pub fn refine_write(self) -> Result<Access<R, Write>, Self> {
        let spec = self.spec;

        let Some(write) = spec.write else {
            Err(Self { spec })?
        };

        Ok(Access {
            spec: AccessSpec {
                read: spec.read,
                write,
            },
        })
    }
}

impl AccessSpec<Option<Read>, Option<Write>> {
    pub fn parse(
        read_args: Option<&SpannedValue<AccessArgs>>,
        write_args: Option<&SpannedValue<AccessArgs>>,
        implicit_schema: Option<Schema>,
        schemas: &HashMap<Ident, Schema>,
    ) -> syn::Result<Self> {
        let get_access_entitlements = |args: &AccessArgs| {
            let mut access_entitlements = HashSet::new();

            for path in args.entitlements.elems.iter().cloned() {
                if !access_entitlements.insert(entitlement::Unrefined::from_path(&path)?) {
                    Err(syn::Error::new_spanned(path, "entitlement exists already"))?
                }
            }

            Ok::<_, syn::Error>(access_entitlements)
        };

        let get_schema = |args: &SpannedValue<AccessArgs>| {
            if let Some(ident) = &args.schema {
                if implicit_schema.is_some() {
                    Err(syn::Error::new(
                        args.span(),
                        "cannot import schema for field which has an implicit schema",
                    ))?
                }

                get_schema_from_set(ident, schemas)
            } else {
                implicit_schema.clone().ok_or(syn::Error::new(
                    args.span(),
                    "a schema must be imported or defined in the field body",
                ))
            }
        };

        let read = if let Some(read_args) = read_args {
            Some(Read {
                args: read_args.clone(),
                schema: get_schema(read_args)?,
                entitlements: get_access_entitlements(read_args)?,
                effects: (),
            })
        } else {
            None
        };

        let write = if let Some(write_args) = write_args {
            Some(Write {
                args: write_args.clone(),
                schema: get_schema(write_args)?,
                entitlements: get_access_entitlements(write_args)?,
                effects: (),
            })
        } else {
            None
        };

        Ok(Self { read, write })
    }

    pub fn is_read(&self) -> bool {
        self.read.is_some()
    }

    pub fn is_write(&self) -> bool {
        self.write.is_some()
    }
}

impl Validator<AccessSpec<Option<Read>, Option<Write>>> for Access<Option<Read>, Option<Write>> {
    type Error = syn::Error;

    fn validate(spec: AccessSpec<Option<Read>, Option<Write>>) -> Result<Self, Self::Error> {
        if let (Some(read), Some(write)) = (&spec.read, &spec.write) {
            if read.schema.width != write.schema.width {
                Err(syn::Error::new(
                    read.args.span().join(write.args.span()).unwrap(),
                    "read and write schemas must be of equal width",
                ))?
            }
        }

        Ok(Self { spec })
    }
}
