use std::borrow::Borrow;

use actix_http::{body::MessageBody, Request};
use actix_web::{
    http::StatusCode,
    test::{call_service, TestRequest},
};

use crate::{
    db::TodoUser,
    dto::NewUser,
    routes::{self, InsertedUser},
};

use super::test_util::{self, prepare_application};

fn create_user_request() -> Request {
    TestRequest::post()
        .uri("/users")
        .set_json(NewUser {
            first_name: String::from("John"),
            last_name: String::from("Doe"),
        })
        .to_request()
}

#[actix_web::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_create_user() {
    let app = test_util::prepare_application(&[&routes::add_user_routes]).await;
    let test_req = create_user_request();

    let response = call_service(&app, test_req).await;

    assert_eq!(StatusCode::CREATED, response.status());
}

#[actix_web::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_retrieve_user() {
    let app = prepare_application(&[&routes::add_user_routes]).await;
    let create_user_req = create_user_request();

    let create_response = call_service(&app, create_user_req).await;
    assert_eq!(StatusCode::CREATED, create_response.status());

    let res_body = create_response.into_body().try_into_bytes();
    let user_id = serde_json::from_slice::<InsertedUser>(
        res_body.expect("Could not read create user body").borrow(),
    )
    .expect("Could not parse create user response");

    let delete_user_req = TestRequest::get()
        .uri("/users".to_string().as_str())
        .to_request();
    let delete_response = call_service(&app, delete_user_req).await;

    assert_eq!(StatusCode::OK, delete_response.status());

    let res_body = delete_response.into_body().try_into_bytes();
    let received_user: Vec<TodoUser> =
        serde_json::from_slice(res_body.expect("Could not extract user list body").borrow())
            .expect("Could not parse user list body");
    let expected_user = TodoUser {
        id: user_id.id,
        first_name: String::from("John"),
        last_name: String::from("Doe"),
    };

    assert_eq!(expected_user, received_user[0]);
}
