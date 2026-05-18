use core::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
        #[repr(transparent)]
        pub struct $name(pub u64);

        impl $name {
            pub const ZERO: Self = Self(0);

            #[inline]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[inline]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

id_type!(ObjectId);
id_type!(SchemaId);
id_type!(TableId);
id_type!(ColumnId);
id_type!(IndexId);
id_type!(ConstraintId);
id_type!(RelationId);
