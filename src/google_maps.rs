use google_maps::prelude::ClientSettings;
use google_maps::{PlaceType, Region};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};

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

pub async fn resolve_location(
    location: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client =
        ClientSettings::new(&env::var("GOOGLE_MAPS_API_KEY").expect("GOOGLE_MAPS_API_KEY"));
    let res = client
        .geocoding()
        .with_address(location)
        .with_region(Region::Australia)
        .execute()
        .await
        .expect("Geocode call failed");

    let result = &res.results[0];

    if result.types != [PlaceType::Locality, PlaceType::Political] {
        Err(Box::new(SimpleError(format!(
            "Not a suburb: {:?}",
            result.types
        ))))
    } else {
        Ok(result.formatted_address.to_string())
    }
}
