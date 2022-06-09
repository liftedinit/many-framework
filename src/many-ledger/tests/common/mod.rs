use coset::CborSerializable;
use many::server::module::abci_backend::{AbciBlock, ManyAbciModuleBackend};
use many::server::module::{self};
use many::{
    server::module::{
        account::{self, features::FeatureInfo, AccountModuleBackend},
        idstore::{CredentialId, PublicKey},
        ledger::LedgerModuleBackend,
    },
    types::{
        identity::{cose::testsutils::generate_random_eddsa_identity, testing::identity},
        ledger::{AccountMultisigTransaction, TokenAmount},
    },
    Identity,
};
use many_ledger::json::InitialStateJson;
use many_ledger::module::LedgerModuleImpl;
use once_cell::sync::Lazy;
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

pub static MFX_SYMBOL: Lazy<Identity> = Lazy::new(|| {
    Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
});

pub struct Setup {
    pub module_impl: LedgerModuleImpl,
    pub id: Identity,
    pub cred_id: CredentialId,
    pub public_key: PublicKey,

    time: Option<u64>,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Setup {
    pub fn new(blockchain: bool) -> Self {
        let id = generate_random_eddsa_identity();
        let public_key = PublicKey(id.clone().key.unwrap().to_vec().unwrap().into());

        Self {
            module_impl: LedgerModuleImpl::new(
                Some(
                    InitialStateJson::read("../../staging/ledger_state.json5")
                        .expect("Could not read initial state."),
                ),
                tempfile::tempdir().unwrap(),
                blockchain,
            )
            .unwrap(),
            id: id.identity,
            cred_id: CredentialId(vec![1; 16].into()),
            public_key,
            time: Some(1_000_000),
        }
    }

    pub fn create_account_args(&mut self, account_type: AccountType) -> account::CreateArgs {
        let (roles, features) = match account_type {
            AccountType::Multisig => {
                let roles = Some(BTreeMap::from_iter([
                    (
                        identity(2),
                        BTreeSet::from_iter([account::Role::CanMultisigApprove]),
                    ),
                    (
                        identity(3),
                        BTreeSet::from_iter([account::Role::CanMultisigSubmit]),
                    ),
                ]));
                let features = account::features::FeatureSet::from_iter([
                    account::features::multisig::MultisigAccountFeature::default().as_feature(),
                ]);
                (roles, features)
            }
            AccountType::Ledger => {
                let roles = Some(BTreeMap::from_iter([(
                    identity(2),
                    BTreeSet::from_iter([account::Role::CanLedgerTransact]),
                )]));
                let features = account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]);
                (roles, features)
            }
        };

        account::CreateArgs {
            description: Some("Foobar".to_string()),
            roles,
            features,
        }
    }

    pub fn create_account(&mut self, account_type: AccountType) -> Identity {
        let args = self.create_account_args(account_type);
        self.module_impl.create(&self.id, args).unwrap().id
    }

    pub fn inc_time(&mut self, amount: u64) {
        self.time = Some(self.time.unwrap_or_default() + amount);
    }

    pub fn block<R>(&mut self, inner_f: impl FnOnce(&mut Self) -> R) -> (u64, R) {
        if let Some(t) = self.time {
            self.time = Some(t + 1000);
        }

        self.module_impl
            .begin_block(AbciBlock { time: self.time })
            .expect("Could not begin block");

        let r = inner_f(self);

        self.module_impl.end_block().expect("Could not end block");
        self.module_impl.commit().expect("Could not commit block");

        let info = ManyAbciModuleBackend::info(&self.module_impl).expect("Could not get info.");

        (info.height, r)
    }
}

pub fn setup() -> Setup {
    Setup::default()
}

pub struct SetupWithArgs {
    pub module_impl: LedgerModuleImpl,
    pub id: Identity,
    pub args: account::CreateArgs,
}

#[non_exhaustive]
pub enum AccountType {
    Multisig,
    Ledger,
}

pub fn setup_with_args(account_type: AccountType) -> SetupWithArgs {
    let mut setup = Setup::default();
    let args = setup.create_account_args(account_type);

    SetupWithArgs {
        module_impl: setup.module_impl,
        id: setup.id,
        args,
    }
}

pub struct SetupWithAccount {
    pub module_impl: LedgerModuleImpl,
    pub id: Identity,
    pub account_id: Identity,
}

pub fn setup_with_account(account_type: AccountType) -> SetupWithAccount {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args(account_type);
    let account = module_impl.create(&id, args).unwrap();
    SetupWithAccount {
        module_impl,
        id,
        account_id: account.id,
    }
}

pub struct SetupWithAccountAndTx {
    pub module_impl: LedgerModuleImpl,
    pub id: Identity,
    pub account_id: Identity,
    pub tx: AccountMultisigTransaction,
}

pub fn setup_with_account_and_tx(account_type: AccountType) -> SetupWithAccountAndTx {
    let SetupWithAccount {
        module_impl,
        id,
        account_id,
    } = setup_with_account(account_type);

    let tx = many::types::ledger::AccountMultisigTransaction::Send(
        many::server::module::ledger::SendArgs {
            from: Some(account_id),
            to: identity(3),
            symbol: Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz")
                .unwrap(),
            amount: many::types::ledger::TokenAmount::from(10u16),
        },
    );

    SetupWithAccountAndTx {
        module_impl,
        id,
        account_id,
        tx,
    }
}

pub fn verify_balance(
    module_impl: &LedgerModuleImpl,
    id: Identity,
    symbol: Identity,
    amount: TokenAmount,
) {
    let result = module_impl.balance(
        &id,
        module::ledger::BalanceArgs {
            account: Some(id),
            symbols: Some(vec![symbol].into()),
        },
    );
    assert!(result.is_ok());
    let balances = result.unwrap();
    assert_eq!(balances.balances, BTreeMap::from([(symbol, amount)]));
}
