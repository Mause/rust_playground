use serenity::client::Context;
use std::ops::Deref;
use tokio::sync::RwLockReadGuard;
use tokio_postgres::Client;
use typemap_rev::TypeMap;
use typemap_rev::TypeMapKey;

pub struct ClientHolder {
    pub client: Client,
}

impl TypeMapKey for ClientHolder {
    type Value = ClientHolder;
}

pub async fn read_client(ctx: &Context) -> ClientReadGuard<'_> {
    let map_guard = ctx.data.read().await;
    ClientReadGuard(map_guard)
}

pub struct ClientReadGuard<'guard>(RwLockReadGuard<'guard, TypeMap>);

impl Deref for ClientReadGuard<'_> {
    type Target = Client;

    fn deref(&self) -> &Client {
        &self.0.get::<ClientHolder>().expect("Missing Client").client
    }
}
