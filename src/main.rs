use crate::client_holder::{read_client, ClientHolder};
use crate::db::{connect_to_postgres, Location, U64};
use crate::google_maps::resolve_location;
use deadpool_postgres::tokio_postgres;
use dotenv::dotenv;
use google_maps::prelude::ClientSettings;
use log::{info, set_max_level, LevelFilter};
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group},
    Args, CommandResult, StandardFramework,
};
use serenity::model::channel::Message;
use simple_logger::SimpleLogger;
use std::env;
use tokio_pg_mapper::FromTokioPostgresRow;
use tokio_postgres::Row;

mod client_holder;
mod db;
mod google_maps;
#[cfg(test)]
mod proxy;
#[cfg(test)]
mod test;

struct Handler;

#[group]
#[commands(ping, load, store)]
struct General;

#[async_trait]
impl EventHandler for Handler {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    SimpleLogger::new().init().unwrap();
    set_max_level(LevelFilter::Debug);

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!lc "))
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

#[command]
async fn store(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if args.is_empty() {
        msg.reply_mention(ctx, "Looks like you forgot to pass a location")
            .await?;
        return Ok(());
    }

    let location = args.message();

    let client =
        ClientSettings::new(&env::var("GOOGLE_MAPS_API_KEY").expect("GOOGLE_MAPS_API_KEY"));

    let res = resolve_location(location, &client).await;

    match res {
        Err(e) => {
            msg.reply_mention(ctx, e).await?;
            Ok(())
        }
        Ok(resolved_location) => {
            info!("Resolved location");

            let client = read_client(ctx).await;

            let guild_id = msg.guild_id.expect("No guild?").0;
            let member_id = msg.author.id.0;

            if exists_by(&client, guild_id, member_id).await {
                update_existing(&client, guild_id, member_id, &resolved_location).await;
            } else {
                insert_new(&client, guild_id, member_id, &resolved_location).await;
            }

            msg.reply_mention(
                ctx,
                format!("Ok, I now have you down living at {}", resolved_location),
            )
            .await?;

            Ok(())
        }
    }
}

async fn update_existing(
    client: &tokio_postgres::Client,
    guild_id: u64,
    member_id: u64,
    location: &String,
) {
    client
        .execute(
            "UPDATE LOCATION SET location=$3 where guild_id=$1 and member_id=$2",
            &[&U64::new(guild_id), &U64::new(member_id), location],
        )
        .await
        .expect("Update failed");
}

async fn exists_by(client: &tokio_postgres::Client, guild_id: u64, member_id: u64) -> bool {
    load_location(client, guild_id, member_id).await.is_some()
}

async fn insert_new(
    client: &tokio_postgres::Client,
    guild_id: u64,
    member_id: u64,
    resolved_location: &String,
) {
    client
        .execute(
            "INSERT INTO LOCATION (guild_id, member_id, location) values ($1, $2, $3)",
            &[&U64::new(guild_id), &U64::new(member_id), resolved_location],
        )
        .await
        .expect("Insert failed");
}

async fn load_location(
    client: &tokio_postgres::Client,
    guild_id: u64,
    member_id: u64,
) -> Option<Row> {
    client
        .query_opt(
            "SELECT * FROM LOCATION where guild_id = $1 and member_id = $2",
            &[&U64::new(guild_id), &U64::new(member_id)],
        )
        .await
        .expect("location query failed")
}

#[command]
async fn load(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild_id.expect("No guild?").0;

    let client = read_client(ctx).await;

    let res = load_location(&client, guild, msg.author.id.0).await;

    let response = match res {
        None => "Sorry, I don't know where you live".to_string(),
        Some(row) => {
            let row = Location::from_row(row).unwrap();

            format!("I have you down as living in {}", row.location)
        }
    };

    msg.reply_mention(ctx, response).await?;

    Ok(())
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}
