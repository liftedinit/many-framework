use many_modules::ledger;
use many_modules::ledger::LedgerModuleBackend;
use proptest::prelude::*;
use test_utils::*;

#[test]
fn info() {
    let Setup {
        module_impl, id, ..
    } = setup();
    let result = module_impl.info(&id, ledger::InfoArgs {});
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
        module_impl.set_balance_only_for_testing(id, amount, *MFX_SYMBOL).expect("Unable to set balance for testing");
        verify_balance(&module_impl, id, *MFX_SYMBOL, amount.into());
    }
}
