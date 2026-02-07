//! Shared macros for batch upsert ingestion repositories.

/// Generate batched upsert trait methods for ingestion repositories.
///
/// The generated methods:
/// - short-circuit on empty input
/// - acquire a pooled connection
/// - convert ingestion records to Diesel insert rows via `From`
/// - execute a single batched `INSERT .. ON CONFLICT .. DO UPDATE` statement
#[macro_export]
macro_rules! impl_upsert_methods {
    (
        impl $trait:ident for $repo:ty {
            error: $error_ty:ty,
            map_pool_error: $map_pool_error:path,
            map_diesel_error: $map_diesel_error:path,
            pool: $pool_field:ident,
            methods: [
                $((
                    $method_name:ident,
                    $ingestion_type:ty,
                    $row_type:ty,
                    $table:ident,
                    [$($field:ident),+ $(,)?]
                )),+ $(,)?
            ],
            keep: { $($keep:tt)* }
        }
    ) => {
        #[async_trait::async_trait]
        impl $trait for $repo {
            $(
                async fn $method_name(
                    &self,
                    records: &[$ingestion_type],
                ) -> Result<(), $error_ty> {
                    if records.is_empty() {
                        return Ok(());
                    }
                    let mut conn = self.$pool_field.get().await.map_err($map_pool_error)?;
                    let rows: Vec<$row_type> = records.iter().map(<$row_type>::from).collect();
                    diesel::insert_into($table::table)
                        .values(&rows)
                        .on_conflict($table::id)
                        .do_update()
                        .set((
                            $(
                                $table::$field.eq(diesel::upsert::excluded($table::$field)),
                            )+
                        ))
                        .execute(&mut conn)
                        .await
                        .map_err($map_diesel_error)?;
                    Ok(())
                }
            )+

            $($keep)*
        }
    };
}
