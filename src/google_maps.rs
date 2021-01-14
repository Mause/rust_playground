use google_maps::prelude::ClientSettings;
use google_maps::Region;
use std::env;
use tokio_compat_02::FutureExt;

pub async fn sync_resolve_location(location: &str) -> String {
    let client =
        ClientSettings::new(&env::var("GOOGLE_MAPS_API_KEY").expect("GOOGLE_MAPS_API_KEY"));
    let res = client
        .geocoding()
        .with_address(&location)
        .with_region(Region::Australia)
        .execute()
        .compat()
        .await
        .expect("Geocode call failed");
    res.results[0].formatted_address.to_string()
}
