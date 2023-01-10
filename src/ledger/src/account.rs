use clap::Parser;
use many_identity::Address;
use many_modules::account::{features, Role};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Parser)]
pub struct CommandOpt {
    #[clap(subcommand)]
    /// Account subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    Create(CreateOpt),
    // SetDescription(SetDescriptionOpt),
    // ListRoles(ListRolesOpt),
    // GetRoles(GetRolesOpt),
    // AddRoles(AddRolesOpt),
    // RemoveRoles(RemoveRolesOpt),
    // Into(IntoOpt),
    // Disable(DisableOpt),
    // AddFeatures(AddFeaturesOpt),
}

#[derive(Parser)]
struct CreateOpt {
    description: Option<String>,
    roles: Option<BTreeMap<Address, BTreeSet<Role>>>,
    features: features::FeatureSet,
}
