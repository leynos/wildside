//! Defines helper macros for generating domain port error enums.

macro_rules! define_port_error {
    (
        $(#[$outer:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident $( { $($field:ident : $ty:ty),* $(,)? } )? => $message:expr
            ),* $(,)?
        }
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[error($message)]
                $variant $( { $($field : $ty),* } )?,
            )*
        }

        impl $name {
            $(
                ::paste::paste! {
                    pub fn [<$variant:snake>]( $( $($field : $ty),* )? ) -> Self {
                        Self::$variant $( { $($field),* } )?
                    }
                }
            )*
        }
    };
}

pub(crate) use define_port_error;
