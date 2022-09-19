use super::{error, KvStoreMetadata, KvStoreModuleImpl};
use coset::CoseSign1;
use many_error::{ManyError, ManyErrorCode};
use many_identity::Address;
use many_modules::account::features::{FeatureInfo, TryCreateFeature};
use many_modules::account::{AccountModuleBackend, Role};
use many_modules::{account, EmptyReturn, ManyModule, ManyModuleInfo};
use many_protocol::{RequestMessage, ResponseMessage};
use many_types::cbor::CborAny;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Formatter};

pub(crate) fn validate_account(account: &account::Account) -> Result<(), ManyError> {
    // Verify that we support all features.
    validate_features_for_account(account)?;

    // Verify the roles are supported by the features
    validate_roles_for_account(account)?;

    Ok(())
}

fn validate_features_for_account(account: &account::Account) -> Result<(), ManyError> {
    let features = account.features();

    // TODO: somehow keep this list updated with the above.
    if let Err(e) = features.get::<account::features::kvstore::AccountKvStore>() {
        if e.code() != ManyErrorCode::AttributeNotFound {
            return Err(e);
        }
    }

    Ok(())
}

fn validate_roles_for_account(account: &account::Account) -> Result<(), ManyError> {
    let features = account.features();

    let mut allowed_roles = BTreeSet::from([account::Role::Owner]);
    let mut account_roles = BTreeSet::<account::Role>::new();
    for (_, r) in account.roles.iter() {
        account_roles.extend(r.iter())
    }

    // TODO: somehow keep this list updated with the above.
    if features
        .get::<account::features::kvstore::AccountKvStore>()
        .is_ok()
    {
        allowed_roles.append(&mut account::features::kvstore::AccountKvStore::roles());
    }

    for r in account_roles {
        if !allowed_roles.contains(&r) {
            return Err(account::errors::unknown_role(r.to_string()));
        }
    }

    Ok(())
}

fn get_roles_for_account(account: &account::Account) -> BTreeSet<account::Role> {
    let features = account.features();

    let mut roles = BTreeSet::new();

    // TODO: somehow keep this list updated with the below.
    if features.has_id(account::features::kvstore::AccountKvStore::ID) {
        roles.append(&mut account::features::kvstore::AccountKvStore::roles());
    }

    roles
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

impl AccountModuleBackend for KvStoreModuleImpl {
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
            return Err(account::errors::user_needs_role(Role::Owner));
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

        if !account.has_role(sender, Role::Owner) {
            return Err(account::errors::user_needs_role(Role::Owner));
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

        account.needs_role(sender, [Role::Owner])?;
        self.storage.add_features(account, args)?;
        Ok(EmptyReturn)
    }
}

impl KvStoreModuleImpl {
    /// Verify the alternative owner is supported
    /// Verify the sender has the rights to use this alternative owner address
    pub(crate) fn validate_alternative_owner<R: TryInto<Role> + std::fmt::Display + Copy>(
        &self,
        sender: &Address,
        alternative_owner: &Address,
        roles: impl IntoIterator<Item = R>,
    ) -> Result<(), ManyError> {
        if let Some(account) = self.storage.get_account(alternative_owner) {
            account.needs_role(sender, roles)
        } else if alternative_owner.is_subresource() {
            Err(error::subres_alt_unsupported())
        } else if alternative_owner.is_anonymous() {
            Err(error::anon_alt_denied())
        } else {
            Err(error::permission_denied())
        }
    }

    pub(crate) fn can_write(&self, sender: &Address, key: Vec<u8>) -> Result<(), ManyError> {
        self.verify_acl(sender, key)
    }

    pub(crate) fn can_disable(&self, sender: &Address, key: Vec<u8>) -> Result<(), ManyError> {
        self.verify_acl(sender, key)
    }

    /// Verify if user is permitted to access the value at the given key
    fn verify_acl(&self, sender: &Address, key: Vec<u8>) -> Result<(), ManyError> {
        // Get ACL, if it exists
        if let Some(acl_cbor) = self.storage.get_metadata(&key)? {
            // Decode ACL
            let acl: KvStoreMetadata = minicbor::decode(&acl_cbor)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

            if &acl.owner == sender {
                return Ok(());
            }

            return Err(error::permission_denied());
        }
        Ok(())
    }
}
