use coset::CborSerializable;
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
}
/// Setup a new identity, credential ID, public key and ledger module implementation
pub fn setup() -> Setup {
    let id = generate_random_eddsa_identity();
    let public_key = PublicKey(id.clone().key.unwrap().to_vec().unwrap().into());
    Setup {
        module_impl: LedgerModuleImpl::new(
            Some(
                serde_json::from_str(
                    &std::fs::read_to_string("../../staging/ledger_state.json").unwrap(),
                )
                .unwrap(),
            ),
            tempfile::tempdir().unwrap(),
            false,
        )
        .unwrap(),
        id: id.identity,
        cred_id: CredentialId(vec![1; 16].into()),
        public_key,
    }
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
    let Setup {
        module_impl,
        id,
        cred_id: _cred_id,
        public_key: _public_key,
    } = setup();

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

    SetupWithArgs {
        module_impl,
        id,
        args: account::CreateArgs {
            description: Some("Foobar".to_string()),
            roles,
            features,
        },
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
