use super::rocket;
use rocket::http::Status;
use rocket::local::blocking::Client;
use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    from: String,
    text: String,
}

#[test]
fn test() {
    let client = Client::tracked(rocket()).unwrap();

    assert_eq!(client.delete("/api").dispatch().status(), Status::Ok);
    assert_eq!(
        client.get("/api").dispatch().into_json::<Vec<i64>>(),
        Some(vec![])
    );

    // test post
    for i in 1_usize..=20 {
        let from = "123123".to_string();
        let text = format!("msg msg {}", i);
        let msg = Message {
            from,
            text,
        };

        let response = client
            .post("/api")
            .json(&msg)
            .dispatch()
            .into_json::<Message>();
        assert_eq!(response.unwrap(), msg);

        let list = client
            .get("/api")
            .dispatch()
            .into_json::<Vec<i64>>()
            .unwrap();
        assert_eq!(list.len(), i);

        let last = list.last().unwrap();
        let response = client.get(format!("/api/{}", last)).dispatch();
        assert_eq!(response.into_json::<Message>().unwrap(), msg);
    }

    // test delete
    for _ in 1..=20 {
        let list = client
            .get("/api")
            .dispatch()
            .into_json::<Vec<i64>>()
            .unwrap();
        let id = list.get(0).expect("have msg");

        let response = client.delete(format!("/api/{}", id)).dispatch();
        assert_eq!(response.status(), Status::Ok);
    }
    let list = client
        .get("/api")
        .dispatch()
        .into_json::<Vec<i64>>()
        .unwrap();
    assert!(list.is_empty());

    let response = client.delete(format!("/api/{}", 1)).dispatch();
    assert_eq!(response.status(), Status::NotFound);
}
