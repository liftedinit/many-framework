use many_error::define_attribute_many_error;

define_attribute_many_error!(
    attribute 3 => {
        1: pub fn permission_denied() => "You do not have the authorization to modify this key.",
        2: pub fn invalid_initial_hash(expected, actual)
            => "Invalid initial hash. Expected '{expected}', was '{actual}'.",
        3: pub fn key_disabled() => "The key was disabled by its owner.",
        4: pub fn anon_alt_denied() => "Anonymous alternative owner denied.",
        5: pub fn subres_alt_unsupported() => "Subresource alternative owner unsupported.",
        6: pub fn key_not_found() => "Key not found.",
    }
);
