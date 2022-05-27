use crate::json::InitialStateJson;
use crate::{error, storage::LedgerStorage};
use coset::{CborSerializable, CoseKey};
use many::message::error::ManyErrorCode;
use many::message::ResponseMessage;
use many::server::module::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo, ManyAbciModuleBackend,
};
use many::server::module::account::features::multisig;
use many::server::module::account::features::{FeatureInfo, TryCreateFeature};
use many::server::module::idstore::{
    GetFromAddressArgs, GetFromRecallPhraseArgs, GetReturns, IdStoreModuleBackend, StoreArgs,
    StoreReturns,
};
use many::server::module::{account, idstore, ledger, EmptyReturn};
use many::types::ledger::{Symbol, Transaction, TransactionKind};
use many::types::{CborRange, Timestamp, VecOrSingle};
use many::{Identity, ManyError};
use minicbor::bytes::ByteVec;
use minicbor::decode;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};
use tracing::info;

const MAXIMUM_TRANSACTION_COUNT: usize = 100;

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
            let mut storage = LedgerStorage::new(
                state.symbols,
                state.initial,
                persistence_store_path,
                state.identity,
                blockchain,
            )
            .map_err(ManyError::unknown)?;

            if let Some(accounts) = state.accounts {
                for account in accounts {
                    account
                        .create_account(&mut storage)
                        .expect("Could not create accounts");
                }
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
    fn send(
        &mut self,
        sender: &Identity,
        args: ledger::SendArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let ledger::SendArgs {
            from,
            to,
            amount,
            symbol,
        } = args;

        let from = from.as_ref().unwrap_or(sender);
        if from != sender {
            if let Some(account) = self.storage.get_account(from) {
                if account
                    .features
                    .has_id(account::features::ledger::AccountLedger::ID)
                {
                    account.needs_role(
                        sender,
                        [account::Role::CanLedgerTransact, account::Role::Owner],
                    )?;
                } else {
                    return Err(error::unauthorized());
                }
            } else {
                return Err(error::unauthorized());
            }
        }

        self.storage.send(from, &to, &symbol, amount)?;
        Ok(EmptyReturn)
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

                // IdStore
                ("idstore.store".to_string(), EndpointInfo { is_command: true}),
                ("idstore.getFromRecallPhrase".to_string(), EndpointInfo { is_command: true}),
                ("idstore.getFromAddress".to_string(), EndpointInfo { is_command: true}),

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
    fn create(
        &mut self,
        sender: &Identity,
        args: account::CreateArgs,
    ) -> Result<account::CreateReturn, ManyError> {
        let account = account::Account::create(sender, args);

        // Verify that we support all features.
        validate_features_for_account(&account)?;

        let id = self.storage.add_account(account)?;
        Ok(account::CreateReturn { id })
    }

    fn set_description(
        &mut self,
        sender: &Identity,
        args: account::SetDescriptionArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let mut account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if account.has_role(sender, account::Role::Owner) {
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
        _sender: &Identity,
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
        sender: &Identity,
        args: account::AddRolesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let mut account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
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
        args: account::RemoveRolesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let mut account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
            return Err(account::errors::user_needs_role(account::Role::Owner));
        }
        for (id, roles) in args.roles {
            for r in roles {
                account.remove_role(&id, r);
            }
        }

        self.storage.commit_account(&args.account, account)?;
        Ok(EmptyReturn)
    }

    fn info(
        &self,
        _sender: &Identity,
        args: account::InfoArgs,
    ) -> Result<account::InfoReturn, ManyError> {
        let account::Account {
            description,
            roles,
            features,
        } = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        Ok(account::InfoReturn {
            description,
            roles,
            features,
        })
    }

    fn delete(
        &mut self,
        sender: &Identity,
        args: account::DeleteArgs,
    ) -> Result<EmptyReturn, ManyError> {
        let account = self
            .storage
            .get_account(&args.account)
            .ok_or_else(|| account::errors::unknown_account(args.account))?;

        if !account.has_role(sender, account::Role::Owner) {
            return Err(account::errors::user_needs_role(account::Role::Owner));
        }

        self.storage.delete_account(&args.account)?;
        Ok(EmptyReturn)
    }

    fn add_features(
        &mut self,
        _sender: &Identity,
        _args: account::AddFeaturesArgs,
    ) -> Result<EmptyReturn, ManyError> {
        Err(ManyError::unknown("Unsupported.".to_string()))
    }
}

impl multisig::AccountMultisigModuleBackend for LedgerModuleImpl {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Identity,
        arg: multisig::SubmitTransactionArgs,
    ) -> Result<multisig::SubmitTransactionReturn, ManyError> {
        let token = self.storage.create_multisig_transaction(sender, arg)?;
        Ok(multisig::SubmitTransactionReturn {
            token: ByteVec::from(token),
        })
    }

    fn multisig_info(
        &self,
        _sender: &Identity,
        args: multisig::InfoArgs,
    ) -> Result<multisig::InfoReturn, ManyError> {
        let info = self.storage.get_multisig_info(&args.token)?;
        Ok(info.info)
    }

    fn multisig_set_defaults(
        &mut self,
        sender: &Identity,
        args: multisig::SetDefaultsArgs,
    ) -> Result<multisig::SetDefaultsReturn, ManyError> {
        self.storage
            .set_multisig_defaults(sender, args)
            .map(|_| EmptyReturn)
    }

    fn multisig_approve(
        &mut self,
        sender: &Identity,
        args: multisig::ApproveArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .approve_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_revoke(
        &mut self,
        sender: &Identity,
        args: multisig::RevokeArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .revoke_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_execute(
        &mut self,
        sender: &Identity,
        args: multisig::ExecuteArgs,
    ) -> Result<ResponseMessage, ManyError> {
        self.storage.execute_multisig(sender, args.token.as_slice())
    }

    fn multisig_withdraw(
        &mut self,
        sender: &Identity,
        args: multisig::WithdrawArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .withdraw_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
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

impl IdStoreModuleBackend for LedgerModuleImpl {
    fn store(
        &mut self,
        sender: &Identity,
        StoreArgs {
            address,
            cred_id,
            public_key,
        }: StoreArgs,
    ) -> Result<StoreReturns, ManyError> {
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
        Ok(StoreReturns(recall_phrase))
    }

    fn get_from_recall_phrase(
        &self,
        args: GetFromRecallPhraseArgs,
    ) -> Result<GetReturns, ManyError> {
        let (cred_id, public_key) = self.storage.get_from_recall_phrase(&args.0)?;
        Ok(GetReturns {
            cred_id,
            public_key,
        })
    }

    fn get_from_address(&self, args: GetFromAddressArgs) -> Result<GetReturns, ManyError> {
        let (cred_id, public_key) = self.storage.get_from_address(&args.0)?;
        Ok(GetReturns {
            cred_id,
            public_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use many::{
        server::module::idstore::{CredentialId, PublicKey},
        types::identity::{cose::testsutils::generate_random_eddsa_identity, CoseKeyIdentity},
    };
    use minicbor::bytes::ByteVec;

    fn setup() -> (
        CoseKeyIdentity,
        CredentialId,
        tempfile::TempDir,
        Option<InitialStateJson>,
    ) {
        let id = generate_random_eddsa_identity();
        let cred_id = CredentialId(ByteVec::from(Vec::from([1; 16])));
        let persistent = tempfile::tempdir().unwrap();

        let content = std::fs::read_to_string("../../staging/ledger_state.json").unwrap();
        let initial_state: InitialStateJson = serde_json::from_str(&content).unwrap();

        (id, cred_id, persistent, Some(initial_state))
    }

    #[test]
    fn idstore_store() {
        let (id, cred_id, persistent, initial_state) = setup();
        let public_key = id.key.unwrap().to_vec().unwrap();
        let mut module_impl = LedgerModuleImpl::new(initial_state, persistent, false).unwrap();

        // Basic call
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 2);

        // Make sure another call provides a different result
        let result2 = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );
        assert!(result2.is_ok());
        let rp2 = result2.unwrap().0;
        assert_eq!(rp2.len(), 2);
        assert_ne!(rp, rp2);

        // Generate the first 8 recall phrase
        for _ in 2..8 {
            let result3 = module_impl.store(
                &id.identity,
                StoreArgs {
                    address: id.identity,
                    cred_id: cred_id.clone(),
                    public_key: PublicKey(public_key.clone().into()),
                },
            );
            assert!(result3.is_ok());
        }

        // And reset the seed 0
        module_impl.storage.set_idstore_seed(0);

        // This should trigger the `recall_phrase_generation_failed()` exception
        let result4 = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );
        assert!(result4.is_err());
        assert_eq!(
            result4.unwrap_err().code,
            idstore::recall_phrase_generation_failed().code
        );

        // Generate a 3-words phrase
        module_impl.storage.set_idstore_seed(0x10000);
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 3);

        // Generate a 4-words phrase
        module_impl.storage.set_idstore_seed(0x1000000);
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 4);

        // Generate a 5-words phrase
        module_impl.storage.set_idstore_seed(0x100000000);
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id,
                public_key: PublicKey(public_key.into()),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 5);
    }

    #[test]
    fn idstore_store_anon() {
        let (id, _, persistent, initial_state) = setup();
        let public_key = id.key.unwrap().to_vec().unwrap();
        let mut module_impl = LedgerModuleImpl::new(initial_state, persistent, false).unwrap();

        let cred_id = CredentialId(ByteVec::from(Vec::from([1; 15])));
        let result = module_impl.store(
            &Identity::anonymous(),
            StoreArgs {
                address: id.identity,
                cred_id,
                public_key: PublicKey(public_key.into()),
            },
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ManyError::invalid_identity().code);
    }

    #[test]
    fn idstore_invalid_cred_id() {
        let (id, _, persistent, initial_state) = setup();
        let public_key = id.key.unwrap().to_vec().unwrap();
        let mut module_impl = LedgerModuleImpl::new(initial_state, persistent, false).unwrap();

        let cred_id = CredentialId(ByteVec::from(Vec::from([1; 15])));
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id,
                public_key: PublicKey(public_key.clone().into()),
            },
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code,
            idstore::invalid_credential_id("".to_string()).code
        );

        let cred_id = CredentialId(ByteVec::from(Vec::from([1; 1024])));
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id,
                public_key: PublicKey(public_key.into()),
            },
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code,
            idstore::invalid_credential_id("".to_string()).code
        );
    }

    #[test]
    fn idstore_get_from_recall_phrase() {
        let (id, cred_id, persistent, initial_state) = setup();
        let public_key = id.key.unwrap().to_vec().unwrap();
        let mut module_impl = LedgerModuleImpl::new(initial_state, persistent, false).unwrap();
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );

        assert!(result.is_ok());
        let store_return = result.unwrap();

        let result = module_impl.get_from_recall_phrase(GetFromRecallPhraseArgs(store_return.0));
        assert!(result.is_ok());
        let get_returns = result.unwrap();

        assert_eq!(get_returns.cred_id, cred_id);
        assert_eq!(get_returns.public_key.0.to_vec(), public_key);
    }

    #[test]
    fn idstore_get_from_address() {
        let (id, cred_id, persistent, initial_state) = setup();
        let public_key = id.key.unwrap().to_vec().unwrap();
        let mut module_impl = LedgerModuleImpl::new(initial_state, persistent, false).unwrap();
        let result = module_impl.store(
            &id.identity,
            StoreArgs {
                address: id.identity,
                cred_id: cred_id.clone(),
                public_key: PublicKey(public_key.clone().into()),
            },
        );

        assert!(result.is_ok());

        let result = module_impl.get_from_address(GetFromAddressArgs(id.identity));
        assert!(result.is_ok());
        let get_returns = result.unwrap();

        assert_eq!(get_returns.cred_id, cred_id);
        assert_eq!(get_returns.public_key.0.to_vec(), public_key);
    }
}
