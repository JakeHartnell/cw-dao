use cosmwasm_schema::cw_serde;

#[cw_serde]
pub enum Update<T> {
    Set(T),
    Clear,
}

/// An update type that allows partial updates of optional fields.
#[cw_serde]
pub struct OptionalUpdate<T>(pub Option<Update<T>>);

impl<T> OptionalUpdate<T> {
    /// Updates the value if it exists, otherwise does nothing.
    pub fn maybe_update(self, update: impl FnOnce(Option<T>)) {
        match self.0 {
            Some(Update::Set(value)) => update(Some(value)),
            Some(Update::Clear) => update(None),
            None => (),
        }
    }

    /// Updates the value if it exists, otherwise does nothing, requiring the
    /// update action to return a result.
    pub fn maybe_update_result<E>(
        self,
        update: impl FnOnce(Option<T>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self.0 {
            Some(Update::Set(value)) => update(Some(value)),
            Some(Update::Clear) => update(None),
            None => Ok(()),
        }
    }
}
