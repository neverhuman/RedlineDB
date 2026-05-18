macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    };
}

id_type!(PageId);
id_type!(RelId);
id_type!(BlockNo);
id_type!(Lsn);
id_type!(TxId);
id_type!(Csn);
id_type!(RowId);
id_type!(UndoPtr);
id_type!(DbId);
id_type!(TimelineId);
id_type!(WalSegmentNo);
id_type!(ReplicationSlotId);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BackupId(pub u128);

impl BackupId {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn get(self) -> u128 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageGeneration(pub u32);

impl PageGeneration {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1);

    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TuplePtr {
    pub page_id: PageId,
    pub slot: u16,
    pub generation: PageGeneration,
}

impl TuplePtr {
    pub const NULL: Self = Self {
        page_id: PageId::ZERO,
        slot: u16::MAX,
        generation: PageGeneration::ZERO,
    };

    #[inline]
    pub const fn new(page_id: PageId, slot: u16) -> Self {
        Self {
            page_id,
            slot,
            generation: PageGeneration::ONE,
        }
    }

    #[inline]
    pub const fn new_with_generation(
        page_id: PageId,
        slot: u16,
        generation: PageGeneration,
    ) -> Self {
        Self {
            page_id,
            slot,
            generation,
        }
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.page_id.0 == 0 && self.slot == u16::MAX
    }
}

impl Default for PageGeneration {
    fn default() -> Self {
        Self::ONE
    }
}

impl Default for TuplePtr {
    fn default() -> Self {
        Self::NULL
    }
}
