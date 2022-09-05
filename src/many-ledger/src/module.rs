use crate::data_migration::Migration;
use crate::json::InitialStateJson;
use crate::{error, storage::LedgerStorage};
use coset::{CborSerializable, CoseKey, CoseSign1};
use many_error::{ManyError, ManyErrorCode};
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::account::features::{multisig, FeatureInfo, TryCreateFeature};
use many_modules::account::AccountModuleBackend;
use many_modules::{account, events, idstore, ledger, EmptyReturn, ManyModule, ManyModuleInfo};
use many_protocol::{RequestMessage, ResponseMessage};
use many_types::cbor::CborAny;
use many_types::ledger::Symbol;
use many_types::{CborRange, Timestamp, VecOrSingle};
use minicbor::bytes::ByteVec;
use minicbor::decode;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Formatter};
use std::path::Path;
use tracing::info;

const MAXIMUM_EVENT_COUNT: usize = 100;

fn get_roles_for_account(account: &account::Account) -> BTreeSet<account::Role> {
    let features = account.features();

    let mut roles = BTreeSet::new();

    // TODO: somehow keep this list updated with the below.
    if features.has_id(multisig::MultisigAccountFeature::ID) {
        roles.append(&mut multisig::MultisigAccountFeature::roles());
    }
    if features.has_id(account::features::ledger::AccountLedger::ID) {
        roles.append(&mut account::features::ledger::AccountLedger::roles());
    }

    roles
}

pub(crate) fn validate_features_for_account(account: &account::Account) -> Result<(), ManyError> {
    let features = account.features();

    // TODO: somehow keep this list updated with the above.
    if let Err(e) = features.get::<multisig::MultisigAccountFeature>() {
        if e.code() != ManyErrorCode::AttributeNotFound {
            return Err(e);
        }
    }
    if let Err(e) = features.get::<account::features::ledger::AccountLedger>() {
        if e.code() != ManyErrorCode::AttributeNotFound {
            return Err(e);
        }
    }

    Ok(())
}

pub(crate) fn validate_roles_for_account(account: &account::Account) -> Result<(), ManyError> {
    let features = account.features();

    let mut allowed_roles = BTreeSet::from([account::Role::Owner]);
    let mut account_roles = BTreeSet::<account::Role>::new();
    for (_, r) in account.roles.iter() {
        account_roles.extend(r.iter())
    }

    // TODO: somehow keep this list updated with the above.
    if features.get::<multisig::MultisigAccountFeature>().is_ok() {
        allowed_roles.append(&mut multisig::MultisigAccountFeature::roles());
    }
    if features
        .get::<account::features::ledger::AccountLedger>()
        .is_ok()
    {
        allowed_roles.append(&mut account::features::ledger::AccountLedger::roles());
    }

    for r in account_roles {
        if !allowed_roles.contains(&r) {
            return Err(account::errors::unknown_role(r.to_string()));
        }
    }

    Ok(())
}

pub(crate) fn validate_account(account: &account::Account) -> Result<(), ManyError> {
    // Verify that we support all features.
    validate_features_for_account(account)?;

    // Verify the roles are supported by the features
    validate_roles_for_account(account)?;

    Ok(())
}

type EventLogResult = Result<events::EventLog, ManyError>;

fn filter_account<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    account: Option<VecOrSingle<Address>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    if let Some(account) = account {
        let account: Vec<Address> = account.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => account.iter().any(|id| t.is_about(id)),
        }))
    } else {
        it
    }
}

fn filter_event_kind<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    event_kind: Option<VecOrSingle<events::EventKind>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    if let Some(k) = event_kind {
        let k: Vec<events::EventKind> = k.into();
        Box::new(it.filter(move |t| match t {
            Err(_) => true,
            Ok(t) => k.contains(&t.kind()),
        }))
    } else {
        it
    }
}

fn filter_symbol<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    symbol: Option<VecOrSingle<Symbol>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
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
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    range: CborRange<Timestamp>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    Box::new(it.filter(move |t| match t {
        // Propagate the errors.
        Err(_) => true,
        Ok(events::EventLog { time, .. }) => range.contains(time),
    }))
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
        migrations: BTreeMap<String, Migration>,
    ) -> Result<Self, ManyError> {
        let storage = if let Some(state) = initial_state {
            let mut storage = LedgerStorage::new(
                state.symbols(),
                state.balances()?,
                persistence_store_path,
                state.identity,
                blockchain,
                state.id_store_seed,
                state.id_store_keys.map(|keys| {
                    keys.iter()
                        .map(|(k, v)| {
                            let k = base64::decode(k).expect("Invalid base64 for key");
                            let v = base64::decode(v).expect("Invalid base64 for value");
                            (k, v)
                        })
                        .collect()
                }),
                migrations,
            )
            .map_err(ManyError::unknown)?;

            if let Some(accounts) = state.accounts {
                for account in accounts {
                    account
                        .create_account(&mut storage)
                        .expect("Could not create accounts");
                }
                storage.commit_persistent_store().expect("Could not commit");
            }
            if let Some(h) = state.hash {
                // Verify the hash.
                let actual = hex::encode(storage.hash());
                if actual != h {
                    return Err(error::invalid_initial_state(h, actual));
                }
            }

            storage
        } else {
            LedgerStorage::load(persistence_store_path, blockchain, migrations).unwrap()
        };

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self { storage })
    }

    #[cfg(feature = "balance_testing")]
    pub fn set_balance_only_for_testing(&mut self, account: Address, balance: u64, symbol: Symbol) {
        self.storage
            .set_balance_only_for_testing(account, balance, symbol);
    }
}

impl ledger::LedgerModuleBackend for LedgerModuleImpl {
    fn info(
        &self,
        _sender: &Address,
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
        sender: &Address,
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
    fn send(&mut self, sender: &Address, args: ledger::SendArgs) -> Result<EmptyReturn, ManyError> {
        let ledger::SendArgs {
            from,
            to,
            amount,
            symbol,
        } = args;

        let from = from.as_ref().unwrap_or(sender);
        if from != sender {
            if let Some(account) = self.storage.get_account(from) {
                if !account.has_role(sender, account::Role::Owner) {
                    if account
                        .features
                        .has_id(account::features::ledger::AccountLedger::ID)
                    {
                        account.needs_role(sender, [account::Role::CanLedgerTransact])?;
                    } else {
                        return Err(error::unauthorized());
                    }
                }
            } else {
                return Err(error::unauthorized());
            }
        }

        self.storage.send(from, &to, &symbol, amount)?;
        Ok(EmptyReturn)
    }
}

impl events::EventsModuleBackend for LedgerModuleImpl {
    fn info(&self, _args: events::InfoArgs) -> Result<events::InfoReturn, ManyError> {
        use strum::IntoEnumIterator;
        Ok(events::InfoReturn {
            total: self.storage.nb_events(),
            event_types: events::EventKind::iter().collect(),
        })
    }

    fn list(&self, args: events::ListArgs) -> Result<events::ListReturns, ManyError> {
        let events::ListArgs {
            count,
            order,
            filter,
        } = args;
        let filter = filter.unwrap_or_default();

        let count = count.map_or(MAXIMUM_EVENT_COUNT, |c| {
            std::cmp::min(c as usize, MAXIMUM_EVENT_COUNT)
        });

        let storage = &self.storage;
        let nb_events = storage.nb_events();
        let iter = storage.iter(
            filter.id_range.unwrap_or_default(),
            order.unwrap_or_default(),
        );

        let iter = Box::new(iter.map(|item| {
            let (_k, v) = item.map_err(|e| ManyError::unknown(e.to_string()))?;
            decode::<events::EventLog>(v.as_slice())
                .map_err(|e| ManyError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_event_kind(iter, filter.kind);
        let iter = filter_symbol(iter, filter.symbol);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());

        let events: Vec<events::EventLog> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(events::ListReturns { nb_events, events })
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

                // Events
                ("events.info".to_string(), EndpointInfo { is_command: false }),
                ("events.list".to_string(), EndpointInfo { is_command: false }),

                // IdStore
                ("idstore.store".to_string(), EndpointInfo { is_command: true }),
                ("idstore.getFromRecallPhrase".to_string(), EndpointInfo { is_command: false }),
                ("idstore.getFromAddress".to_string(), EndpointInfo { is_command: false }),

                // Accounts
                ("account.create".to_string(), EndpointInfo { is_command: true }),
                ("account.setDescription".to_string(), EndpointInfo { is_command: true }),
                ("account.listRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.getRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.addRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.removeRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.info".to_string(), EndpointInfo { is_command: false }),
                ("account.disable".to_string(), EndpointInfo { is_command: true }),
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

    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError> {
        info!("abci.init_chain()",);
        Ok(InitChainReturn {})
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        let time = info.time;
        info!(
            "abci.block_begin(): time={:?} curr_height={}",
            time,
            self.storage.get_height()
        );

        if let Some(time) = time {
            let time = Timestamp::new(time)?;
            self.storage.set_time(time);
        }

        Ok(BeginBlockReturn {})
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

impl AccountModuleBackend for LedgerModuleImpl {
    fn create(
        &mut self,
        sender: &Address,
        args: account::CreateArgs,
    ) -> Result<account::CreateReturn, ManyError> {
        if args.features.is_empty() {
            return Err(account::errors::empty_feature());
        }
        let account = account::Account::create(sender, args);

        validate_account(&account)?;

        let id = self.storage.add_account(account)?;
        Ok(account::CreateReturn { id })
    }

    fn set_description(
        &mut self,
        sender: &Address,
        args: account::SetDescriptionArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
            return Err(account::errors::user_needs_role("owner"));
        }

        self.storage.set_description(account, args)?;
        Ok(EmptyReturn)
    }

    fn list_roles(
        &self,
        _sender: &Address,
        args: account::ListRolesArgs,
    ) -> Result<account::ListRolesReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;
        Ok(account::ListRolesReturn {
            roles: get_roles_for_account(&account),
        })
    }

    fn get_roles(
        &self,
        _sender: &Address,
        args: account::GetRolesArgs,
    ) -> Result<account::GetRolesReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        let mut roles = BTreeMap::new();
        for id in args.identities {
            roles.insert(id, account.get_roles(&id));
        }

        Ok(account::GetRolesReturn { roles })
    }

    fn add_roles(
        &mut self,
        sender: &Address,
        args: account::AddRolesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
            return Err(account::errors::user_needs_role("owner"));
        }
        self.storage.add_roles(account, args)?;
        Ok(EmptyReturn)
    }

    fn remove_roles(
        &mut self,
        sender: &Address,
        args: account::RemoveRolesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
            return Err(account::errors::user_needs_role(account::Role::Owner));
        }
        self.storage.remove_roles(account, args)?;
        Ok(EmptyReturn)
    }

    fn info(
        &self,
        _sender: &Address,
        args: account::InfoArgs,
    ) -> Result<account::InfoReturn, ManyError> {
        let account::Account {
            description,
            roles,
            features,
            disabled,
        } = self
            .storage
            .get_account_even_disabled(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        Ok(account::InfoReturn {
            description,
            roles,
            features,
            disabled,
        })
    }

    fn disable(
        &mut self,
        sender: &Address,
        args: account::DisableArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
            return Err(account::errors::user_needs_role(account::Role::Owner));
        }

        self.storage.disable_account(&args.account)?;
        Ok(EmptyReturn)
    }

    fn add_features(
        &mut self,
        sender: &Address,
        args: account::AddFeaturesArgs,
    ) -> Result<account::AddFeaturesReturn, ManyError> {
        if args.features.is_empty() {
            return Err(account::errors::empty_feature());
        }
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        account.needs_role(sender, [account::Role::Owner])?;
        self.storage.add_features(account, args)?;
        Ok(EmptyReturn)
    }
}

impl multisig::AccountMultisigModuleBackend for LedgerModuleImpl {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Address,
        arg: multisig::SubmitTransactionArgs,
    ) -> Result<multisig::SubmitTransactionReturn, ManyError> {
        let token = self.storage.create_multisig_transaction(sender, arg)?;
        Ok(multisig::SubmitTransactionReturn {
            token: ByteVec::from(token),
        })
    }

    fn multisig_info(
        &self,
        _sender: &Address,
        args: multisig::InfoArgs,
    ) -> Result<multisig::InfoReturn, ManyError> {
        let info = self.storage.get_multisig_info(&args.token)?;
        Ok(info.info)
    }

    fn multisig_set_defaults(
        &mut self,
        sender: &Address,
        args: multisig::SetDefaultsArgs,
    ) -> Result<multisig::SetDefaultsReturn, ManyError> {
        self.storage
            .set_multisig_defaults(sender, args)
            .map(|_| EmptyReturn)
    }

    fn multisig_approve(
        &mut self,
        sender: &Address,
        args: multisig::ApproveArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .approve_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_revoke(
        &mut self,
        sender: &Address,
        args: multisig::RevokeArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .revoke_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_execute(
        &mut self,
        sender: &Address,
        args: multisig::ExecuteArgs,
    ) -> Result<ResponseMessage, ManyError> {
        self.storage.execute_multisig(sender, args.token.as_slice())
    }

    fn multisig_withdraw(
        &mut self,
        sender: &Address,
        args: multisig::WithdrawArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .withdraw_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }
}

/// A module for returning the features by this account.
pub struct AccountFeatureModule<T: AccountModuleBackend> {
    inner: account::AccountModule<T>,
    info: ManyModuleInfo,
}

impl<T: AccountModuleBackend> AccountFeatureModule<T> {
    pub fn new(
        inner: account::AccountModule<T>,
        features: impl IntoIterator<Item = account::features::Feature>,
    ) -> Self {
        let mut info: ManyModuleInfo = inner.info().clone();
        info.attribute = info.attribute.map(|mut a| {
            for f in features.into_iter() {
                a.arguments.push(CborAny::Int(f.id() as i64));
            }
            a
        });

        Self { inner, info }
    }
}

impl<T: AccountModuleBackend> Debug for AccountFeatureModule<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccountFeatureModule")
    }
}

#[async_trait::async_trait]
impl<T: AccountModuleBackend> ManyModule for AccountFeatureModule<T> {
    fn info(&self) -> &ManyModuleInfo {
        &self.info
    }

    fn validate(&self, message: &RequestMessage, envelope: &CoseSign1) -> Result<(), ManyError> {
        self.inner.validate(message, envelope)
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        self.inner.execute(message).await
    }
}

/// Return a recall phrase
//
/// The following relation need to hold for having a valid decoding/encoding:
///
///     // length_bytes(data) * 8 + checksum = number_of(words) * 11
///
/// See [bip39-dict](https://github.com/vincenthz/bip39-dict) for details
///
/// # Generic Arguments
///
/// * `W` - Word cound
/// * `FB` - Full Bytes
/// * `CS` - Checksum Bytes
pub fn generate_recall_phrase<const W: usize, const FB: usize, const CS: usize>(
    seed: &[u8],
) -> Result<Vec<String>, ManyError> {
    let entropy = bip39_dict::Entropy::<FB>::from_slice(seed)
        .ok_or_else(|| ManyError::unknown("Unable to generate entropy"))?;
    let mnemonic = entropy.to_mnemonics::<W, CS>().unwrap();
    let recall_phrase = mnemonic
        .to_string(&bip39_dict::ENGLISH)
        .split_whitespace()
        .map(|e| e.to_string())
        .collect::<Vec<String>>();
    Ok(recall_phrase)
}

impl idstore::IdStoreModuleBackend for LedgerModuleImpl {
    fn store(
        &mut self,
        sender: &Address,
        idstore::StoreArgs {
            address,
            cred_id,
            public_key,
        }: idstore::StoreArgs,
    ) -> Result<idstore::StoreReturns, ManyError> {
        if sender.is_anonymous() {
            return Err(ManyError::invalid_identity());
        }

        if !address.is_public_key() {
            return Err(idstore::invalid_address(address.to_string()));
        }

        if !(16..=1023).contains(&cred_id.0.len()) {
            return Err(idstore::invalid_credential_id(hex::encode(&*cred_id.0)));
        }

        let _: CoseKey = CoseKey::from_slice(&public_key.0)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        let mut current_try = 1u8;
        let recall_phrase = loop {
            if current_try > 8 {
                return Err(idstore::recall_phrase_generation_failed());
            }

            let seed = self.storage.inc_idstore_seed();
            // Entropy can only be generated if the seed array contains the
            // EXACT amount of full bytes, i.e., the FB parameter of
            // `generate_recall_phrase`
            let recall_phrase = match seed {
                0..=0xFFFF => generate_recall_phrase::<2, 2, 6>(&seed.to_be_bytes()[6..]),
                0x10000..=0xFFFFFF => generate_recall_phrase::<3, 4, 1>(&seed.to_be_bytes()[4..]),
                0x1000000..=0xFFFFFFFF => {
                    generate_recall_phrase::<4, 5, 4>(&seed.to_be_bytes()[3..])
                }
                0x100000000..=0xFFFFFFFFFF => {
                    generate_recall_phrase::<5, 6, 7>(&seed.to_be_bytes()[2..])
                }
                _ => unimplemented!(),
            }?;

            if self.storage.get_from_recall_phrase(&recall_phrase).is_ok() {
                current_try += 1;
                tracing::debug!("Recall phrase generation failed, retrying...")
            } else {
                break recall_phrase;
            }
        };

        self.storage
            .store(&recall_phrase, &address, cred_id, public_key)?;
        Ok(idstore::StoreReturns(recall_phrase))
    }

    fn get_from_recall_phrase(
        &self,
        args: idstore::GetFromRecallPhraseArgs,
    ) -> Result<idstore::GetReturns, ManyError> {
        let (cred_id, public_key) = self.storage.get_from_recall_phrase(&args.0)?;
        Ok(idstore::GetReturns {
            cred_id,
            public_key,
        })
    }

    fn get_from_address(
        &self,
        args: idstore::GetFromAddressArgs,
    ) -> Result<idstore::GetReturns, ManyError> {
        let (cred_id, public_key) = self.storage.get_from_address(&args.0)?;
        Ok(idstore::GetReturns {
            cred_id,
            public_key,
        })
    }
}

pub struct IdStoreWebAuthnModule<T: idstore::IdStoreModuleBackend> {
    pub inner: idstore::IdStoreModule<T>,
    pub check_webauthn: bool,
}

impl<T: idstore::IdStoreModuleBackend> Debug for IdStoreWebAuthnModule<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdStoreWebAuthnModule")
    }
}

#[async_trait::async_trait]
impl<T: idstore::IdStoreModuleBackend> ManyModule for IdStoreWebAuthnModule<T> {
    fn info(&self) -> &ManyModuleInfo {
        self.inner.info()
    }

    fn validate(&self, message: &RequestMessage, envelope: &CoseSign1) -> Result<(), ManyError> {
        let result: Result<(), ManyError> = self.inner.validate(message, envelope);
        if let Err(e) = result {
            if e.code() == ManyError::non_webauthn_request_denied("").code() && !self.check_webauthn
            {
                return Ok(());
            } else {
                return Err(e);
            }
        };
        Ok(())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        self.inner.execute(message).await
    }
}

#[cfg(test)]
mod tests {
    use crate::json::InitialStateJson;
    use crate::module::LedgerModuleImpl;
    use coset::CborSerializable;
    use many_identity::Identity;
    use many_identity_dsa::ed25519::generate_random_ed25519_identity;
    use many_modules::idstore;
    use many_modules::idstore::IdStoreModuleBackend;

    #[test]
    /// Test every recall phrase generation codepath
    fn idstore_generate_recall_phrase_all_codepaths() {
        let cose_key_id = generate_random_ed25519_identity();
        let public_key: idstore::PublicKey =
            idstore::PublicKey(cose_key_id.public_key().to_vec().unwrap().into());
        let mut module_impl = LedgerModuleImpl::new(
            Some(
                InitialStateJson::read("../../staging/ledger_state.json5")
                    .or_else(|_| InitialStateJson::read("staging/ledger_state.json5"))
                    .expect("Could not read initial state."),
            ),
            tempfile::tempdir().unwrap(),
            false,
        )
        .unwrap();
        let cred_id = idstore::CredentialId(vec![1; 16].into());
        let id = cose_key_id.address();

        // Basic call
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 2);

        // Make sure another call provides a different result
        let result2 = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result2.is_ok());
        let rp2 = result2.unwrap().0;
        assert_eq!(rp2.len(), 2);
        assert_ne!(rp, rp2);

        // Generate the first 8 recall phrase
        for _ in 2..8 {
            let result3 = module_impl.store(
                &id,
                idstore::StoreArgs {
                    address: id,
                    cred_id: cred_id.clone(),
                    public_key: public_key.clone(),
                },
            );
            assert!(result3.is_ok());
        }

        // And reset the seed 0
        module_impl.storage.set_idstore_seed(0);

        // This should trigger the `recall_phrase_generation_failed()` exception
        let result4 = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result4.is_err());
        assert_eq!(
            result4.unwrap_err().code(),
            idstore::recall_phrase_generation_failed().code()
        );

        // Generate a 3-words phrase
        module_impl.storage.set_idstore_seed(0x10000);
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 3);

        // Generate a 4-words phrase
        module_impl.storage.set_idstore_seed(0x1000000);
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 4);

        // Generate a 5-words phrase
        module_impl.storage.set_idstore_seed(0x100000000);
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id,
                public_key,
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 5);
    }
}
