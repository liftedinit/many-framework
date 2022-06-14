use crate::error;
use crate::storage::LedgerStorage;
use many::server::module::account;
use many::server::module::account::features;
use many::server::module::account::features::{FeatureInfo, TryCreateFeature};
use many::types::ledger::{Symbol, TokenAmount};
use many::{Identity, ManyError};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct MultisigFeatureArgJson {
    pub threshold: Option<u64>,
    pub timeout_in_secs: Option<u64>,
    pub execute_automatically: Option<bool>,
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct FeatureJson {
    pub id: u32,
    pub arg: Option<serde_json::value::Value>,
}

impl FeatureJson {
    pub fn try_into_feature(&self) -> Option<features::Feature> {
        match self.id {
            features::ledger::AccountLedger::ID => Some(features::Feature::with_id(
                features::ledger::AccountLedger::ID,
            )),
            features::multisig::MultisigAccountFeature::ID => self.arg_into_multisig(),
            _ => None,
        }
    }

    fn arg_into_multisig(&self) -> Option<features::Feature> {
        self.arg.as_ref().map(|a| {
            let s = serde_json::to_string(a).expect("Invalid Feature argument.");
            let a: MultisigFeatureArgJson =
                serde_json::from_str(&s).expect("Invalid Feature argument.");

            features::multisig::MultisigAccountFeature::create(
                a.threshold,
                a.timeout_in_secs,
                a.execute_automatically,
            )
            .as_feature()
        })
    }
}

impl Eq for FeatureJson {}

impl PartialEq<Self> for FeatureJson {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd<Self> for FeatureJson {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for FeatureJson {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct AccountJson {
    pub id: Option<Identity>,
    pub subresource_id: Option<u32>,
    pub description: Option<String>,
    pub roles: BTreeMap<Identity, BTreeSet<String>>,
    pub features: BTreeSet<FeatureJson>,
}

impl AccountJson {
    pub fn create_account(&self, ledger: &mut LedgerStorage) -> Result<(), ManyError> {
        let id = ledger._add_account(
            account::Account {
                description: self.description.clone(),
                roles: self
                    .roles
                    .iter()
                    .map(|(id, roles)| {
                        (*id, {
                            roles
                                .iter()
                                .map(|s| std::str::FromStr::from_str(s))
                                .collect::<Result<BTreeSet<account::Role>, _>>()
                                .expect("Invalid role.")
                        })
                    })
                    .collect(),
                features: self
                    .features
                    .iter()
                    .map(|f| f.try_into_feature().expect("Unsupported feature."))
                    .collect(),
                disabled: None,
            },
            false,
        )?;

        if self.subresource_id.is_some()
            && id.subresource_id().is_some()
            && id.subresource_id() != self.subresource_id
        {
            return Err(error::unexpected_subresource_id(
                id.subresource_id().unwrap().to_string(),
                self.subresource_id.unwrap().to_string(),
            ));
        }
        if let Some(self_id) = self.id {
            if id != self_id {
                return Err(error::unexpected_account_id(id, self_id));
            }
        }

        Ok(())
    }
}

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct InitialStateJson {
    pub identity: Identity,
    pub initial: BTreeMap<Identity, BTreeMap<String, TokenAmount>>,
    pub symbols: BTreeMap<Identity, String>,
    pub accounts: Option<Vec<AccountJson>>,
    pub hash: Option<String>,
}

impl InitialStateJson {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(Box::new)?;
        let s = json5::from_str(&content).map_err(Box::new)?;
        Ok(s)
    }

    pub fn symbols(&self) -> BTreeMap<Identity, String> {
        self.symbols.clone()
    }

    pub fn balances(&self) -> Result<BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>>, ManyError> {
        self.initial
            .iter()
            .map(|(id, b)| {
                let mut balances = BTreeMap::new();
                for (token_name, amount) in b {
                    let symbol = self
                        .symbols
                        .iter()
                        .find_map(|(s, n)| {
                            if *s == token_name.as_str() || n == token_name {
                                Some(*s)
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            ManyError::unknown(format!("Could not resolve symbol '{}'", token_name))
                        })?;
                    balances.insert(symbol, amount.clone());
                }
                Ok((*id, balances))
            })
            .collect()
    }
}
