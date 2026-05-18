use crate::error::Result;
use crate::value::Value;

pub trait Params {
    fn bind_into(self, stmt: &mut crate::Statement<'_>) -> Result<()>;
}

impl Params for () {
    fn bind_into(self, _stmt: &mut crate::Statement<'_>) -> Result<()> {
        Ok(())
    }
}

impl Params for Vec<Value> {
    fn bind_into(self, stmt: &mut crate::Statement<'_>) -> Result<()> {
        stmt.clear_bindings();
        for (index, value) in self.into_iter().enumerate() {
            stmt.bind_value(index + 1, value)?;
        }
        Ok(())
    }
}

impl Params for &[Value] {
    fn bind_into(self, stmt: &mut crate::Statement<'_>) -> Result<()> {
        stmt.clear_bindings();
        for (index, value) in self.iter().cloned().enumerate() {
            stmt.bind_value(index + 1, value)?;
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! params {
    () => {
        ::std::vec::Vec::<$crate::Value>::new()
    };
    ($($value:expr),+ $(,)?) => {
        ::std::vec![ $( $crate::Value::from($value) ),+ ]
    };
}
