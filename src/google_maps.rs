use google_maps::prelude::ClientSettings;
use google_maps::Region;
use std::env;

pub fn sync_resolve_location(location: &str) -> String {
    let mut client =
        ClientSettings::new(&env::var("GOOGLE_MAPS_API_KEY").expect("GOOGLE_MAPS_API_KEY"));

    let res = client
        .geocoding()
        .with_address(location)
        .with_region(Region::Australia)
        .execute()
        .expect("Geocode call failed");

    res.results[0].formatted_address.to_string()
}
