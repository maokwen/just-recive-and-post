#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;

#[cfg(test)]
mod test;

use rocket::tokio::sync::broadcast::Sender;
use rocket::response::{status::Created, Debug};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{State, Shutdown};
use rocket::{Build, Rocket};

use rocket_dyn_templates::Template;
use rocket_sync_db_pools::rusqlite;

use self::rusqlite::params;

use chrono::{TimeZone, Utc, FixedOffset};

#[database("sms_db")]

struct Db(rusqlite::Connection);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    date: Option<String>,
    from: String,
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    msgs: Option<Vec<Message>>,
}

type Result<T, E = Debug<rusqlite::Error>> = std::result::Result<T, E>;

#[get("/")]
async fn list(db: Db) -> Result<Json<Vec<i64>>> {
    let ids = db
        .run(|conn| {
            conn.prepare("SELECT id FROM msgs")?
                .query_map(params![], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()
        })
        .await?;

    Ok(Json(ids))
}

#[post("/", data = "<msg>")]
async fn create(db: Db, msg: Json<Message>, queue: &State<Sender<Message>>) -> Result<Created<Json<Message>>> {
    let now = Utc::now().naive_utc();
    let offset = FixedOffset::east_opt(8 * 3600);
    let date = offset.unwrap().from_utc_datetime(&now);
    let date_str = format!("{}", date.format("%Y-%m-%d %H:%M:%S"));
    
    let mut item = msg.clone();
    item.date = Some(date_str.clone());
    
    db.run(move |conn| {
        conn.execute(
            "INSERT INTO msgs (date, sms_from, sms_text) VALUES (?1, ?2, ?3)",
            params![date_str, msg.from, msg.text],
        )
    })
    .await?;

    let _ = queue.send(item.clone().into_inner());

    Ok(Created::new("/").body(item))
}

#[get("/<id>")]
async fn read(db: Db, id: i64) -> Option<Json<Message>> {
    let post = db
        .run(move |conn| {
            conn.query_row(
                "SELECT id, date, sms_from, sms_text FROM msgs WHERE id = ?1",
                params![id],
                |r| {
                    Ok(Message {
                        id: Some(r.get(0)?),
                        date: Some(r.get(1)?),
                        from: r.get(2)?,
                        text: r.get(3)?,
                    })
                },
            )
        })
        .await
        .ok()?;

    Some(Json(post))
}

#[delete("/<id>")]
async fn delete(db: Db, id: i64) -> Result<Option<()>> {
    let affected = db
        .run(move |conn| conn.execute("DELETE FROM msgs WHERE id = ?1", params![id]))
        .await?;

    Ok((affected == 1).then(|| ()))
}

#[delete("/")]
async fn destroy(db: Db) -> Result<()> {
    db.run(move |conn| conn.execute("DELETE FROM msgs", params![]))
        .await?;

    Ok(())
}

#[get("/")]
async fn index(db: Db) -> Template {
    let msgs: std::prelude::v1::Result<Vec<Message>, rusqlite::Error> = db
        .run(|conn| {
            conn.prepare("SELECT id, date, sms_from, sms_text FROM msgs ORDER BY id DESC LIMIT 20")?
                .query_map(params![], |row| {
                    Ok(Message {
                        id: Some(row.get(0)?),
                        date: Some(row.get(1)?),
                        from: row.get(2)?,
                        text: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<Message>, _>>()
        })
        .await;

    Template::render("index", Context { msgs: msgs.ok() })
}

#[get("/watch")]
async fn watch(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    use rocket::tokio::sync::broadcast::error::RecvError;

    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = rocket::tokio::select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };
            yield Event::json(&msg);
        }
    }
}

async fn init_db(rocket: Rocket<Build>) -> Rocket<Build> {
    Db::get_one(&rocket)
        .await
        .expect("database mounted")
        .run(|conn| {
            conn.execute(
                r#"
            CREATE TABLE IF NOT EXISTS msgs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date VARCHAR NOT NULL,
                sms_from VARCHAR NOT NULL,
                sms_text VARCHAR NOT NULL,
                published BOOLEAN NOT NULL DEFAULT 0
            )"#,
                params![],
            )
        })
        .await
        .expect("can init rusqlite DB");

    rocket
}

#[launch]
fn rocket() -> _ {
    use rocket::fairing::AdHoc;
    use rocket::fs::FileServer;
    use rocket::tokio::sync::broadcast::channel;

    rocket::build()
        .attach(Db::fairing())
        .attach(AdHoc::on_ignite("Rusqlite Init", init_db))
        .manage(channel::<Message>(1024).0)
        .mount("/api", routes![list, create, read, delete, destroy])
        .attach(Template::fairing())
        .mount("/", FileServer::from("static"))
        .mount("/", routes![index, watch])
}
