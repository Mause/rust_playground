use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use serenity::model::channel::Message;
use std::env;
use tokio_postgres::Socket;
use typemap_rev::TypeMapKey;
extern crate dotenv;
use dotenv::dotenv;

struct Handler;

#[group]
#[commands(ping, store)]
struct General;

#[async_trait]
impl EventHandler for Handler {}

async fn connect_to_postgres() -> Result<tokio_postgres::Client, Box<dyn std::error::Error>> {
    let database_url = &env::var("DATABASE_URL").expect("database_url");

    let na = TlsConnector::builder().danger_accept_invalid_certs(true).build()?;
    let connector = MakeTlsConnector::new(na);

    let (client, connection) = tokio_postgres::connect(database_url, connector).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let res = client.execute("SELECT 1", &[]).await?;
    println!("{:?}", res);

    Ok(client)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!lc"))
        .group(&GENERAL_GROUP);

    let client = connect_to_postgres().await?;

    let token = env::var("BOT_TOKEN").expect("token");
    let mut client = Client::builder(token)
        .event_handler(Handler)
        .type_map_insert::<ClientHolder>(ClientHolder { client })
        .framework(framework)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        eprintln!("An error occured while running the client: {:?}", why);
    }

    Ok(())
}

struct ClientHolder {
    client: tokio_postgres::Client,
}

impl TypeMapKey for ClientHolder {
    type Value = ClientHolder;
}

#[command]
async fn store(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild_id;

    let data = ctx.data.read().await;

    let client = &(*data).get::<ClientHolder>().unwrap().client;

    client.execute("SELECT 1", &[]).await?;

    Ok(())
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}
