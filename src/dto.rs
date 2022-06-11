use derive_more::Display;
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Display, Validate)]
#[display(fmt = "{} {}", "first_name", "last_name")]
pub struct NewUser {
    #[validate(length(max = 30))]
    pub first_name: String,
    #[validate(length(max = 50))]
    pub last_name: String,
}

#[derive(Deserialize, Validate)]
pub struct NewTask {
    #[validate(length(min = 1))]
    pub item_desc: String,
}

#[derive(Deserialize, Validate)]
pub struct UpdateTask {
    #[validate(length(min = 1))]
    pub item_desc: String,
}
