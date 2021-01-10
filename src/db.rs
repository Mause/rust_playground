use tokio_postgres::types::FromSql;
use bytes::{Buf, BufMut, BytesMut};
use tokio_postgres::types::{IsNull, ToSql, Type};

#[derive(Debug)]
pub struct U64 {
    pub item: u64,
}

impl ToSql for U64 {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<
        IsNull,
        Box<(dyn std::error::Error + Sync + std::marker::Send + 'static)>,
    > {
        out.put_u64(self.item);
        Ok(IsNull::No)
    }
    fn to_sql_checked(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<
        IsNull,
        Box<(dyn std::error::Error + Sync + std::marker::Send + 'static)>,
    > {
        out.put_u64(self.item);
        Ok(IsNull::No)
    }
    fn accepts(_ty: &Type) -> bool {
        return true;
    }
}

impl FromSql<'_> for U64 {
    fn from_sql<'a>(
        _: &tokio_postgres::types::Type,
        array: &'a [u8],
    ) -> std::result::Result<
        Self,
        std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>,
    > {
        let mut copy = array;
        Ok(U64 {
            item: copy.get_u64(),
        })
    }
    fn accepts(_: &tokio_postgres::types::Type) -> bool {
        true
    }
}

use tokio_pg_mapper_derive::PostgresMapper;

#[derive(PostgresMapper, Debug)]
#[pg_mapper(table = "location")]
pub struct Location {
    pub id: i32,
    pub guild_id: U64,
    pub member_id: U64,
    pub location: String,
}
