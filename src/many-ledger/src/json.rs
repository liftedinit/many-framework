use crate::error;
use crate::storage::LedgerStorage;
use many::server::module::account;
use many::server::module::account::features::{multisig, Feature, FeatureInfo, TryCreateFeature};
use many::types::ledger::{Symbol, TokenAmount};
use many::{Identity, ManyError};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

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
    pub fn try_into_ledger(&self) -> Option<Feature> {
        if self.id == 0 {
            Some(Feature::with_id(0))
        } else {
            None
        }
    }

    pub fn try_into_multisig(&self) -> Option<multisig::MultisigAccountFeature> {
        if self.id != multisig::MultisigAccountFeature::ID {
            return None;
        }

        self.arg.as_ref().map(|a| {
            let s = serde_json::to_string(a).expect("Invalid Feature argument.");
            let a: MultisigFeatureArgJson =
                serde_json::from_str(&s).expect("Invalid Feature argument.");
            multisig::MultisigAccountFeature::create(
                a.threshold.into(),
                a.timeout_in_secs.into(),
                a.execute_automatically,
            )
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
    pub subresource_id: Option<u32>,
    pub description: Option<String>,
    pub roles: BTreeMap<Identity, BTreeSet<String>>,
    pub features: BTreeSet<FeatureJson>,
}

impl AccountJson {
    pub fn create_account(&self, ledger: &mut LedgerStorage) -> Result<(), ManyError> {
        let id = ledger.add_account(account::Account {
            description: self.description.clone(),
            roles: self
                .roles
                .iter()
                .map(|(id, roles)| {
                    (id.clone(), {
                        roles
                            .iter()
                            .map(|s| std::str::FromStr::from_str(&s))
                            .collect::<Result<BTreeSet<account::Role>, _>>()
                            .expect("Invalid role.")
                    })
                })
                .collect(),
            features: self
                .features
                .iter()
                .map(|f| {
                    f.try_into_ledger()
                        .or_else(|| f.try_into_multisig().map(|f| f.as_feature()))
                        .expect("Unsupported feature.")
                })
                .collect(),
        })?;

        if self.subresource_id.is_some()
            && id.subresource_id().is_some()
            && id.subresource_id() != self.subresource_id
        {
            return Err(error::unexpected_subresource_id(
                id.subresource_id().unwrap().to_string(),
                self.subresource_id.unwrap().to_string(),
            ));
        }

        Ok(())
    }
}

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct InitialStateJson {
    pub identity: Identity,
    pub initial: BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>>,
    pub symbols: BTreeMap<Identity, String>,
    pub accounts: Option<Vec<AccountJson>>,
    pub hash: Option<String>,
}
