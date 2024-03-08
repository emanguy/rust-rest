use axum::body::{self, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use tower::Service; // THIS IS REQUIRED FOR Router.call()

use crate::{
    api, dto,
    dto::{InsertedUser, NewUser},
};

use super::test_util;

fn create_user_request() -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/users")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&NewUser {
                first_name: String::from("John"),
                last_name: String::from("Doe"),
            })
            .unwrap(),
        ))
        .unwrap()
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_create_user() {
    let router = Router::new().nest("/users", api::user::user_routes());
    let (mut app, _) = test_util::prepare_application(router).await;
    let test_req = create_user_request();

    let response = app.call(test_req).await.unwrap();

    let status = response.status();
    let body = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Could not read response body");
    assert_eq!(
        StatusCode::CREATED,
        status,
        "Failed to create with response body {:?}",
        body
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_retrieve_user() {
    let router = Router::new().nest("/users", api::user::user_routes());
    let (mut app, _) = test_util::prepare_application(router).await;
    let create_user_req = create_user_request();

    let create_response = app.call(create_user_req).await.unwrap();
    let create_status = create_response.status();
    let res_body = body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("Could not read create user body");
    assert_eq!(
        StatusCode::CREATED,
        create_status,
        "Did not get expected status code, received response was {:?}",
        res_body
    );

    let user_id = serde_json::from_slice::<InsertedUser>(&res_body)
        .unwrap_or_else(|_| panic!("Could not parse create user response, got body {res_body:?}"));

    let list_users_req = Request::builder()
        .method(Method::GET)
        .uri("/users")
        .body(Body::empty())
        .expect("List users request failed to construct");
    let list_users_resp = app
        .call(list_users_req)
        .await
        .expect("User lookup request failed");
    let list_users_status = list_users_resp.status();
    let res_body = body::to_bytes(list_users_resp.into_body(), usize::MAX)
        .await
        .expect("Could not read response from list users endpoint");

    assert_eq!(
        StatusCode::OK,
        list_users_status,
        "Got bad status code from list users endpoint, received response {:?}",
        res_body
    );

    let received_user: Vec<dto::TodoUser> =
        serde_json::from_slice(&res_body).expect("Could not parse user list body");
    let expected_user = dto::TodoUser {
        id: user_id.id,
        first_name: String::from("John"),
        last_name: String::from("Doe"),
    };

    assert_eq!(expected_user, received_user[0]);
}
