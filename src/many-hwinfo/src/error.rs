use many::define_attribute_many_error;

define_attribute_many_error!(
    attribute 3 => {
        1: pub fn unauthorized() => "Identity unauthorized to do this operation.",
        5: pub fn invalid_initial_state(expected, actual)
            => "Invalid initial state hash. Expected '{expected}', was '{actual}'.",    }
);
