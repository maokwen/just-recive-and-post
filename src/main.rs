#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;

#[cfg(test)]
mod test;

use rocket::fairing::AdHoc;
use rocket::fs::{relative, FileServer};
use rocket::response::{status::Created, Debug};
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{Build, Rocket};

use rocket_dyn_templates::Template;
use rocket_sync_db_pools::rusqlite;

use self::rusqlite::params;

#[database("sms_db")]

struct Db(rusqlite::Connection);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    msg_type: String,
    text: String,
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
async fn create(db: Db, msg: Json<Message>) -> Result<Created<Json<Message>>> {
    let item = msg.clone();
    db.run(move |conn| {
        conn.execute(
            "INSERT INTO msgs (msg_type, text) VALUES (?1, ?2)",
            params![item.msg_type, item.text],
        )
    })
    .await?;

    Ok(Created::new("/").body(msg))
}

#[get("/<id>")]
async fn read(db: Db, id: i64) -> Option<Json<Message>> {
    let post = db
        .run(move |conn| {
            conn.query_row(
                "SELECT id, msg_type, text FROM msgs WHERE id = ?1",
                params![id],
                |r| {
                    Ok(Message {
                        id: Some(r.get(0)?),
                        msg_type: r.get(1)?,
                        text: r.get(2)?,
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

async fn init_db(rocket: Rocket<Build>) -> Rocket<Build> {
    Db::get_one(&rocket)
        .await
        .expect("database mounted")
        .run(|conn| {
            conn.execute(
                r#"
            CREATE TABLE IF NOT EXISTS msgs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                msg_type VARCHAR NOT NULL,
                text VARCHAR NOT NULL,
                published BOOLEAN NOT NULL DEFAULT 0
            )"#,
                params![],
            )
        })
        .await
        .expect("can init rusqlite DB");

    rocket
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    msgs: Option<Vec<Message>>,
}

#[get("/")]
async fn index(db: Db) -> Template {
    let msgs = db
        .run(|conn| {
            conn.prepare("SELECT id, msg_type, text FROM msgs DESC LIMIT 20")?
                .query_map(params![], |row| {
                    Ok(Message {
                        id: Some(row.get(0)?),
                        msg_type: row.get(1)?,
                        text: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<Message>, _>>()
        })
        .await;

    Template::render("index", Context { msgs: msgs.ok() })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Db::fairing())
        .attach(AdHoc::on_ignite("Rusqlite Init", init_db))
        .mount("/db", routes![list, create, read, delete, destroy])
        .attach(Template::fairing())
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![index])
}
