use actix_web::{
    test::{init_service, TestRequest, call_service},
    App, http::StatusCode,
};

use crate::{dto::NewUser, routes};

use super::test_util;

#[test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
fn can_create_user() {
    test_util::prepare_db_and_test(|db| async move {
        let test = TestRequest::post()
            .uri("/users")
            .set_json(NewUser {
                first_name: String::from("Evan"),
                last_name: String::from("Rittenhouse"),
            })
            .to_request();

        let test_svc =
            init_service(App::new().app_data(db).configure(routes::add_user_routes)).await;
        let response = call_service(&test_svc, test).await;

        assert_eq!(StatusCode::CREATED, response.status());
    });
}
