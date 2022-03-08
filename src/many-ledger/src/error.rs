use many::define_attribute_many_error;

define_attribute_many_error!(
    attribute 2 => {
        1: pub fn unknown_symbol(symbol) => "Symbol not supported by this ledger: {symbol}.",
        2: pub fn unauthorized() => "Unauthorized to do this operation.",
        3: pub fn insufficient_funds() => "Insufficient funds.",
        4: pub fn anonymous_cannot_hold_funds() => "Anonymous is not a valid account identity.",
        5: pub fn invalid_initial_state(expected, actual)
            => "Invalid initial state hash. Expected '{expected}', was '{actual}'.",
    }
);
