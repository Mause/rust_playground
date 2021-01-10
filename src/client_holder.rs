use tokio_postgres::Client;
use typemap_rev::TypeMapKey;

pub struct ClientHolder {
    pub client: Client,
}

impl TypeMapKey for ClientHolder {
    type Value = ClientHolder;
}
