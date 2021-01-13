use futures::executor::block_on;
use futures::future::ok;
use google_maps::prelude::ClientSettings;
use google_maps::Region;
use std::env;

pub async fn sync_resolve_location(location: &str) -> String {
    let copied_location = location.to_string();

    let fut = futures::future::lazy(move |_| {
        let mut client =
            ClientSettings::new(&env::var("GOOGLE_MAPS_API_KEY").expect("GOOGLE_MAPS_API_KEY"));
        let res = client
            .geocoding()
            .with_address(&copied_location)
            .with_region(Region::Australia)
            .execute()
            .expect("Geocode call failed");
        ok::<String, String>(res.results[0].formatted_address.to_string())
    });

    let spawn = futures::executor::spawn_with_handle(fut);
    let handle = block_on(spawn).unwrap();
    block_on(handle).unwrap()
}
