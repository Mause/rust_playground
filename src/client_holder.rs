use ::google_maps::prelude::ClientSettings;
use deadpool_postgres::Manager;
use deadpool_postgres::Pool;
use serenity::client::Context;
use serenity::prelude::TypeMapKey;

pub struct ClientHolder {
    pub client: Pool,
}

impl TypeMapKey for ClientHolder {
    type Value = ClientHolder;
}

pub struct MapsClientHolder {
    pub maps_client: ClientSettings,
}

impl TypeMapKey for MapsClientHolder {
    type Value = MapsClientHolder;
}

pub async fn read_maps_client(ctx: &Context) -> ClientSettings {
    let map_guard = ctx.data.read().await;

    map_guard
        .get::<MapsClientHolder>()
        .expect("Missing Client")
        .maps_client
        .clone()
}

pub async fn read_client(ctx: &Context) -> deadpool::managed::Object<Manager> {
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
