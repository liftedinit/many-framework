use coset::CborSerializable;
use many::message::ResponseMessage;
use many::server::module::abci_backend::{AbciBlock, ManyAbciModuleBackend};
use many::server::module::account::features::multisig::{
    AccountMultisigModuleBackend, ExecuteArgs, InfoReturn,
};
use many::server::module::ledger::{BalanceArgs, LedgerCommandsModuleBackend};
use many::server::module::{self};
use many::types::events;
use many::types::ledger::Symbol;
use many::{
    server::module::{
        account::{self, features::FeatureInfo, AccountModuleBackend},
        idstore::{CredentialId, PublicKey},
        ledger::LedgerModuleBackend,
    },
    types::{
        identity::{cose::testsutils::generate_random_eddsa_identity, testing::identity},
        ledger::TokenAmount,
    },
    Identity, ManyError,
};
use many_ledger::json::InitialStateJson;
use many_ledger::module::LedgerModuleImpl;
use minicbor::bytes::ByteVec;
use once_cell::sync::Lazy;
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

pub static MFX_SYMBOL: Lazy<Identity> = Lazy::new(|| {
    Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
});

pub fn assert_many_err<I: std::fmt::Debug + PartialEq>(r: Result<I, ManyError>, err: ManyError) {
    assert_eq!(r, Err(err));
}

fn create_account_args(account_type: AccountType) -> account::CreateArgs {
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

    pub fn set_balance(&mut self, id: Identity, amount: u64, symbol: Symbol) {
        self.module_impl
            .set_balance_only_for_testing(id, amount, symbol);
    }

    pub fn balance(&self, account: Identity, symbol: Symbol) -> Result<TokenAmount, ManyError> {
        Ok(self
            .module_impl
            .balance(
                &account,
                BalanceArgs {
                    account: None,
                    symbols: Some(vec![symbol].into()),
                },
            )?
            .balances
            .get(&symbol)
            .cloned()
            .unwrap_or_default())
    }

    pub fn balance_(&self, account: Identity) -> TokenAmount {
        self.balance(account, *MFX_SYMBOL).unwrap()
    }

    pub fn send(
        &mut self,
        from: Identity,
        to: Identity,
        amount: impl Into<TokenAmount>,
        symbol: Symbol,
    ) -> Result<(), ManyError> {
        self.send_as(from, from, to, amount, symbol)
    }

    pub fn send_as(
        &mut self,
        sender: Identity,
        from: Identity,
        to: Identity,
        amount: impl Into<TokenAmount>,
        symbol: Symbol,
    ) -> Result<(), ManyError> {
        self.module_impl.send(
            &sender,
            module::ledger::SendArgs {
                from: Some(from),
                to,
                amount: amount.into(),
                symbol,
            },
        )?;
        Ok(())
    }

    pub fn send_(&mut self, from: Identity, to: Identity, amount: impl Into<TokenAmount>) {
        self.send(from, to, amount, *MFX_SYMBOL)
            .expect("Could not send tokens")
    }

    pub fn create_account_as(
        &mut self,
        id: Identity,
        account_type: AccountType,
    ) -> Result<Identity, ManyError> {
        let args = create_account_args(account_type);
        self.module_impl.create(&id, args).map(|x| x.id)
    }

    pub fn create_account(&mut self, account_type: AccountType) -> Result<Identity, ManyError> {
        self.create_account_as(self.id, account_type)
    }

    pub fn create_account_(&mut self, account_type: AccountType) -> Identity {
        self.create_account(account_type).unwrap()
    }

    pub fn create_account_as_(&mut self, id: Identity, account_type: AccountType) -> Identity {
        self.create_account_as(id, account_type).unwrap()
    }

    pub fn inc_time(&mut self, amount: u64) {
        self.time = Some(self.time.unwrap_or_default() + amount);
    }

    /// Execute a block begin+inner_f+end+commit.
    /// See https://docs.tendermint.com/master/spec/abci/abci.html#block-execution
    pub fn block<R>(&mut self, inner_f: impl FnOnce(&mut Self) -> R) -> (u64, R) {
        if let Some(t) = self.time {
            self.time = Some(t + 1);
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

    pub fn add_roles_as(
        &mut self,
        id: Identity,
        account_id: Identity,
        roles: BTreeMap<Identity, BTreeSet<account::Role>>,
    ) {
        self.module_impl
            .add_roles(
                &id,
                account::AddRolesArgs {
                    account: account_id,
                    roles,
                },
            )
            .unwrap();
    }

    pub fn add_roles(
        &mut self,
        account_id: Identity,
        roles: BTreeMap<Identity, BTreeSet<account::Role>>,
    ) {
        self.add_roles_as(self.id, account_id, roles);
    }

    /// Create a multisig transaction using the owner ID.
    pub fn create_multisig(
        &mut self,
        account_id: Identity,
        event: events::AccountMultisigTransaction,
    ) -> Result<ByteVec, ManyError> {
        self.create_multisig_as(self.id, account_id, event)
    }

    pub fn create_multisig_as(
        &mut self,
        id: Identity,
        account_id: Identity,
        event: events::AccountMultisigTransaction,
    ) -> Result<ByteVec, ManyError> {
        self.module_impl
            .multisig_submit_transaction(
                &id,
                account::features::multisig::SubmitTransactionArgs {
                    account: account_id,
                    memo: Some("Foo".to_string()),
                    transaction: Box::new(event),
                    threshold: None,
                    timeout_in_secs: None,
                    execute_automatically: None,
                    data: None,
                },
            )
            .map(|x| x.token)
    }

    pub fn create_multisig_(
        &mut self,
        account_id: Identity,
        transaction: events::AccountMultisigTransaction,
    ) -> ByteVec {
        self.create_multisig(account_id, transaction).unwrap()
    }

    /// Send some tokens as a multisig transaction.
    pub fn multisig_send(
        &mut self,
        account_id: Identity,
        to: Identity,
        amount: impl Into<TokenAmount>,
        symbol: Identity,
    ) -> Result<ByteVec, ManyError> {
        self.create_multisig(
            account_id,
            events::AccountMultisigTransaction::Send(module::ledger::SendArgs {
                from: Some(account_id),
                to,
                symbol,
                amount: amount.into(),
            }),
        )
    }

    pub fn multisig_send_(
        &mut self,
        account_id: Identity,
        to: Identity,
        amount: impl Into<TokenAmount>,
    ) -> ByteVec {
        self.multisig_send(account_id, to, amount, *MFX_SYMBOL)
            .unwrap()
    }

    /// Approve a multisig transaction.
    pub fn multisig_approve(&mut self, id: Identity, token: &ByteVec) -> Result<(), ManyError> {
        let token = token.clone();
        self.module_impl
            .multisig_approve(&id, account::features::multisig::ApproveArgs { token })?;
        Ok(())
    }

    pub fn multisig_approve_(&mut self, id: Identity, token: &ByteVec) {
        self.multisig_approve(id, token)
            .expect("Could not approve multisig")
    }

    pub fn multisig_execute_as(
        &mut self,
        id: Identity,
        token: &ByteVec,
    ) -> Result<ResponseMessage, ManyError> {
        self.module_impl.multisig_execute(
            &id,
            ExecuteArgs {
                token: token.clone(),
            },
        )
    }

    /// Execute the transaction.
    pub fn multisig_execute(&mut self, token: &ByteVec) -> Result<ResponseMessage, ManyError> {
        self.multisig_execute_as(self.id, token)
    }

    pub fn multisig_execute_as_(&mut self, id: Identity, token: &ByteVec) -> ResponseMessage {
        self.multisig_execute_as(id, token).unwrap()
    }

    pub fn multisig_execute_(&mut self, token: &ByteVec) -> ResponseMessage {
        self.multisig_execute(token)
            .expect("Could not execute multisig")
    }

    pub fn assert_multisig_info(&self, token: &ByteVec, assert_f: impl FnOnce(InfoReturn)) {
        let token = token.clone();
        assert_f(
            self.module_impl
                .multisig_info(&self.id, account::features::multisig::InfoArgs { token })
                .expect("Could not find multisig info"),
        );
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

#[derive(Clone)]
#[non_exhaustive]
pub enum AccountType {
    Multisig,
    Ledger,
}

pub fn setup_with_args(account_type: AccountType) -> SetupWithArgs {
    let setup = Setup::default();
    let args = create_account_args(account_type);

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

#[derive(Debug)]
pub struct SetupWithAccountAndTx {
    pub module_impl: LedgerModuleImpl,
    pub id: Identity,
    pub account_id: Identity,
    pub tx: events::AccountMultisigTransaction,
}

fn event_from_kind(
    event: events::EventKind,
    module_impl: &mut LedgerModuleImpl,
    id: Identity,
    account_id: Identity,
    account_type: AccountType,
) -> events::AccountMultisigTransaction {
    let send_tx = events::AccountMultisigTransaction::Send(module::ledger::SendArgs {
        from: Some(account_id),
        to: identity(3),
        symbol: *MFX_SYMBOL,
        amount: many::types::ledger::TokenAmount::from(10u16),
    });

    match event {
        events::EventKind::Send => send_tx,
        events::EventKind::AccountCreate => {
            events::AccountMultisigTransaction::AccountCreate(create_account_args(account_type))
        }
        events::EventKind::AccountDisable => {
            events::AccountMultisigTransaction::AccountDisable(account::DisableArgs {
                account: account_id,
            })
        }
        events::EventKind::AccountSetDescription => {
            events::AccountMultisigTransaction::AccountSetDescription(account::SetDescriptionArgs {
                account: account_id,
                description: "New description".to_string(),
            })
        }
        events::EventKind::AccountAddRoles => {
            events::AccountMultisigTransaction::AccountAddRoles(account::AddRolesArgs {
                account: account_id,
                roles: BTreeMap::from([(
                    identity(100),
                    BTreeSet::from([account::Role::CanMultisigSubmit]),
                )]),
            })
        }
        events::EventKind::AccountRemoveRoles => {
            events::AccountMultisigTransaction::AccountRemoveRoles(account::RemoveRolesArgs {
                account: account_id,
                roles: BTreeMap::from([(
                    identity(3),
                    BTreeSet::from([account::Role::CanMultisigSubmit]),
                )]),
            })
        }
        events::EventKind::AccountAddFeatures => {
            events::AccountMultisigTransaction::AccountAddFeatures(account::AddFeaturesArgs {
                account: account_id,
                roles: Some(BTreeMap::from([(
                    identity(200),
                    BTreeSet::from([account::Role::CanLedgerTransact]),
                )])),
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            })
        }
        events::EventKind::AccountMultisigSubmit => {
            events::AccountMultisigTransaction::AccountMultisigSubmit(
                account::features::multisig::SubmitTransactionArgs {
                    account: account_id,
                    memo: Some("A memo".to_string()),
                    transaction: Box::new(send_tx),
                    threshold: None,
                    timeout_in_secs: None,
                    execute_automatically: Some(false),
                    data: None,
                },
            )
        }
        events::EventKind::AccountMultisigApprove => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some("A memo".to_string()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data: None,
                    },
                )
                .unwrap()
                .token;
            events::AccountMultisigTransaction::AccountMultisigApprove(
                account::features::multisig::ApproveArgs { token },
            )
        }
        events::EventKind::AccountMultisigRevoke => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some("A memo".to_string()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data: None,
                    },
                )
                .unwrap()
                .token;

            events::AccountMultisigTransaction::AccountMultisigRevoke(
                account::features::multisig::RevokeArgs { token },
            )
        }
        events::EventKind::AccountMultisigExecute => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some("A memo".to_string()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data: None,
                    },
                )
                .unwrap()
                .token;
            // Pre-approve the transaction
            for i in [id, identity(2), identity(3)] {
                let _ = module_impl.multisig_approve(
                    &i,
                    account::features::multisig::ApproveArgs {
                        token: token.clone(),
                    },
                );
            }
            events::AccountMultisigTransaction::AccountMultisigExecute(
                account::features::multisig::ExecuteArgs { token },
            )
        }
        events::EventKind::AccountMultisigWithdraw => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some("A memo".to_string()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data: None,
                    },
                )
                .unwrap()
                .token;
            events::AccountMultisigTransaction::AccountMultisigWithdraw(
                account::features::multisig::WithdrawArgs { token },
            )
        }
        events::EventKind::AccountMultisigSetDefaults => {
            events::AccountMultisigTransaction::AccountMultisigSetDefaults(
                account::features::multisig::SetDefaultsArgs {
                    account: account_id,
                    threshold: Some(1),
                    timeout_in_secs: Some(500),
                    execute_automatically: Some(true),
                },
            )
        }
        _ => unimplemented!(),
    }
}

prop_compose! {
    pub fn setup_with_account_and_tx(account_type: AccountType)(event in arb_event_kind()) -> SetupWithAccountAndTx {
        let SetupWithAccount {
            mut module_impl,
            id,
            account_id,
        } = setup_with_account(account_type.clone());

        let event = event_from_kind(event, &mut module_impl, id, account_id, account_type.clone());

        SetupWithAccountAndTx {
            module_impl,
            id,
            account_id,
            tx: event,
        }
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
        BalanceArgs {
            account: Some(id),
            symbols: Some(vec![symbol].into()),
        },
    );
    assert!(result.is_ok());
    let balances = result.unwrap();
    assert_eq!(balances.balances, BTreeMap::from([(symbol, amount)]));
}

fn arb_event_kind() -> impl Strategy<Value = events::EventKind> {
    prop_oneof![
        // Ledger-related
        Just(events::EventKind::Send),
        // Account-related
        Just(events::EventKind::AccountCreate),
        Just(events::EventKind::AccountDisable),
        Just(events::EventKind::AccountSetDescription),
        Just(events::EventKind::AccountAddRoles),
        Just(events::EventKind::AccountRemoveRoles),
        Just(events::EventKind::AccountAddFeatures),
        // Multisig-related
        Just(events::EventKind::AccountMultisigSubmit),
        Just(events::EventKind::AccountMultisigApprove),
        Just(events::EventKind::AccountMultisigRevoke),
        Just(events::EventKind::AccountMultisigExecute),
        Just(events::EventKind::AccountMultisigWithdraw),
        Just(events::EventKind::AccountMultisigSetDefaults),
    ]
}
