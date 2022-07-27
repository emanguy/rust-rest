use derive_more::Display;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Serialize, Display, Validate)]
#[display(fmt = "{} {}", "first_name", "last_name")]
pub struct NewUser {
    #[validate(length(max = 30))]
    pub first_name: String,
    #[validate(length(max = 50))]
    pub last_name: String,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct NewTask {
    #[validate(length(min = 1))]
    pub item_desc: String,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct UpdateTask {
    #[validate(length(min = 1))]
    pub item_desc: String,
}

#[cfg(test)]
mod dto_tests {
    use super::*;

    #[test]
    fn bad_user_data_gets_rejected() {
        let bad_user = NewUser {
            first_name: (0..35).map(|_| "A").collect(),
            last_name: (0..55).map(|_| "B").collect(),
        };
        let validation_result = bad_user.validate();
        assert!(validation_result.is_err());
        let validation_errors = validation_result.unwrap_err();
        let field_validations = validation_errors.field_errors();
        assert!(field_validations.contains_key("first_name"));
        assert!(field_validations.contains_key("last_name"));
        println!("Hellur");
    }
}
