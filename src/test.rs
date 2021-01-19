use crate::proxy::{start_proxy, Proxy};
use log::{set_max_level, LevelFilter};
use serenity::client::bridge::gateway::ShardMessenger;
use serenity::client::Cache;
use serenity::client::Context;
use serenity::framework::standard::Args;
use serenity::futures::channel::mpsc::unbounded;
use serenity::gateway::InterMessage;
use serenity::http::Http;
use serenity::prelude::RwLock;
use serenity::prelude::TypeMap;
use simple_logger::SimpleLogger;
use std::fs::File;
use std::io::BufReader;
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

#[test]
fn it_works() {
    SimpleLogger::new().init().unwrap();
    set_max_level(LevelFilter::Trace);

    println!("Starting proxy");
    let mut proxy = Proxy::new();
    start_proxy(&mut proxy);
    println!("Proxy {}", proxy.url());

    let context = build_context(&proxy);

    let message =
        serde_json::from_reader(BufReader::new(File::open("src/message.json").unwrap())).unwrap();

    let res = crate::store(&context, &message, Args::new("", &[]));

    let ares = block_on(res).unwrap();

    assert_eq!(ares, ());
}
