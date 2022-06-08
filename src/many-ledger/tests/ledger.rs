pub mod common;
use common::*;
use many::server::module::ledger::LedgerModuleBackend;
use many::server::module::{self};
use proptest::prelude::*;

#[test]
fn info() {
    let Setup {
        module_impl, id, ..
    } = setup();
    let result = module_impl.info(&id, module::ledger::InfoArgs {});
    assert!(result.is_ok());
}

proptest! {
    #[test]
    fn balance(amount in any::<u64>()) {
        let Setup {
            mut module_impl,
            id,
            ..
        } = setup();
        module_impl.set_balance_only_for_testing(id, amount, *MFX_SYMBOL);
        verify_balance(&module_impl, id, *MFX_SYMBOL, amount.into());
    }
}
