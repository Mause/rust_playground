use crate::proxy::{Mock, Proxy};
use log::{set_max_level, LevelFilter};
use serenity::client::bridge::gateway::ShardMessenger;
use serenity::client::Cache;
use serenity::client::Context;
use serenity::framework::standard::Args;
use serenity::futures::channel::mpsc::unbounded;
use serenity::gateway::InterMessage;
use serenity::http::Http;
use serenity::model::prelude::*;
use serenity::prelude::*;
use simple_logger::SimpleLogger;
use std::sync::Arc;
use tokio_test::block_on;

fn build_context(proxy: &Proxy) -> Context {
    let data = Arc::new(RwLock::new(TypeMap::new()));
    let shard_id = 0;

    let certificate = reqwest::Certificate::from_pem(&proxy.get_certificate()).unwrap();

    let http = Arc::new(
        reqwest::ClientBuilder::new()
            .add_root_certificate(certificate)
            .proxy(reqwest::Proxy::all(&proxy.url()).unwrap())
            .build()
            .unwrap(),
    );
    let cache = Arc::new(Cache::default());

    let (runner_tx, _) = unbounded::<InterMessage>();

    let shard = ShardMessenger::new(runner_tx);
    let context = Context {
        data: data,
        shard: shard,
        shard_id: shard_id,
        http: Arc::new(Http::new(http, "")),
        cache: cache,
    };

    context
}

fn build_message() -> Message {
    let obj = json::object! {
        "id": 0,
        "attachments": [],
        "content": "",
        "channel_id": 0,
        "embeds": [],
        "type": 1,
        "timestamp":  "2015-12-31T12:00:00.000Z",
        "tts": false,
        "pinned": false,
        "mention_everyone": false,
        "mention_roles": [],
        "mentions": [],
        "author": {
            "id": 0,
            "discriminator": 0,
            "username": "Mause"
        }
    };

    serde_json::from_str(&json::stringify(obj)).unwrap()
}

#[test]
fn it_works() {
    SimpleLogger::new().init().unwrap();
    set_max_level(LevelFilter::Trace);

    let mut proxy = Proxy::new();

    proxy.register(
        Mock::new("POST", "/api/v8/channels/0/messages")
            .with_body_from_file("src/message.json")
            .unwrap()
            .create(),
    );
    proxy.register(
        Mock::new("GET", "/api/v8/guilds/0/members")
            .with_body_from_json(json::object! {
                hello: "world"
            })
            .unwrap()
            .create(),
    );

    proxy.start();

    let context = build_context(&proxy);

    let message = build_message();

    let res = crate::store(&context, &message, Args::new("Victoria Park", &[]));

    let ares = block_on(res).unwrap();

    assert_eq!(ares, ());
}
