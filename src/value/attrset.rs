mod builder;

use std::collections::{btree_map, hash_map};
use std::rc::Rc;

pub use builder::AttrsetBuilder;

use crate::derivation::{Derivation, DerivationOutput};

use super::{NixValue, NixVar};

pub type NixAttrSetDynamic = std::collections::BTreeMap<String, NixVar>;

#[derive(Clone)]
pub enum NixAttrSet {
    Dynamic(Rc<NixAttrSetDynamic>),
    Derivation {
        selected_output: String,
        derivation: Rc<Derivation>,
    },
}

impl NixAttrSet {
    pub fn get(&self, attr: &str) -> Option<NixVar> {
        match self {
            NixAttrSet::Dynamic(set) => set.get(attr).cloned(),
            NixAttrSet::Derivation {
                selected_output: _,
                derivation,
            } => derivation.get(attr),
        }
    }

    pub fn keys(&self) -> NixAttrSetKeys<'_> {
        match self {
            NixAttrSet::Dynamic(d) => NixAttrSetKeys::Dynamic(d.keys()),
            NixAttrSet::Derivation { derivation, .. } => NixAttrSetKeys::Derivation {
                outputs: derivation.outputs.keys(),
                extra: derivation.extra_fields.keys(),
            },
        }
    }

    pub fn values(&self) -> NixAttrSetValues<'_> {
        match self {
            NixAttrSet::Dynamic(d) => NixAttrSetValues::Dynamic(d.values()),
            NixAttrSet::Derivation { derivation, .. } => NixAttrSetValues::Derivation {
                derivation,
                outputs: derivation.outputs.keys(),
                extra: derivation.extra_fields.values(),
            },
        }
    }

    pub fn iter(&self) -> NixAttrSetIter<'_> {
        match self {
            NixAttrSet::Dynamic(d) => NixAttrSetIter::Dynamic(d.iter()),
            NixAttrSet::Derivation { derivation, .. } => NixAttrSetIter::Derivation {
                derivation,
                outputs: derivation.outputs.keys(),
                map: derivation.extra_fields.iter(),
            },
        }
    }
}

pub enum NixAttrSetIter<'a> {
    Dynamic(btree_map::Iter<'a, String, NixVar>),
    Derivation {
        derivation: &'a Rc<Derivation>,
        outputs: btree_map::Keys<'a, String, DerivationOutput>,
        map: hash_map::Iter<'a, String, NixVar>,
    },
}

impl<'a> Iterator for NixAttrSetIter<'a> {
    type Item = (&'a String, NixVar);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NixAttrSetIter::Dynamic(dynamic) => dynamic.next().map(|(k, v)| (k, v.clone())),
            NixAttrSetIter::Derivation {
                outputs,
                derivation,
                map,
            } => {
                if let Some(output) = outputs.next() {
                    derivation.get(output).map(|v| (output, v))
                } else {
                    map.next().map(|(k, v)| (k, v.clone()))
                }
            }
        }
    }
}

impl<'a> IntoIterator for &'a NixAttrSet {
    type Item = (String, NixVar);

    type IntoIter = NixAttrSetIntoIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            NixAttrSet::Dynamic(d) => NixAttrSetIntoIter::Dynamic(d.iter()),
            NixAttrSet::Derivation { derivation, .. } => NixAttrSetIntoIter::Derivation {
                outputs: derivation.outputs.keys(),
                map: derivation.extra_fields.iter(),
                derivation,
            },
        }
    }
}

pub enum NixAttrSetIntoIter<'a> {
    Dynamic(btree_map::Iter<'a, String, NixVar>),
    Derivation {
        derivation: &'a Rc<Derivation>,
        outputs: btree_map::Keys<'a, String, DerivationOutput>,
        map: hash_map::Iter<'a, String, NixVar>,
    },
}

impl<'a> Iterator for NixAttrSetIntoIter<'a> {
    type Item = (String, NixVar);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NixAttrSetIntoIter::Dynamic(dynamic) => {
                dynamic.next().map(|(k, v)| (k.clone(), v.clone()))
            }
            NixAttrSetIntoIter::Derivation {
                outputs,
                derivation,
                map,
            } => {
                if let Some(output) = outputs.next() {
                    derivation.get(output).map(|v| (output.clone(), v))
                } else {
                    map.next().map(|(k, v)| (k.clone(), v.clone()))
                }
            }
        }
    }
}

pub enum NixAttrSetKeys<'a> {
    Dynamic(btree_map::Keys<'a, String, NixVar>),
    Derivation {
        outputs: btree_map::Keys<'a, String, DerivationOutput>,
        extra: hash_map::Keys<'a, String, NixVar>,
    },
}

impl<'a> Iterator for NixAttrSetKeys<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NixAttrSetKeys::Dynamic(d) => d.next(),
            NixAttrSetKeys::Derivation { outputs, extra } => {
                outputs.next().or_else(|| extra.next())
            }
        }
    }
}

pub enum NixAttrSetValues<'a> {
    Dynamic(btree_map::Values<'a, String, NixVar>),
    Derivation {
        derivation: &'a Rc<Derivation>,
        outputs: btree_map::Keys<'a, String, DerivationOutput>,
        extra: hash_map::Values<'a, String, NixVar>,
    },
}

impl<'a> Iterator for NixAttrSetValues<'a> {
    type Item = NixVar;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NixAttrSetValues::Dynamic(d) => d.next().cloned(),
            NixAttrSetValues::Derivation {
                outputs,
                extra,
                derivation,
            } => outputs
                .next()
                .map(|output| {
                    NixValue::AttrSet(NixAttrSet::Derivation {
                        selected_output: output.to_owned(),
                        derivation: derivation.clone(),
                    })
                    .wrap_var()
                })
                .or_else(|| extra.next().cloned()),
        }
    }
}

impl Into<NixAttrSet> for NixAttrSetDynamic {
    fn into(self) -> NixAttrSet {
        NixAttrSet::Dynamic(self.into())
    }
}

impl Into<NixValue> for NixAttrSetDynamic {
    fn into(self) -> NixValue {
        NixValue::AttrSet(self.into())
    }
}
