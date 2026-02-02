//! Defines helper macros for generating domain port error enums.

macro_rules! define_port_error {
    (@ctor $variant:ident) => {
        ::paste::paste! {
            pub fn [<$variant:snake>]() -> Self {
                Self::$variant
            }
        }
    };

    (@ctor $variant:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        define_port_error!(@ctor_impl $variant () () $( $field : $ty, )*);
    };

    (@ctor_impl $variant:ident ($($params:tt)*) ($($inits:tt)*) ) => {
        ::paste::paste! {
            pub fn [<$variant:snake>]($($params)*) -> Self {
                Self::$variant { $($inits)* }
            }
        }
    };

    (@ctor_impl $variant:ident ($($params:tt)*) ($($inits:tt)*) $field:ident : $ty:ty, $($rest:tt)*) => {
        define_port_error!(
            @ctor_impl
            $variant
            ($($params)* $field: impl Into<$ty>,)
            ($($inits)* $field: $field.into(),)
            $($rest)*
        );
    };
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
                define_port_error!(@ctor $variant $( { $($field : $ty),* } )?);
            )*
        }
    };
}

pub(crate) use define_port_error;

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    define_port_error! {
        pub enum ExamplePortError {
            Foo { message: String } => "foo: {message}",
            Bar { count: u32 } => "bar: {count}",
            Baz { message: String, count: u32 } => "baz: {message} ({count})",
        }
    }

    #[test]
    fn constructors_accept_str_for_string_fields() {
        let err = ExamplePortError::foo("hello");
        assert_eq!(err.to_string(), "foo: hello");
    }

    #[test]
    fn constructors_preserve_non_string_types() {
        let err = ExamplePortError::bar(42_u32);
        assert_eq!(err.to_string(), "bar: 42");
    }

    #[test]
    fn constructors_support_mixed_fields() {
        let err = ExamplePortError::baz("hello", 42_u32);
        assert_eq!(err.to_string(), "baz: hello (42)");
    }
}
