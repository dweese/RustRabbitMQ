use envy::Error;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub queue_name1: String,
    pub queue_name2: String,

}

impl Config {


    pub fn load() -> Result<Self, Error> {

        envy::from_env::<Config>()
    }
}
