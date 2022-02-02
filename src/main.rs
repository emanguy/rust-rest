use actix_web::*;
use log::*;
use dotenv::dotenv;
use postgres::{Client, Config, NoTls, Row};
use std::env;

mod routes;
mod db;

#[derive(Debug)]
struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

impl From<&Row> for TodoUser {
    fn from(row: &Row) -> Self {
        TodoUser {
            id: row.get("id"),
            first_name: row.get("first_name"),
            last_name: row.get("last_name"),
        }
    }
}

struct NewUser {
    pub first_name: String,
    pub last_name: String,
}

fn connect() -> Client {
    Config::new()
        .host("127.0.0.1")
        .port(5432)
        .user("postgres")
        .password("sample123")
        .connect(NoTls)
        .unwrap()
}

fn get_and_print_users(conn: &mut Client) {
    let fetched_users = match conn.query("SELECT * FROM todo_user JOIN todo_item on todo_item.user_id = todo_user.id", &[]) {
        Ok(rows) => rows.iter().map(TodoUser::from).collect::<Vec<TodoUser>>(),
        Err(_) => {
            error!("Failed to select on users");
            return;
        }
    };

    for user in fetched_users {
        info!("Got user: {:?}", user);
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::builder().filter_level(LevelFilter::Debug).init();
    // let db_url = env::var("DATABASE_URL").expect("Could not get database URL from environment");    
    let mut db_connection = connect();
    get_and_print_users(&mut db_connection);

    info!("Starting server.");
    return HttpServer::new(|| {
        App::new()
            .service(routes::hello)
            .service(routes::echo)
            .route("/hey", web::get().to(routes::manual_hello))
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await;
}
