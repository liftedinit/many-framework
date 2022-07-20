use many_error::define_attribute_many_error;

define_attribute_many_error!(
    attribute 2 => {
        1: pub fn unknown_symbol(symbol) => "Symbol not supported by this ledger: {symbol}.",
        2: pub fn unauthorized() => "Unauthorized to do this operation.",
        3: pub fn insufficient_funds() => "Insufficient funds.",
        4: pub fn anonymous_cannot_hold_funds() => "Anonymous is not a valid account identity.",
        5: pub fn invalid_initial_state(expected, actual)
            => "Invalid initial state hash. Expected '{expected}', was '{actual}'.",
        6: pub fn unexpected_subresource_id(expected, actual)
            => "Invalid initial state account subresource_id. Expected '{expected}', was '{actual}'.",
        7: pub fn unexpected_account_id(expected, actual)
            => "Invalid initial state account id. Expected '{expected}', was '{actual}'.",
        8: pub fn destination_is_source()
            => "Unable to send tokens to a destination (to) that is the same as the source (from).",
        9: pub fn amount_is_zero()
            => "Unable to send zero (0) token.",
    }
);
