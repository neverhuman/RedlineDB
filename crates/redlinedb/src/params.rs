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

macro_rules! tuple_params {
    ($($name:ident),+) => {
        impl<$($name),+> Params for ($($name,)+)
        where
            $($name: Into<Value>,)+
        {
            #[allow(non_snake_case)]
            fn bind_into(self, stmt: &mut crate::Statement<'_>) -> Result<()> {
                stmt.clear_bindings();
                let ($($name,)+) = self;
                let mut index = 1usize;
                $(
                    stmt.bind_value(index, $name.into())?;
                    #[allow(unused_assignments)]
                    { index += 1; }
                )+
                Ok(())
            }
        }
    };
}

tuple_params!(A);
tuple_params!(A, B);
tuple_params!(A, B, C);
tuple_params!(A, B, C, D);
tuple_params!(A, B, C, D, E);
tuple_params!(A, B, C, D, E, F);
tuple_params!(A, B, C, D, E, F, G);
tuple_params!(A, B, C, D, E, F, G, H);
tuple_params!(A, B, C, D, E, F, G, H, I);
tuple_params!(A, B, C, D, E, F, G, H, I, J);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K, L);

#[macro_export]
macro_rules! params {
    () => {
        ::std::vec::Vec::<$crate::Value>::new()
    };
    ($($value:expr),+ $(,)?) => {
        ::std::vec![ $( $crate::Value::from($value) ),+ ]
    };
}
