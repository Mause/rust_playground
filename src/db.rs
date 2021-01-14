use bytes::{Buf, BufMut, BytesMut};
use deadpool_postgres::config::SslMode;
use deadpool_postgres::tokio_postgres;
use deadpool_postgres::{Config, Pool};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use std::env;
use std::str::{from_utf8, FromStr};
use tokio_pg_mapper_derive::PostgresMapper;
use tokio_postgres::config::Host;
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

fn convert_config(main_config: tokio_postgres::Config) -> Config {
    let mut config = Config::new();
    config.dbname = main_config.get_dbname().map(|a| a.to_string());
    config.host = match &main_config.get_hosts()[0] {
        Host::Tcp(u) => Some(u.to_string()),
        #[cfg(unix)]
        Host::Unix(u) => panic!("Invalid state")
    };
    config.user = main_config.get_user().map(|a| a.to_string());
    config.ssl_mode = Some(SslMode::Require);
    config.password = Some(
        from_utf8(main_config.get_password().unwrap())
            .unwrap()
            .to_string(),
    );
    config
}

pub async fn connect_to_postgres() -> Result<Pool, Box<dyn std::error::Error>> {
    let database_url = &env::var("DATABASE_URL").expect("database_url");

    let na = TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let connector = MakeTlsConnector::new(na);

    let config = convert_config(tokio_postgres::Config::from_str(database_url).unwrap());

    let pool = config.create_pool(connector).unwrap();

    Ok(pool)
}

#[derive(Debug)]
pub struct U64 {
    pub item: u64,
}

impl U64 {
    pub fn new(i: u64) -> U64 {
        U64 { item: i }
    }
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
        true
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

#[derive(PostgresMapper, Debug)]
#[pg_mapper(table = "location")]
pub struct Location {
    pub id: i32,
    pub guild_id: U64,
    pub member_id: U64,
    pub location: String,
}
