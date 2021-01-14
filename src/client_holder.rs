use deadpool_postgres::{ClientWrapper, Pool};
use serenity::client::Context;
use serenity::prelude::TypeMapKey;

pub struct ClientHolder {
    pub client: Pool,
}

impl TypeMapKey for ClientHolder {
    type Value = ClientHolder;
}

pub async fn read_client(
    ctx: &Context,
) -> deadpool::managed::Object<ClientWrapper, tokio_postgres::Error> {
    let map_guard = ctx.data.read().await;

    map_guard
        .get::<ClientHolder>()
        .expect("Missing Client")
        .client
        .clone()
        .get()
        .await
        .unwrap()
}
