use super::rocket;
use rocket::http::Status;
use rocket::local::blocking::Client;
use rocket::serde::{Deserialize, Serialize};

use chrono::{Duration, Utc};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    date: String,
    text: String,
}

#[test]
fn test() {
    let client = Client::tracked(rocket()).unwrap();

    assert_eq!(client.delete("/db").dispatch().status(), Status::Ok);
    assert_eq!(
        client.get("/db").dispatch().into_json::<Vec<i64>>(),
        Some(vec![])
    );

    // test post
    for i in 1_usize..=20 {
        let date = (Utc::now() + Duration::minutes(i as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let text = format!("msg msg {}", i);
        let msg = Message {
            date: date,
            text: text,
        };

        let response = client
            .post("/db")
            .json(&msg)
            .dispatch()
            .into_json::<Message>();
        assert_eq!(response.unwrap(), msg);

        let list = client
            .get("/db")
            .dispatch()
            .into_json::<Vec<i64>>()
            .unwrap();
        assert_eq!(list.len(), i);

        let last = list.last().unwrap();
        let response = client.get(format!("/db/{}", last)).dispatch();
        assert_eq!(response.into_json::<Message>().unwrap(), msg);
    }

    // test delete
    for _ in 1..=20 {
        let list = client
            .get("/db")
            .dispatch()
            .into_json::<Vec<i64>>()
            .unwrap();
        let id = list.get(0).expect("have msg");

        let response = client.delete(format!("/db/{}", id)).dispatch();
        assert_eq!(response.status(), Status::Ok);
    }
    let list = client
        .get("/db")
        .dispatch()
        .into_json::<Vec<i64>>()
        .unwrap();
    assert!(list.is_empty());

    let response = client.delete(format!("/db/{}", 1)).dispatch();
    assert_eq!(response.status(), Status::NotFound);
}
