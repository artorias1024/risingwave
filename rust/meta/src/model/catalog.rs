use prost::Message;
use risingwave_common::error::Result;
use risingwave_pb::meta::{
    Catalog as ProstCatalog, Database as ProstDatabase, Schema as ProstSchema, Table as ProstTable,
};
use risingwave_pb::plan::{DatabaseRefId, SchemaRefId, TableRefId};

use crate::manager::Epoch;
use crate::model::MetadataModel;
use crate::storage::MetaStoreRef;

/// Column family name for table.
const TABLE_CF_NAME: &str = "cf/table";
/// Column family name for schema.
const SCHEMA_CF_NAME: &str = "cf/schema";
/// Column family name for database.
const DATABASE_CF_NAME: &str = "cf/database";

pub struct Database(ProstDatabase);
pub struct Schema(ProstSchema);
/// TODO: for mv on mv, we still need to record mv dependency, as `ref_count`.
pub struct Table(ProstTable);
pub struct Catalog(ProstCatalog);

macro_rules! impl_model_for_catalog {
    ($name:ident, $cf:ident, $prost_ty:ty, $key_ty:ty, $key_fn:ident) => {
        impl MetadataModel for $name {
            type ProstType = $prost_ty;
            type KeyType = $key_ty;

            fn cf_name() -> String {
                $cf.to_string()
            }

            fn to_protobuf(&self) -> Self::ProstType {
                self.0.clone()
            }

            fn from_protobuf(prost: Self::ProstType) -> Self {
                Self(prost)
            }

            fn key(&self) -> Result<Self::KeyType> {
                Ok(self.0.$key_fn()?.clone())
            }

            fn version(&self) -> Epoch {
                Epoch::from(self.0.version)
            }
        }
    };
}

impl_model_for_catalog!(
    Database,
    DATABASE_CF_NAME,
    ProstDatabase,
    DatabaseRefId,
    get_database_ref_id
);
impl_model_for_catalog!(
    Schema,
    SCHEMA_CF_NAME,
    ProstSchema,
    SchemaRefId,
    get_schema_ref_id
);
impl_model_for_catalog!(
    Table,
    TABLE_CF_NAME,
    ProstTable,
    TableRefId,
    get_table_ref_id
);

impl Catalog {
    pub async fn get(store: &MetaStoreRef) -> Result<Self> {
        let catalog_pb = store
            .list_batch_cf(vec![DATABASE_CF_NAME, SCHEMA_CF_NAME, TABLE_CF_NAME])
            .await?;
        assert_eq!(catalog_pb.len(), 3);

        Ok(Catalog(ProstCatalog {
            databases: catalog_pb
                .get(0)
                .unwrap()
                .iter()
                .map(|d| ProstDatabase::decode(d.as_slice()).unwrap())
                .collect::<Vec<_>>(),
            schemas: catalog_pb
                .get(1)
                .unwrap()
                .iter()
                .map(|d| ProstSchema::decode(d.as_slice()).unwrap())
                .collect::<Vec<_>>(),
            tables: catalog_pb
                .get(2)
                .unwrap()
                .iter()
                .map(|d| ProstTable::decode(d.as_slice()).unwrap())
                .collect::<Vec<_>>(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use futures::future;

    use super::*;
    use crate::manager::MetaSrvEnv;

    #[tokio::test]
    async fn test_database() -> Result<()> {
        let store = &MetaSrvEnv::for_test().await.meta_store_ref();
        let databases = Database::list(store).await?;
        assert!(databases.is_empty());
        assert!(
            Database::select(store, &DatabaseRefId { database_id: 0 }, Epoch::from(0))
                .await
                .is_err()
        );

        future::join_all((0..100).map(|i| async move {
            Database::from_protobuf(ProstDatabase {
                database_ref_id: Some(DatabaseRefId { database_id: i }),
                database_name: format!("database_{}", i),
                version: i as u64,
            })
            .create(store)
            .await
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
        for i in 0..100 {
            let database = Database::select(
                store,
                &DatabaseRefId {
                    database_id: i as i32,
                },
                Epoch::from(i as u64),
            )
            .await?;
            assert_eq!(
                database.to_protobuf(),
                ProstDatabase {
                    database_ref_id: Some(DatabaseRefId { database_id: i }),
                    database_name: format!("database_{}", i),
                    version: i as u64,
                }
            );
        }

        Database::from_protobuf(ProstDatabase {
            database_ref_id: Some(DatabaseRefId { database_id: 0 }),
            database_name: "database_0".to_string(),
            version: 101,
        })
        .create(store)
        .await?;

        let databases = Database::list(store).await?;
        assert_eq!(databases.len(), 100);

        for i in 0..100 {
            assert!(Database::delete(store, &DatabaseRefId { database_id: i })
                .await
                .is_ok());
        }
        let databases = Database::list(store).await?;
        assert!(databases.is_empty());

        Ok(())
    }
}
