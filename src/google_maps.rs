use google_maps::prelude::ClientSettings;
use google_maps::Region;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use tokio_compat_02::FutureExt;

#[derive(Debug, Clone)]
pub struct SimpleError(pub String);
unsafe impl Send for SimpleError {}
impl Display for SimpleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}
impl Error for SimpleError {
    fn source(&self) -> std::option::Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
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
