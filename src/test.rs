use crate::proxy::{get_certificate, start_proxy, url};
use log::{set_max_level, LevelFilter};
use reqwest::Proxy;
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

fn build_context(base: String) -> Context {
    let data = Arc::new(RwLock::new(TypeMap::new()));
    let shard_id = 0;

    let proxy = Proxy::all(&base).unwrap();

    println!("{:?}", &proxy);

    let certificate = reqwest::Certificate::from_pem(&get_certificate()).unwrap();

    let http = Arc::new(
        reqwest::ClientBuilder::new()
            .add_root_certificate(certificate)
            .proxy(proxy)
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
    let mut proxy = start_proxy();
    println!("Proxy {}", url());

    let context = build_context(url());

    let message =
        serde_json::from_reader(BufReader::new(File::open("src/message.json").unwrap())).unwrap();

    let res = crate::store(&context, &message, Args::new("", &[]));

    let ares = block_on(res).unwrap();

    assert_eq!(ares, ());
}
