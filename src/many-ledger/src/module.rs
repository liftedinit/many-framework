use crate::{error, storage::LedgerStorage};
use many::message::error::ManyErrorCode;
use many::message::ResponseMessage;
use many::server::module::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo, ManyAbciModuleBackend,
};
use many::server::module::account::features::multisig::{
    ApproveArg, ExecuteArg, InfoArg, RevokeArg, SetDefaultsArg, SetDefaultsReturn,
    SubmitTransactionArg, SubmitTransactionReturn, WithdrawArg,
};
use many::server::module::account::features::{FeatureInfo, TryCreateFeature};
use many::server::module::account::{
    Account, AddFeaturesArgs, AddRolesArgs, CreateArgs, CreateReturn, DeleteArgs, GetRolesArgs,
    GetRolesReturn, InfoArgs, InfoReturn, ListRolesArgs, ListRolesReturn, RemoveRolesArgs,
    SetDescriptionArgs,
};
use many::server::module::{account, ledger, EmptyReturn};
use many::types::ledger::{Symbol, TokenAmount, Transaction, TransactionKind};
use many::types::{CborRange, Timestamp, VecOrSingle};
use many::{Identity, ManyError};
use minicbor::bytes::ByteVec;
use minicbor::decode;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};
use tracing::info;

const MAXIMUM_TRANSACTION_COUNT: usize = 100;

fn get_roles_for_account(account: &Account) -> BTreeSet<String> {
    let features = account.features();

    let mut roles = BTreeSet::new();

    // TODO: somehow keep this list updated with the below.
    if features.has_id(account::features::multisig::MultisigAccountFeature::ID) {
        roles.append(&mut account::features::multisig::MultisigAccountFeature::roles());
    }
    if features.has_id(account::features::ledger::AccountLedger::ID) {
        roles.append(&mut account::features::ledger::AccountLedger::roles());
    }

    roles
}

fn validate_features_for_account(account: &Account) -> Result<(), ManyError> {
    let features = account.features();

    // TODO: somehow keep this list updated with the above.
    if let Err(e) = features.get::<account::features::multisig::MultisigAccountFeature>() {
        if e.code != ManyErrorCode::AttributeNotFound {
            return Err(e);
        }
    }
    if let Err(e) = features.get::<account::features::ledger::AccountLedger>() {
        if e.code != ManyErrorCode::AttributeNotFound {
            return Err(e);
        }
    }

    Ok(())
}

type TxResult = Result<Transaction, ManyError>;
fn filter_account<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    account: Option<VecOrSingle<Identity>>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    if let Some(account) = account {
        let account: Vec<Identity> = account.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => account.iter().any(|id| t.is_about(id)),
        }))
    } else {
        it
    }
}

fn filter_transaction_kind<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    transaction_kind: Option<VecOrSingle<TransactionKind>>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    if let Some(k) = transaction_kind {
        let k: Vec<TransactionKind> = k.into();
        Box::new(it.filter(move |t| match t {
            Err(_) => true,
            Ok(t) => k.contains(&t.kind()),
        }))
    } else {
        it
    }
}

fn filter_symbol<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    symbol: Option<VecOrSingle<Symbol>>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    if let Some(s) = symbol {
        let s: BTreeSet<Symbol> = s.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => t.symbol().map_or(false, |x| s.contains(x)),
        }))
    } else {
        it
    }
}

fn filter_date<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    range: CborRange<Timestamp>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    Box::new(it.filter(move |t| match t {
        // Propagate the errors.
        Err(_) => true,
        Ok(Transaction { time, .. }) => range.contains(time),
    }))
}

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    identity: Identity,
    initial: BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>>,
    symbols: BTreeMap<Identity, String>,
    hash: Option<String>,
}

/// A simple ledger that keeps transactions in memory.
#[derive(Debug)]
pub struct LedgerModuleImpl {
    storage: LedgerStorage,
}

impl LedgerModuleImpl {
    pub fn new<P: AsRef<Path>>(
        initial_state: Option<InitialStateJson>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = if let Some(state) = initial_state {
            let storage = LedgerStorage::new(
                state.symbols,
                state.initial,
                persistence_store_path,
                state.identity,
                blockchain,
            )
            .map_err(ManyError::unknown)?;

            if let Some(h) = state.hash {
                // Verify the hash.
                let actual = hex::encode(storage.hash());
                if actual != h {
                    return Err(error::invalid_initial_state(h, actual));
                }
            }

            storage
        } else {
            LedgerStorage::load(persistence_store_path, blockchain).unwrap()
        };

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self { storage })
    }

    #[cfg(feature = "balance_testing")]
    pub fn set_balance_only_for_testing(
        &mut self,
        account: Identity,
        balance: u64,
        symbol: Identity,
    ) {
        self.storage
            .set_balance_only_for_testing(account, balance, symbol);
    }
}

impl ledger::LedgerModuleBackend for LedgerModuleImpl {
    fn info(
        &self,
        _sender: &Identity,
        _args: ledger::InfoArgs,
    ) -> Result<ledger::InfoReturns, ManyError> {
        let storage = &self.storage;

        // Hash the storage.
        let hash = storage.hash();
        let symbols = storage.get_symbols();

        info!(
            "info(): hash={} symbols={:?}",
            hex::encode(storage.hash()).as_str(),
            symbols
        );

        Ok(ledger::InfoReturns {
            symbols: symbols.keys().copied().collect(),
            hash: hash.into(),
            local_names: symbols,
        })
    }

    fn balance(
        &self,
        sender: &Identity,
        args: ledger::BalanceArgs,
    ) -> Result<ledger::BalanceReturns, ManyError> {
        let ledger::BalanceArgs { account, symbols } = args;

        let identity = account.as_ref().unwrap_or(sender);

        let storage = &self.storage;
        let symbols = symbols.unwrap_or_default().0;

        let balances = storage
            .get_multiple_balances(identity, &BTreeSet::from_iter(symbols.clone().into_iter()));
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        Ok(ledger::BalanceReturns {
            balances: balances.into_iter().map(|(k, v)| (*k, v)).collect(),
        })
    }
}

impl ledger::LedgerCommandsModuleBackend for LedgerModuleImpl {
    fn send(&mut self, sender: &Identity, args: ledger::SendArgs) -> Result<(), ManyError> {
        let ledger::SendArgs {
            from,
            to,
            amount,
            symbol,
        } = args;

        let from = from.as_ref().unwrap_or(sender);

        // TODO: allow some ACLs or delegation on the ledger.
        if from != sender {
            return Err(error::unauthorized());
        }

        self.storage.send(from, &to, &symbol, amount)?;
        Ok(())
    }
}

impl ledger::LedgerTransactionsModuleBackend for LedgerModuleImpl {
    fn transactions(
        &self,
        _args: ledger::TransactionsArgs,
    ) -> Result<ledger::TransactionsReturns, ManyError> {
        Ok(ledger::TransactionsReturns {
            nb_transactions: self.storage.nb_transactions(),
        })
    }

    fn list(&self, args: ledger::ListArgs) -> Result<ledger::ListReturns, ManyError> {
        let ledger::ListArgs {
            count,
            order,
            filter,
        } = args;
        let filter = filter.unwrap_or_default();

        let count = count.map_or(MAXIMUM_TRANSACTION_COUNT, |c| {
            std::cmp::min(c as usize, MAXIMUM_TRANSACTION_COUNT)
        });

        let storage = &self.storage;
        let nb_transactions = storage.nb_transactions();
        let iter = storage.iter(
            filter.id_range.unwrap_or_default(),
            order.unwrap_or_default(),
        );

        let iter = Box::new(iter.map(|(_k, v)| {
            decode::<Transaction>(v.as_slice())
                .map_err(|e| ManyError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_transaction_kind(iter, filter.kind);
        let iter = filter_symbol(iter, filter.symbol);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());

        let transactions: Vec<Transaction> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(ledger::ListReturns {
            nb_transactions,
            transactions,
        })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl ManyAbciModuleBackend for LedgerModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), EndpointInfo { is_command: false }),
                ("ledger.balance".to_string(), EndpointInfo { is_command: false }),
                ("ledger.send".to_string(), EndpointInfo { is_command: true }),
                ("ledger.transactions".to_string(), EndpointInfo { is_command: false }),
                ("ledger.list".to_string(), EndpointInfo { is_command: false }),

                // Accounts
                ("account.create".to_string(), EndpointInfo { is_command: true }),
                ("account.setDescription".to_string(), EndpointInfo { is_command: true }),
                ("account.listRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.getRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.addRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.removeRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.info".to_string(), EndpointInfo { is_command: false }),
                ("account.delete".to_string(), EndpointInfo { is_command: true }),
                ("account.addFeatures".to_string(), EndpointInfo { is_command: true }),

                // Account Features - Multisig
                ("account.multisigSetDefaults".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigSubmitTransaction".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigInfo".to_string(), EndpointInfo { is_command: false }),
                ("account.multisigApprove".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigRevoke".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigExecute".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigWithdraw".to_string(), EndpointInfo { is_command: true }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<(), ManyError> {
        info!("abci.init_chain()",);
        Ok(())
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<(), ManyError> {
        let time = info.time;
        info!("abci.block_begin(): time={:?}", time);

        if let Some(time) = time {
            let time = UNIX_EPOCH.checked_add(Duration::from_secs(time)).unwrap();
            self.storage.set_time(time);
        }

        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
        let storage = &self.storage;

        info!(
            "abci.info(): height={} hash={}",
            storage.get_height(),
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }
}

impl account::AccountModuleBackend for LedgerModuleImpl {
    fn create(&mut self, sender: &Identity, args: CreateArgs) -> Result<CreateReturn, ManyError> {
        let account = Account::create(sender, args);

        // Verify that we support all features.
        validate_features_for_account(&account)?;

        let id = self.storage.add_account(account)?;
        Ok(CreateReturn { id })
    }

    fn set_description(
        &mut self,
        sender: &Identity,
        args: SetDescriptionArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let mut account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if account.has_role(sender, "owner") {
            account.set_description(Some(args.description));
            self.storage.commit_account(&args.account, account)?;
            Ok(EmptyReturn)
        } else {
            Err(account::errors::user_needs_role("owner"))
        }
    }

    fn list_roles(
        &self,
        _sender: &Identity,
        args: ListRolesArgs,
    ) -> Result<ListRolesReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;
        Ok(ListRolesReturn {
            roles: get_roles_for_account(&account),
        })
    }

    fn get_roles(
        &self,
        _sender: &Identity,
        args: GetRolesArgs,
    ) -> Result<GetRolesReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        let mut roles = BTreeMap::new();
        for id in args.identities {
            roles.insert(id, account.get_roles(&id));
        }

        Ok(GetRolesReturn { roles })
    }

    fn add_roles(
        &mut self,
        sender: &Identity,
        args: AddRolesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let mut account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, "owner") {
            return Err(account::errors::user_needs_role("owner"));
        }
        for (id, roles) in args.roles {
            for r in roles {
                account.add_role(&id, r);
            }
        }

        self.storage.commit_account(&args.account, account)?;
        Ok(EmptyReturn)
    }

    fn remove_roles(
        &mut self,
        sender: &Identity,
        args: RemoveRolesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let mut account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, "owner") {
            return Err(account::errors::user_needs_role("owner"));
        }
        for (id, roles) in args.roles {
            for r in roles {
                account.remove_role(&id, r);
            }
        }

        self.storage.commit_account(&args.account, account)?;
        Ok(EmptyReturn)
    }

    fn info(&self, _sender: &Identity, args: InfoArgs) -> Result<InfoReturn, ManyError> {
        let Account {
            description,
            roles,
            features,
        } = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        Ok(InfoReturn {
            description,
            roles,
            features,
        })
    }

    fn delete(&mut self, sender: &Identity, args: DeleteArgs) -> Result<EmptyReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, "owner") {
            return Err(account::errors::user_needs_role("owner"));
        }

        self.storage.delete_account(&args.account)?;
        Ok(EmptyReturn)
    }

    fn add_features(
        &mut self,
        _sender: &Identity,
        _args: AddFeaturesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        Err(ManyError::unknown("Unsupported.".to_string()))
    }
}

impl account::features::multisig::AccountMultisigModuleBackend for LedgerModuleImpl {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Identity,
        arg: SubmitTransactionArg,
    ) -> Result<SubmitTransactionReturn, ManyError> {
        let token = self.storage.create_multisig_transaction(sender, arg)?;
        Ok(SubmitTransactionReturn {
            token: ByteVec::from(token),
        })
    }

    fn multisig_info(
        &self,
        _sender: &Identity,
        args: InfoArg,
    ) -> Result<account::features::multisig::InfoReturn, ManyError> {
        let info = self.storage.get_multisig_info(&args.token)?;
        Ok(info.info)
    }

    fn multisig_set_defaults(
        &mut self,
        sender: &Identity,
        args: SetDefaultsArg,
    ) -> Result<SetDefaultsReturn, ManyError> {
        self.storage
            .set_multisig_defaults(sender, args)
            .map(|_| EmptyReturn)
    }

    fn multisig_approve(
        &mut self,
        sender: &Identity,
        args: ApproveArg,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .approve_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_revoke(
        &mut self,
        sender: &Identity,
        args: RevokeArg,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .revoke_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_execute(
        &mut self,
        sender: &Identity,
        args: ExecuteArg,
    ) -> Result<ResponseMessage, ManyError> {
        self.storage.execute_multisig(sender, args.token.as_slice())
    }

    fn multisig_withdraw(
        &mut self,
        sender: &Identity,
        args: WithdrawArg,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .withdraw_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }
}
