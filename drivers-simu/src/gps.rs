pub use drivers_shared::gps::*;
pub struct GPSRessources {}

impl GPSRessources {
    pub async fn on<'a>(&'a mut self) -> GPS<'a> {
        GPS { ressources: self }
    }
}

pub struct GPS<'a> {
    #[allow(unused)]
    ressources: &'a mut GPSRessources,
}
