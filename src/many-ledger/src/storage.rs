use crate::error;
use crate::module::validate_account;
use many::message::ResponseMessage;
use many::server::module;
use many::server::module::abci_backend::AbciCommitInfo;
use many::server::module::account::features::multisig;
use many::server::module::account::features::multisig::MultisigTransactionState;
use many::server::module::account::features::FeatureInfo;
use many::server::module::idstore;
use many::server::module::idstore::{CredentialId, PublicKey, RecallPhrase};
use many::server::module::{account, EmptyReturn};
use many::types::ledger::{Symbol, TokenAmount};
use many::types::{events, CborRange, Either, SortOrder, Timestamp};
use many::{Identity, ManyError};
use merk::tree::Tree;
use merk::{rocksdb, BatchEntry, Op};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, Bound};
use std::ops::RangeBounds;
use std::path::Path;
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

fn _execute_multisig_tx(
    ledger: &mut LedgerStorage,
    _tx_id: &[u8],
    storage: &MultisigTransactionStorage,
) -> Result<Vec<u8>, ManyError> {
    let sender = &storage.account;
    match &storage.info.transaction {
        events::AccountMultisigTransaction::Send(module::ledger::SendArgs {
            from: _from,
            to,
            symbol,
            amount,
        }) => {
            ledger.send(sender, to, symbol, amount.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountCreate(args) => {
            let account = account::Account::create(sender, args.clone());
            validate_account(&account)?;

            let id = ledger.add_account(account)?;
            minicbor::to_vec(account::CreateReturn { id })
        }

        events::AccountMultisigTransaction::AccountDisable(args) => {
            let account = ledger
                .get_account(&args.account)
                .ok_or_else(|| account::errors::unknown_account(args.account))?;

            account.needs_role(sender, [account::Role::Owner])?;
            ledger.disable_account(&args.account)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountSetDescription(args) => {
            let account = ledger
                .get_account(&args.account)
                .ok_or_else(|| account::errors::unknown_account(args.account))?;

            account.needs_role(sender, [account::Role::Owner])?;
            ledger.set_description(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountAddRoles(args) => {
            let account = ledger
                .get_account(&args.account)
                .ok_or_else(|| account::errors::unknown_account(args.account))?;
            account.needs_role(sender, [account::Role::Owner])?;
            ledger.add_roles(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountRemoveRoles(args) => {
            let account = ledger
                .get_account(&args.account)
                .ok_or_else(|| account::errors::unknown_account(args.account))?;
            account.needs_role(sender, [account::Role::Owner])?;
            ledger.remove_roles(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountAddFeatures(args) => {
            let account = ledger
                .get_account(&args.account)
                .ok_or_else(|| account::errors::unknown_account(args.account))?;

            account.needs_role(sender, [account::Role::Owner])?;
            ledger.add_features(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigSubmit(arg) => {
            ledger.create_multisig_transaction(sender, arg.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigSetDefaults(arg) => {
            ledger.set_multisig_defaults(sender, arg.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigApprove(arg) => {
            ledger.approve_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigRevoke(arg) => {
            ledger.revoke_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigExecute(arg) => {
            ledger.execute_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigWithdraw(arg) => {
            ledger.withdraw_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        _ => return Err(multisig::errors::transaction_type_unsupported()),
    }
    .map_err(|e| ManyError::serialization_error(e.to_string()))
}

#[derive(minicbor::Encode, minicbor::Decode)]
#[cbor(map)]
pub struct MultisigTransactionStorage {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub info: multisig::InfoReturn,

    #[n(2)]
    pub creation: SystemTime,

    #[n(3)]
    pub disabled: bool,
}

impl MultisigTransactionStorage {
    pub fn disable(&mut self, state: MultisigTransactionState) {
        self.disabled = true;
        self.info.state = state;
    }

    pub fn should_execute(&self) -> bool {
        self.info.approvers.values().filter(|i| i.approved).count() >= self.info.threshold as usize
    }
}

pub const MULTISIG_DEFAULT_THRESHOLD: u64 = 1;
pub const MULTISIG_DEFAULT_TIMEOUT_IN_SECS: u64 = 60 * 60 * 24; // A day.
pub const MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY: bool = false;
pub const MULTISIG_MAXIMUM_TIMEOUT_IN_SECS: u64 = 185 * 60 * 60 * 24; // ~6 months.

#[derive(Clone, minicbor::Encode, minicbor::Decode)]
#[cbor(map)]
struct CredentialStorage {
    #[n(0)]
    cred_id: CredentialId,

    #[n(1)]
    public_key: PublicKey,
}

enum IdStoreRootSeparator {
    RecallPhrase,
    Address,
}

impl IdStoreRootSeparator {
    fn value(&self) -> &[u8] {
        match *self {
            IdStoreRootSeparator::RecallPhrase => b"00",
            IdStoreRootSeparator::Address => b"01",
        }
    }
}

pub(crate) const EVENTS_ROOT: &[u8] = b"/events/";
pub(crate) const MULTISIG_TRANSACTIONS_ROOT: &[u8] = b"/multisig/";
pub(crate) const IDSTORE_ROOT: &[u8] = b"/idstore/";

// Left-shift the height by this amount of bits
const HEIGHT_EVENTID_SHIFT: u64 = 32;

/// Number of bytes in an event ID when serialized. Keys smaller than this
/// will have `\0` prepended, and keys larger will be cut to this number of
/// bytes.
const EVENT_ID_KEY_SIZE_IN_BYTES: usize = 32;

/// Returns the key for the persistent kv-store.
pub(super) fn key_for_account_balance(id: &Identity, symbol: &Symbol) -> Vec<u8> {
    format!("/balances/{}/{}", id, symbol).into_bytes()
}

/// Returns the storage key for an event in the kv-store.
pub(super) fn key_for_event(id: events::EventId) -> Vec<u8> {
    let id = id.as_ref();
    let id = if id.len() > EVENT_ID_KEY_SIZE_IN_BYTES {
        &id[0..EVENT_ID_KEY_SIZE_IN_BYTES]
    } else {
        id
    };

    let mut exp_id = [0u8; EVENT_ID_KEY_SIZE_IN_BYTES];
    exp_id[(EVENT_ID_KEY_SIZE_IN_BYTES - id.len())..].copy_from_slice(id);
    vec![EVENTS_ROOT.to_vec(), exp_id.to_vec()].concat()
}

pub(super) fn key_for_account(id: &Identity) -> Vec<u8> {
    format!("/accounts/{}", id).into_bytes()
}

/// Returns the storage key for a multisig pending transaction.
pub(super) fn key_for_multisig_transaction(token: &[u8]) -> Vec<u8> {
    let token = if token.len() > EVENT_ID_KEY_SIZE_IN_BYTES {
        &token[0..EVENT_ID_KEY_SIZE_IN_BYTES]
    } else {
        token
    };

    let mut exp_token = [0u8; EVENT_ID_KEY_SIZE_IN_BYTES];
    exp_token[(EVENT_ID_KEY_SIZE_IN_BYTES - token.len())..].copy_from_slice(token);

    vec![MULTISIG_TRANSACTIONS_ROOT, &exp_token[..]]
        .concat()
        .to_vec()
}

pub struct LedgerStorage {
    symbols: BTreeMap<Symbol, String>,
    persistent_store: merk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_tid: events::EventId,

    current_time: Option<SystemTime>,
    current_hash: Option<Vec<u8>>,

    next_account_id: u32,
    account_identity: Identity,

    idstore_seed: u64,
}

impl LedgerStorage {
    #[cfg(feature = "balance_testing")]
    pub(crate) fn set_balance_only_for_testing(
        &mut self,
        account: Identity,
        amount: u64,
        symbol: Identity,
    ) {
        assert!(self.symbols.contains_key(&symbol));
        // Make sure we don't run this function when the store has started.
        assert_eq!(self.current_hash, None);

        let key = key_for_account_balance(&account, &symbol);
        let amount = TokenAmount::from(amount);

        self.persistent_store
            .apply(&[(key, Op::Put(amount.to_vec()))])
            .unwrap();

        // Always commit to the store. In blockchain mode this will fail.
        self.persistent_store.commit(&[]).unwrap();
    }
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("symbols", &self.symbols)
            .finish()
    }
}

impl LedgerStorage {
    #[inline]
    pub fn set_time(&mut self, time: SystemTime) {
        self.current_time = Some(time);
    }
    #[inline]
    pub fn now(&self) -> SystemTime {
        self.current_time.unwrap_or_else(SystemTime::now)
    }

    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let symbols = persistent_store
            .get(b"/config/symbols")
            .map_err(|e| e.to_string())?;
        let symbols: BTreeMap<Symbol, String> = symbols
            .map_or_else(|| Ok(Default::default()), |bytes| minicbor::decode(&bytes))
            .map_err(|e| e.to_string())?;
        let next_account_id = persistent_store
            .get(b"/config/account_id")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            });

        let account_identity: Identity = Identity::from_bytes(
            &persistent_store
                .get(b"/config/identity")
                .expect("Could not open storage.")
                .expect("Could not find key '/config/identity' in storage."),
        )
        .map_err(|e| e.to_string())?;

        let height = persistent_store.get(b"/height").unwrap().map_or(0u64, |x| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(x.as_slice());
            u64::from_be_bytes(bytes)
        });

        let idstore_seed = persistent_store
            .get(b"/config/idstore_seed")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            });

        let latest_tid = events::EventId::from(height << HEIGHT_EVENTID_SHIFT);

        Ok(Self {
            symbols,
            persistent_store,
            blockchain,
            latest_tid,
            current_time: None,
            current_hash: None,
            next_account_id,
            account_identity,
            idstore_seed,
        })
    }

    pub fn new<P: AsRef<Path>>(
        symbols: BTreeMap<Symbol, String>,
        initial_balances: BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>>,
        persistent_path: P,
        identity: Identity,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<BatchEntry> = Vec::new();

        for (k, v) in initial_balances.into_iter() {
            for (symbol, tokens) in v.into_iter() {
                if !symbols.contains_key(&symbol) {
                    return Err(format!(r#"Unknown symbol "{}" for identity {}"#, symbol, k));
                }

                let key = key_for_account_balance(&k, &symbol);
                batch.push((key, Op::Put(tokens.to_vec())));
            }
        }

        batch.push((b"/config/identity".to_vec(), Op::Put(identity.to_vec())));
        batch.push((
            b"/config/symbols".to_vec(),
            Op::Put(minicbor::to_vec(&symbols).map_err(|e| e.to_string())?),
        ));

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;
        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            symbols,
            persistent_store,
            blockchain,
            latest_tid: events::EventId::from(vec![0]),
            current_time: None,
            current_hash: None,
            next_account_id: 0,
            account_identity: identity,
            idstore_seed: 0,
        })
    }

    pub fn commit_persistent_store(&mut self) -> Result<(), String> {
        self.persistent_store.commit(&[]).map_err(|e| e.to_string())
    }

    pub fn get_symbols(&self) -> BTreeMap<Symbol, String> {
        self.symbols.clone()
    }

    fn inc_height(&mut self) -> u64 {
        let current_height = self.get_height();
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .unwrap();
        current_height
    }

    pub fn get_height(&self) -> u64 {
        self.persistent_store
            .get(b"/height")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    fn new_account_id(&mut self) -> Identity {
        let current_id = self.next_account_id;
        self.next_account_id += 1;
        self.persistent_store
            .apply(&[(
                b"/config/account_id".to_vec(),
                Op::Put(self.next_account_id.to_be_bytes().to_vec()),
            )])
            .unwrap();

        self.account_identity
            .with_subresource_id(current_id)
            .expect("Too many accounts")
    }

    pub(crate) fn inc_idstore_seed(&mut self) -> u64 {
        let current_seed = self.idstore_seed;
        self.idstore_seed += 1;
        self.persistent_store
            .apply(&[(
                b"/config/idstore_seed".to_vec(),
                Op::Put(self.idstore_seed.to_be_bytes().to_vec()),
            )])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        current_seed
    }

    fn new_event_id(&mut self) -> events::EventId {
        self.latest_tid += 1;
        self.latest_tid.clone()
    }

    pub fn check_timed_out_multisig_transactions(&mut self) -> Result<(), ManyError> {
        use rocksdb::{Direction, IteratorMode, ReadOptions};

        // Set the iterator bounds to iterate all multisig transactions.
        // We will break the loop later if we can.
        let mut options = ReadOptions::default();
        options.set_iterate_lower_bound(MULTISIG_TRANSACTIONS_ROOT);
        let mut bound = MULTISIG_TRANSACTIONS_ROOT.to_vec();
        bound[MULTISIG_TRANSACTIONS_ROOT.len() - 1] += 1;

        let it = self
            .persistent_store
            .iter_opt(IteratorMode::From(&bound, Direction::Reverse), options);

        let mut batch = vec![];

        for (k, v) in it {
            let v = Tree::decode(k.to_vec(), v.as_ref());

            let mut storage: MultisigTransactionStorage = minicbor::decode(v.value())
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
            let now = self.now();

            if now >= storage.info.timeout.0 {
                if !storage.disabled {
                    storage.disable(MultisigTransactionState::Expired);

                    if let Ok(v) = minicbor::to_vec(storage) {
                        batch.push((k.to_vec(), Op::Put(v)));
                    }
                }
            } else if let Ok(d) = now.duration_since(storage.creation) {
                // Since the DB is ordered by event ID (keys), at this point we don't need
                // to continue since we know that the rest is all timed out anyway.
                if d.as_secs() > MULTISIG_MAXIMUM_TIMEOUT_IN_SECS {
                    break;
                }
            }
        }

        if !batch.is_empty() {
            // Reverse the batch so keys are in sorted order.
            batch.reverse();
            self.persistent_store.apply(&batch).unwrap();
        }

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn commit(&mut self) -> AbciCommitInfo {
        // First check if there's any need to clean up multisig transactions. Ignore
        // errors.
        let _ = self.check_timed_out_multisig_transactions();

        let height = self.inc_height();
        let retain_height = 0;
        self.persistent_store.commit(&[]).unwrap();

        let hash = self.persistent_store.root_hash().to_vec();
        self.current_hash = Some(hash.clone());

        self.latest_tid = events::EventId::from(height << HEIGHT_EVENTID_SHIFT);

        AbciCommitInfo {
            retain_height,
            hash: hash.into(),
        }
    }

    pub fn nb_events(&self) -> u64 {
        self.persistent_store
            .get(b"/events_count")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    fn log_event(&mut self, content: events::EventInfo) {
        let current_nb_events = self.nb_events();
        let event = events::EventLog {
            id: self.new_event_id(),
            time: self.now().into(),
            content,
        };

        self.persistent_store
            .apply(&[
                (
                    key_for_event(event.id.clone()),
                    Op::Put(minicbor::to_vec(&event).unwrap()),
                ),
                (
                    b"/events_count".to_vec(),
                    Op::Put((current_nb_events + 1).to_be_bytes().to_vec()),
                ),
            ])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &Symbol) -> TokenAmount {
        if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            let key = key_for_account_balance(identity, symbol);
            match self.persistent_store.get(&key).unwrap() {
                None => TokenAmount::zero(),
                Some(amount) => TokenAmount::from(amount),
            }
        }
    }

    fn get_all_balances(&self, identity: &Identity) -> BTreeMap<&Symbol, TokenAmount> {
        if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            let mut result = BTreeMap::new();
            for symbol in self.symbols.keys() {
                match self
                    .persistent_store
                    .get(&key_for_account_balance(identity, symbol))
                {
                    Ok(None) => {}
                    Ok(Some(value)) => {
                        result.insert(symbol, TokenAmount::from(value));
                    }
                    Err(_) => {}
                }
            }

            result
        }
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Identity,
        symbols: &BTreeSet<Symbol>,
    ) -> BTreeMap<&Symbol, TokenAmount> {
        if symbols.is_empty() {
            self.get_all_balances(identity)
        } else {
            self.get_all_balances(identity)
                .into_iter()
                .filter(|(k, _v)| symbols.contains(*k))
                .collect()
        }
    }

    pub fn send(
        &mut self,
        from: &Identity,
        to: &Identity,
        symbol: &Symbol,
        amount: TokenAmount,
    ) -> Result<(), ManyError> {
        if from == to {
            return Err(error::destination_is_source());
        }

        if amount.is_zero() {
            return Err(error::amount_is_zero());
        }

        if to.is_anonymous() || from.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

        let mut amount_from = self.get_balance(from, symbol);
        if amount > amount_from {
            return Err(error::insufficient_funds());
        }

        info!("send({} => {}, {} {})", from, to, &amount, symbol);

        let mut amount_to = self.get_balance(to, symbol);
        amount_to += amount.clone();
        amount_from -= amount.clone();

        // Keys in batch must be sorted.
        let key_from = key_for_account_balance(from, symbol);
        let key_to = key_for_account_balance(to, symbol);

        let batch: Vec<BatchEntry> = match key_from.cmp(&key_to) {
            Ordering::Less | Ordering::Equal => vec![
                (key_from, Op::Put(amount_from.to_vec())),
                (key_to, Op::Put(amount_to.to_vec())),
            ],
            _ => vec![
                (key_to, Op::Put(amount_to.to_vec())),
                (key_from, Op::Put(amount_from.to_vec())),
            ],
        };

        self.persistent_store.apply(&batch).unwrap();

        self.log_event(events::EventInfo::Send {
            from: *from,
            to: *to,
            symbol: *symbol,
            amount,
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn hash(&self) -> Vec<u8> {
        self.current_hash
            .as_ref()
            .map_or_else(|| self.persistent_store.root_hash().to_vec(), |x| x.clone())
    }

    pub fn iter(&self, range: CborRange<events::EventId>, order: SortOrder) -> LedgerIterator {
        LedgerIterator::scoped_by_id(&self.persistent_store, range, order)
    }

    pub(crate) fn _add_account(
        &mut self,
        mut account: account::Account,
        add_event: bool,
    ) -> Result<Identity, ManyError> {
        let id = self.new_account_id();

        // Set the multisig threshold properly.
        if let Ok(mut multisig) = account.features.get::<multisig::MultisigAccountFeature>() {
            multisig.arg.threshold =
                Some(multisig.arg.threshold.unwrap_or(
                    account
                        .roles
                        .iter()
                        .filter(|(_, roles)| {
                            roles.contains(&account::Role::Owner)
                                || roles.contains(&account::Role::CanMultisigApprove)
                                || roles.contains(&account::Role::CanMultisigSubmit)
                        })
                        .count() as u64,
                ));
            multisig.arg.timeout_in_secs = Some(
                multisig
                    .arg
                    .timeout_in_secs
                    .map_or(MULTISIG_DEFAULT_TIMEOUT_IN_SECS, |v| {
                        MULTISIG_MAXIMUM_TIMEOUT_IN_SECS.min(v)
                    }),
            );
            multisig.arg.execute_automatically = Some(
                multisig
                    .arg
                    .execute_automatically
                    .unwrap_or(MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY),
            );

            account.features.insert(multisig.as_feature());
        }

        if add_event {
            self.log_event(events::EventInfo::AccountCreate {
                account: id,
                description: account.clone().description,
                roles: account.clone().roles,
                features: account.clone().features,
            });
        }

        self.commit_account(&id, account)?;
        Ok(id)
    }

    pub fn add_account(&mut self, account: account::Account) -> Result<Identity, ManyError> {
        let id = self._add_account(account, true)?;
        Ok(id)
    }

    pub fn set_multisig_defaults(
        &mut self,
        sender: &Identity,
        args: multisig::SetDefaultsArgs,
    ) -> Result<(), ManyError> {
        // Verify the sender has the rights to the account.
        let mut account = self
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account.to_string()))?;

        account.needs_role(sender, [account::Role::Owner])?;

        // Set the multisig threshold properly.
        if let Ok(mut multisig) = account.features.get::<multisig::MultisigAccountFeature>() {
            if let Some(threshold) = args.threshold {
                multisig.arg.threshold = Some(threshold);
            }
            let timeout_in_secs = args
                .timeout_in_secs
                .map(|t| t.min(MULTISIG_MAXIMUM_TIMEOUT_IN_SECS));
            if let Some(timeout_in_secs) = timeout_in_secs {
                multisig.arg.timeout_in_secs = Some(timeout_in_secs);
            }
            if let Some(execute_automatically) = args.execute_automatically {
                multisig.arg.execute_automatically = Some(execute_automatically);
            }

            account.features.insert(multisig.as_feature());
            self.log_event(events::EventInfo::AccountMultisigSetDefaults {
                submitter: *sender,
                account: args.account,
                threshold: args.threshold,
                timeout_in_secs,
                execute_automatically: args.execute_automatically,
            });
            self.commit_account(&args.account, account)?;
        }
        Ok(())
    }

    pub fn disable_account(&mut self, id: &Identity) -> Result<(), ManyError> {
        let mut account = self
            .get_account_even_disabled(id)
            .ok_or_else(|| account::errors::unknown_account(*id))?;

        if account.disabled.is_none() || account.disabled == Some(Either::Left(false)) {
            account.disabled = Some(Either::Left(true));
            self.commit_account(id, account)?;
            self.log_event(events::EventInfo::AccountDisable { account: *id });

            if !self.blockchain {
                self.persistent_store
                    .commit(&[])
                    .expect("Could not commit to store.");
            }

            Ok(())
        } else {
            Err(account::errors::unknown_account(*id))
        }
    }

    pub fn set_description(
        &mut self,
        mut account: account::Account,
        args: account::SetDescriptionArgs,
    ) -> Result<(), ManyError> {
        account.set_description(Some(args.clone().description));
        self.log_event(events::EventInfo::AccountSetDescription {
            account: args.account,
            description: args.description,
        });
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn add_roles(
        &mut self,
        mut account: account::Account,
        args: account::AddRolesArgs,
    ) -> Result<(), ManyError> {
        for (id, roles) in &args.roles {
            for r in roles {
                account.add_role(id, *r);
            }
        }

        self.log_event(events::EventInfo::AccountAddRoles {
            account: args.account,
            roles: args.clone().roles,
        });
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn remove_roles(
        &mut self,
        mut account: account::Account,
        args: account::RemoveRolesArgs,
    ) -> Result<(), ManyError> {
        for (id, roles) in &args.roles {
            for r in roles {
                account.remove_role(id, *r);
            }
        }

        self.log_event(events::EventInfo::AccountRemoveRoles {
            account: args.account,
            roles: args.clone().roles,
        });
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn add_features(
        &mut self,
        mut account: account::Account,
        args: account::AddFeaturesArgs,
    ) -> Result<(), ManyError> {
        for new_f in args.features.iter() {
            if account.features.insert(new_f.clone()) {
                return Err(ManyError::unknown("Feature already part of the account."));
            }
        }
        if let Some(ref r) = args.roles {
            for (id, new_r) in r {
                for role in new_r {
                    account.roles.entry(*id).or_default().insert(*role);
                }
            }
        }

        validate_account(&account)?;

        self.log_event(events::EventInfo::AccountAddFeatures {
            account: args.account,
            roles: args.clone().roles.unwrap_or_default(), // TODO: Verify this
            features: args.clone().features,
        });
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn get_account(&self, id: &Identity) -> Option<account::Account> {
        self.get_account_even_disabled(id).and_then(|x| {
            if x.disabled.is_none() || x.disabled == Some(Either::Left(false)) {
                Some(x)
            } else {
                None
            }
        })
    }

    pub fn get_account_even_disabled(&self, id: &Identity) -> Option<account::Account> {
        self.persistent_store
            .get(&key_for_account(id))
            .unwrap_or_default()
            .as_ref()
            .and_then(|bytes| {
                minicbor::decode::<account::Account>(bytes)
                    .map_err(|e| ManyError::deserialization_error(e.to_string()))
                    .ok()
            })
    }

    pub fn commit_account(
        &mut self,
        id: &Identity,
        account: account::Account,
    ) -> Result<(), ManyError> {
        tracing::debug!("commit({:?})", account);

        self.persistent_store
            .apply(&[(
                key_for_account(id),
                Op::Put(
                    minicbor::to_vec(account)
                        .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                ),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .expect("Could not commit to store.");
        }
        Ok(())
    }

    pub fn commit_multisig_transaction(
        &mut self,
        tx_id: &[u8],
        tx: &MultisigTransactionStorage,
    ) -> Result<(), ManyError> {
        self.persistent_store
            .apply(&[(
                key_for_multisig_transaction(tx_id),
                Op::Put(
                    minicbor::to_vec(tx)
                        .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                ),
            )])
            .unwrap();

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .expect("Could not commit to store.");
        }
        Ok(())
    }

    pub fn create_multisig_transaction(
        &mut self,
        sender: &Identity,
        arg: multisig::SubmitTransactionArgs,
    ) -> Result<Vec<u8>, ManyError> {
        let event_id = self.new_event_id();
        let account_id = arg.account;

        // Validate the transaction's information.
        #[allow(clippy::single_match)]
        match arg.transaction.as_ref() {
            events::AccountMultisigTransaction::Send(module::ledger::SendArgs { from, .. }) => {
                if from.as_ref() != Some(&account_id) {
                    warn!("{:?} != {}", from, account_id.to_string());
                    return Err(ManyError::unknown("Invalid transaction.".to_string()));
                }
            }
            _ => {}
        }

        let account = self
            .get_account(&account_id)
            .ok_or_else(|| account::errors::unknown_account(account_id))?;

        let is_owner = account.has_role(sender, "owner");
        account.needs_role(
            sender,
            [account::Role::CanMultisigSubmit, account::Role::Owner],
        )?;

        let multisig_f = account.features.get::<multisig::MultisigAccountFeature>()?;

        let threshold = match arg.threshold {
            Some(t) if is_owner => t,
            Some(_) => return Err(account::errors::user_needs_role("owner")),
            _ => multisig_f
                .arg
                .threshold
                .unwrap_or(MULTISIG_DEFAULT_THRESHOLD),
        };
        let timeout_in_secs = match arg.timeout_in_secs {
            Some(t) if is_owner => t,
            Some(_) => return Err(account::errors::user_needs_role("owner")),
            _ => multisig_f
                .arg
                .timeout_in_secs
                .unwrap_or(MULTISIG_DEFAULT_TIMEOUT_IN_SECS),
        }
        .min(MULTISIG_MAXIMUM_TIMEOUT_IN_SECS);
        let execute_automatically = match arg.execute_automatically {
            Some(e) if is_owner => e,
            Some(_) => return Err(account::errors::user_needs_role("owner")),
            _ => multisig_f
                .arg
                .execute_automatically
                .unwrap_or(MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY),
        };
        let time = self.now();

        // Set the approvers list to include the sender as true.
        let approvers = BTreeMap::from_iter([(*sender, multisig::ApproverInfo { approved: true })]);

        let timeout = Timestamp::from(time + Duration::from_secs(timeout_in_secs));
        let storage = MultisigTransactionStorage {
            account: account_id,
            info: multisig::InfoReturn {
                memo: arg.memo.clone(),
                transaction: arg.transaction.as_ref().clone(),
                submitter: *sender,
                approvers,
                threshold,
                execute_automatically,
                timeout,
                data: arg.data.clone(),
                state: MultisigTransactionState::Pending,
            },
            creation: self.now(),
            disabled: false,
        };

        self.commit_multisig_transaction(event_id.as_ref(), &storage)?;
        self.log_event(events::EventInfo::AccountMultisigSubmit {
            submitter: *sender,
            account: account_id,
            memo: arg.memo,
            transaction: Box::new(*arg.transaction),
            token: Some(event_id.clone().into()),
            threshold,
            timeout,
            execute_automatically,
            data: arg.data,
        });

        Ok(event_id.into())
    }

    pub fn get_multisig_info(&self, tx_id: &[u8]) -> Result<MultisigTransactionStorage, ManyError> {
        let storage_bytes = self
            .persistent_store
            .get(&key_for_multisig_transaction(tx_id))
            .unwrap_or(None)
            .ok_or_else(multisig::errors::transaction_cannot_be_found)?;
        minicbor::decode::<MultisigTransactionStorage>(&storage_bytes)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))
    }

    pub fn approve_multisig(&mut self, sender: &Identity, tx_id: &[u8]) -> Result<bool, ManyError> {
        let mut storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(multisig::errors::transaction_expired_or_withdrawn());
        }

        let account = self
            .get_account(&storage.account)
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        // Validate the right.
        if !account.has_role(sender, account::Role::CanMultisigApprove)
            && !account.has_role(sender, account::Role::CanMultisigSubmit)
            && !account.has_role(sender, account::Role::Owner)
        {
            return Err(multisig::errors::user_cannot_approve_transaction());
        }

        // Update the entry.
        storage.info.approvers.entry(*sender).or_default().approved = true;

        self.commit_multisig_transaction(tx_id, &storage)?;
        self.log_event(events::EventInfo::AccountMultisigApprove {
            account: storage.account,
            token: tx_id.to_vec().into(),
            approver: *sender,
        });

        // If the transaction executes automatically, calculate number of approvers.
        if storage.info.execute_automatically && storage.should_execute() {
            let response = self.execute_multisig_transaction_internal(tx_id, &storage, true)?;
            self.log_event(events::EventInfo::AccountMultisigExecute {
                account: storage.account,
                token: tx_id.to_vec().into(),
                executer: None,
                response,
            });
            return Ok(true);
        }

        Ok(false)
    }

    pub fn revoke_multisig(&mut self, sender: &Identity, tx_id: &[u8]) -> Result<bool, ManyError> {
        let mut storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(multisig::errors::transaction_expired_or_withdrawn());
        }

        let account = self
            .get_account(&storage.account)
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        // We make an exception here for people who already approved.
        if let Some(info) = storage.info.approvers.get_mut(sender) {
            info.approved = false;
        } else if account.has_role(sender, account::Role::CanMultisigSubmit)
            || account.has_role(sender, account::Role::CanMultisigApprove)
            || account.has_role(sender, account::Role::Owner)
        {
            storage.info.approvers.entry(*sender).or_default().approved = false;
        } else {
            return Err(multisig::errors::user_cannot_approve_transaction());
        }

        self.commit_multisig_transaction(tx_id, &storage)?;
        self.log_event(events::EventInfo::AccountMultisigRevoke {
            account: storage.account,
            token: tx_id.to_vec().into(),
            revoker: *sender,
        });
        Ok(false)
    }

    pub fn execute_multisig(
        &mut self,
        sender: &Identity,
        tx_id: &[u8],
    ) -> Result<ResponseMessage, ManyError> {
        let storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(multisig::errors::transaction_expired_or_withdrawn());
        }

        // Verify the sender has the rights to the account.
        let account = self
            .get_account(&storage.account)
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        if !(account.has_role(sender, account::Role::Owner) || storage.info.submitter == *sender) {
            return Err(multisig::errors::cannot_execute_transaction());
        }

        if storage.should_execute() {
            let response = self.execute_multisig_transaction_internal(tx_id, &storage, false)?;
            self.log_event(events::EventInfo::AccountMultisigExecute {
                account: storage.account,
                token: tx_id.to_vec().into(),
                executer: Some(*sender),
                response: response.clone(),
            });
            Ok(response)
        } else {
            Err(multisig::errors::cannot_execute_transaction())
        }
    }

    pub fn withdraw_multisig(&mut self, sender: &Identity, tx_id: &[u8]) -> Result<(), ManyError> {
        let storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(multisig::errors::transaction_expired_or_withdrawn());
        }

        // Verify the sender has the rights to the account.
        let account = self
            .get_account(&storage.account)
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        if !(account.has_role(sender, "owner") || storage.info.submitter == *sender) {
            return Err(multisig::errors::cannot_execute_transaction());
        }

        self.disable_multisig_transaction(tx_id, MultisigTransactionState::Withdrawn)?;
        self.log_event(events::EventInfo::AccountMultisigWithdraw {
            account: storage.account,
            token: tx_id.to_vec().into(),
            withdrawer: *sender,
        });
        Ok(())
    }

    fn disable_multisig_transaction(
        &mut self,
        tx_id: &[u8],
        state: MultisigTransactionState,
    ) -> Result<(), ManyError> {
        let mut storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(account::features::multisig::errors::transaction_expired_or_withdrawn());
        }
        storage.disable(state);

        let v =
            minicbor::to_vec(storage).map_err(|e| ManyError::serialization_error(e.to_string()))?;

        self.persistent_store
            .apply(&[(key_for_multisig_transaction(tx_id), Op::Put(v))])
            .unwrap();
        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .expect("Could not commit to store.");
        }
        Ok(())
    }

    fn execute_multisig_transaction_internal(
        &mut self,
        tx_id: &[u8],
        storage: &MultisigTransactionStorage,
        automatic: bool,
    ) -> Result<ResponseMessage, ManyError> {
        let result = _execute_multisig_tx(self, tx_id, storage);
        self.disable_multisig_transaction(
            tx_id,
            if automatic {
                MultisigTransactionState::ExecutedAutomatically
            } else {
                MultisigTransactionState::ExecutedManually
            },
        )?;

        Ok(ResponseMessage {
            from: storage.account,
            to: None,
            data: result,
            ..Default::default()
        })
    }

    // IdStore
    pub fn store(
        &mut self,
        recall_phrase: &RecallPhrase,
        address: &Identity,
        cred_id: CredentialId,
        public_key: PublicKey,
    ) -> Result<(), ManyError> {
        let recall_phrase_cbor = minicbor::to_vec(recall_phrase)
            .map_err(|e| ManyError::serialization_error(e.to_string()))?;
        if self
            .persistent_store
            .get(&recall_phrase_cbor)
            .map_err(|e| ManyError::unknown(e.to_string()))?
            .is_some()
        {
            return Err(idstore::existing_entry());
        }
        let value = minicbor::to_vec(CredentialStorage {
            cred_id,
            public_key,
        })
        .map_err(|e| ManyError::serialization_error(e.to_string()))?;

        let batch = vec![
            (
                vec![
                    IDSTORE_ROOT,
                    IdStoreRootSeparator::RecallPhrase.value(),
                    &recall_phrase_cbor,
                ]
                .concat(),
                Op::Put(value.clone()),
            ),
            (
                vec![
                    IDSTORE_ROOT,
                    IdStoreRootSeparator::Address.value(),
                    &address.to_vec(),
                ]
                .concat(),
                Op::Put(value),
            ),
        ];

        self.persistent_store.apply(&batch).unwrap();

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .expect("Could not commit to store.");
        }

        Ok(())
    }

    fn get_from_storage(
        &self,
        key: &Vec<u8>,
        sep: IdStoreRootSeparator,
    ) -> Result<Option<Vec<u8>>, ManyError> {
        self.persistent_store
            .get(&vec![IDSTORE_ROOT, sep.value(), key].concat())
            .map_err(|e| ManyError::unknown(e.to_string()))
    }

    pub fn get_from_recall_phrase(
        &self,
        recall_phrase: &RecallPhrase,
    ) -> Result<(CredentialId, PublicKey), ManyError> {
        let recall_phrase_cbor = minicbor::to_vec(&recall_phrase)
            .map_err(|e| ManyError::serialization_error(e.to_string()))?;
        if let Some(value) =
            self.get_from_storage(&recall_phrase_cbor, IdStoreRootSeparator::RecallPhrase)?
        {
            let value: CredentialStorage = minicbor::decode(&value)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
            Ok((value.cred_id, value.public_key))
        } else {
            Err(idstore::entry_not_found(recall_phrase.join(" ")))
        }
    }

    pub fn get_from_address(
        &self,
        address: &Identity,
    ) -> Result<(CredentialId, PublicKey), ManyError> {
        if let Some(value) =
            self.get_from_storage(&address.to_vec(), IdStoreRootSeparator::Address)?
        {
            let value: CredentialStorage = minicbor::decode(&value)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
            Ok((value.cred_id, value.public_key))
        } else {
            Err(idstore::entry_not_found(address.to_string()))
        }
    }
}

pub struct LedgerIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> LedgerIterator<'a> {
    pub fn scoped_by_id(
        merk: &'a merk::Merk,
        range: CborRange<events::EventId>,
        order: SortOrder,
    ) -> Self {
        use rocksdb::{IteratorMode, ReadOptions};
        let mut opts = ReadOptions::default();

        match range.start_bound() {
            Bound::Included(x) => opts.set_iterate_lower_bound(key_for_event(x.clone())),
            Bound::Excluded(x) => opts.set_iterate_lower_bound(key_for_event(x.clone() + 1)),
            Bound::Unbounded => opts.set_iterate_lower_bound(EVENTS_ROOT),
        }
        match range.end_bound() {
            Bound::Included(x) => opts.set_iterate_upper_bound(key_for_event(x.clone() + 1)),
            Bound::Excluded(x) => opts.set_iterate_upper_bound(key_for_event(x.clone())),
            Bound::Unbounded => {
                let mut bound = EVENTS_ROOT.to_vec();
                bound[EVENTS_ROOT.len() - 1] += 1;
                opts.set_iterate_upper_bound(bound);
            }
        }

        let mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        Self {
            inner: merk.iter_opt(mode, opts),
        }
    }
}

impl<'a> Iterator for LedgerIterator<'a> {
    type Item = (Box<[u8]>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| {
            let new_v = Tree::decode(k.to_vec(), v.as_ref());

            (k, new_v.value().to_vec())
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use many::types::events::EventId;

    impl LedgerStorage {
        pub fn set_idstore_seed(&mut self, seed: u64) {
            self.idstore_seed = seed;
        }
    }

    #[test]
    fn event_key_size() {
        let golden_size = key_for_event(EventId::from(0)).len();

        assert_eq!(golden_size, key_for_event(EventId::from(u64::MAX)).len());

        // Test at 1 byte, 2 bytes and 4 bytes boundaries.
        for i in [u8::MAX as u64, u16::MAX as u64, u32::MAX as u64] {
            assert_eq!(golden_size, key_for_event(EventId::from(i - 1)).len());
            assert_eq!(golden_size, key_for_event(EventId::from(i)).len());
            assert_eq!(golden_size, key_for_event(EventId::from(i + 1)).len());
        }

        assert_eq!(
            golden_size,
            key_for_event(EventId::from(b"012345678901234567890123456789".to_vec())).len()
        );

        // Trim the Event ID if it's too long.
        assert_eq!(
            golden_size,
            key_for_event(EventId::from(
                b"0123456789012345678901234567890123456789".to_vec()
            ))
            .len()
        );
        assert_eq!(
            key_for_event(EventId::from(b"01234567890123456789012345678901".to_vec())).len(),
            key_for_event(EventId::from(
                b"0123456789012345678901234567890123456789012345678901234567890123456789".to_vec()
            ))
            .len()
        )
    }
}
