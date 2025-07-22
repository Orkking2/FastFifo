macro_rules! impl_index_enum {
    ($vis:vis $name:ident { $($variant:ident = $value:expr),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(usize)]
        $vis enum $name {
            $($variant = $value),*
        }

        impl From<$name> for usize {
            fn from(val: $name) -> usize {
                val as usize
            }
        }

        impl TryFrom<usize> for $name {
            type Error = ();

            fn try_from(value: usize) -> Result<Self, Self::Error> {
                match value {
                    $(x if x == $name::$variant as usize => Ok($name::$variant),)*
                    _ => Err(()),
                }
            }
        }
    };
}

impl_index_enum! {
    pub Layer {
        Producer = 0,
        Transformer = 1,
        Consumer = 2,
    }
}

impl Layer {
    /// Which atom pair does this pair chase
    ///
    /// Producers chase consumers
    ///
    /// Transformers chase producers
    ///
    /// Consumers chase transformers
    pub const fn chasing(self) -> Self {
        match self {
            Layer::Producer => Layer::Consumer,
            Layer::Transformer => Layer::Producer,
            Layer::Consumer => Layer::Transformer,
        }
    }
}
