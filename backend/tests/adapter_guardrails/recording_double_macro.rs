//! Declarative macro for generating recording test doubles.
//!
//! This macro standardises the boilerplate for recording calls and returning
//! configured responses in adapter guardrails tests.

macro_rules! recording_double {
    (
        $(#[$enum_meta:meta])*
        $enum_vis:vis enum $response_enum:ident {
            $ok_variant:ident($ok_type:ty),
            $err_variant:ident($err_type:ty) $(,)?
        }

        $(#[$struct_meta:meta])*
        $struct_vis:vis struct $struct_name:ident {
            calls: $call_type:ty,
            trait: $trait_name:path,
            method: $method_name:ident ( &self $(, $arg_name:ident : $arg_ty:ty )* )
                -> Result<$method_ok:ty, $method_err:ty>,
            record: $record_expr:expr,
            calls_lock: $calls_lock:literal,
            response_lock: $response_lock:literal $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Clone)]
        $enum_vis enum $response_enum {
            $ok_variant($ok_type),
            $err_variant($err_type),
        }

        #[derive(Clone)]
        $struct_vis struct $struct_name {
            calls: std::sync::Arc<std::sync::Mutex<Vec<$call_type>>>,
            response: std::sync::Arc<std::sync::Mutex<$response_enum>>,
        }

        impl $struct_name {
            $struct_vis fn new(response: $response_enum) -> Self {
                Self {
                    calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                    response: std::sync::Arc::new(std::sync::Mutex::new(response)),
                }
            }

            $struct_vis fn calls(&self) -> Vec<$call_type> {
                self.calls.lock().expect($calls_lock).clone()
            }

            $struct_vis fn set_response(&self, response: $response_enum) {
                *self.response.lock().expect($response_lock) = response;
            }
        }

        #[async_trait::async_trait]
        impl $trait_name for $struct_name {
            async fn $method_name(
                &self $(, $arg_name: $arg_ty )*
            ) -> Result<$method_ok, $method_err> {
                self.calls
                    .lock()
                    .expect($calls_lock)
                    .push($record_expr);
                match self.response.lock().expect($response_lock).clone() {
                    $response_enum::$ok_variant(response) => Ok(response),
                    $response_enum::$err_variant(error) => Err(error),
                }
            }
        }
    };
}

pub(crate) use recording_double;
