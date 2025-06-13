// src/shared_types.rs

/// Represents the type of the option.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum OptionType {
    Call,
    Put,
}
