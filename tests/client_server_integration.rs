#![allow(
    missing_docs,
    reason = "integration tests are not part of the public API"
)]

use pretty_assertions::assert_eq;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        Barrier,
    },
    thread,
};

use chrono::Utc;
use hstdb::{
    client::{
        self,
        Client,
    },
    entry::Entry,
    message::{
        CommandFinished,
        CommandStart,
        Message,
    },
    server,
    store::{
        self,
        Filter,
    },
};
use uuid::Uuid;

struct TestClient {
    client: Client,
    barrier_stop: Arc<Barrier>,
    data_dir: PathBuf,
    state_dir: PathBuf,

    keep_datadir: bool,
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.barrier_stop.wait();

        std::fs::remove_dir_all(&self.state_dir).expect("Failed to remove state dir");

        if !self.keep_datadir {
            std::fs::remove_dir_all(&self.data_dir).expect("Failed to remove data dir");
        }
    }
}

fn create_client_and_server(keep_datadir: bool) -> TestClient {
    let data_dir = tempfile::tempdir()
        .expect("Failed to create data dir")
        .keep();

    let state_dir = tempfile::tempdir()
        .expect("Failed to create state dir")
        .keep();

    let socket = tempfile::NamedTempFile::new()
        .expect("Failed to create socket file")
        .into_temp_path()
        .to_path_buf();

    let barrier_start = Arc::new(Barrier::new(2));
    let barrier_stop = Arc::new(Barrier::new(2));

    {
        let barrier_start = Arc::clone(&barrier_start);
        let barrier_stop = Arc::clone(&barrier_stop);

        let data_dir = data_dir.clone();
        let state_dir = state_dir.clone();
        let socket = socket.clone();

        let server = server::builder(data_dir, socket, state_dir, false)
            .build()
            .expect("Failed to build server");

        thread::spawn(move || {
            barrier_start.wait();
            server.run().expect("Server run failed");
            barrier_stop.wait();
        });
    }

    barrier_start.wait();

    let client = client::new(socket);

    TestClient {
        client,
        barrier_stop,
        data_dir,
        state_dir,
        keep_datadir,
    }
}

#[test]
fn stop_server() {
    let client = create_client_and_server(false);
    client.client.send(&Message::Stop).unwrap();
}

#[test]
fn write_entry() {
    let client = create_client_and_server(true);

    let session_id = Uuid::new_v4();

    let start_data = CommandStart {
        command: "Test".to_string(),
        pwd: PathBuf::from("/tmp"),
        session_id,
        time_stamp: Utc::now(),
        user: "testuser".to_string(),
        hostname: "testhostname".to_string(),
    };

    let finish_data = CommandFinished {
        session_id,
        time_stamp: Utc::now(),
        result: 0,
    };

    client
        .client
        .send(&Message::CommandStart(start_data.clone()))
        .unwrap();

    client
        .client
        .send(&Message::CommandFinished(finish_data.clone()))
        .unwrap();

    client.client.send(&Message::Stop).unwrap();

    let data_dir = client.data_dir.clone();
    drop(client);

    let mut entries = store::new(data_dir.clone())
        .get_entries(&Filter::default())
        .unwrap();

    std::fs::remove_dir_all(data_dir).unwrap();

    dbg!(&entries);

    assert_eq!(entries.len(), 1);

    let got = entries.remove(0);
    let expected = Entry {
        time_finished: finish_data.time_stamp,
        time_start: start_data.time_stamp,
        hostname: start_data.hostname,
        command: start_data.command,
        pwd: start_data.pwd,
        result: finish_data.result,
        session_id: start_data.session_id,
        user: start_data.user,
    };

    assert_eq!(expected, got);
}

#[test]
fn write_entry_whitespace() {
    let client = create_client_and_server(true);

    let session_id = Uuid::new_v4();

    let start_data = CommandStart {
        command: "Test\nTest\nTest      ".to_string(),
        pwd: PathBuf::from("/tmp"),
        session_id,
        time_stamp: Utc::now(),
        user: "testuser".to_string(),
        hostname: "testhostname".to_string(),
    };

    let finish_data = CommandFinished {
        session_id,
        time_stamp: Utc::now(),
        result: 0,
    };

    client
        .client
        .send(&Message::CommandStart(start_data.clone()))
        .unwrap();

    client
        .client
        .send(&Message::CommandFinished(finish_data.clone()))
        .unwrap();

    client.client.send(&Message::Stop).unwrap();

    let data_dir = client.data_dir.clone();
    drop(client);

    let mut entries = store::new(data_dir.clone())
        .get_entries(&Filter::default())
        .unwrap();

    std::fs::remove_dir_all(data_dir).unwrap();

    dbg!(&entries);

    assert_eq!(entries.len(), 1);

    let got = entries.remove(0);
    let expected = Entry {
        time_finished: finish_data.time_stamp,
        time_start: start_data.time_stamp,
        hostname: start_data.hostname,
        command: "Test\nTest\nTest".to_string(),
        pwd: start_data.pwd,
        result: finish_data.result,
        session_id: start_data.session_id,
        user: start_data.user,
    };

    assert_eq!(expected, got);
}

// TODO: Make a test for this probably needs a restructuring of how we
// detect leading spaces in commands
//#[test]
// fn write_command_starting_spaces() {
//    let client = create_client_and_server(true);
//
//    let session_id = Uuid::new_v4();
//
//    let start_data = CommandStart {
//        command: " Test".to_string(),
//        pwd: PathBuf::from("/tmp"),
//        session_id: session_id.clone(),
//        time_stamp: Utc::now(),
//        user: "testuser".to_string(),
//        hostname: "testhostname".to_string(),
//    };
//
//    let finish_data = CommandFinished {
//        session_id,
//        time_stamp: Utc::now(),
//        result: 0,
//    };
//
//    client
//        .client
//        .send(&Message::CommandStart(start_data.clone()))
//        .unwrap();
//
//    client
//        .client
//        .send(&Message::CommandFinished(finish_data.clone()))
//        .unwrap();
//
//    client.client.send(&Message::Stop).unwrap();
//
//    let data_dir = client.data_dir.clone();
//    drop(client);
//
//    let entries = store::new(data_dir.clone())
//        .get_entries(&Filter::default())
//        .unwrap();
//
//    std::fs::remove_dir_all(data_dir).unwrap();
//
//    dbg!(&entries);
//
//    assert_eq!(entries.len(), 0);
//}

#[test]
fn write_empty_command() {
    let client = create_client_and_server(true);

    let session_id = Uuid::new_v4();

    let start_data = CommandStart {
        command: String::new(),
        pwd: PathBuf::from("/tmp"),
        session_id,
        time_stamp: Utc::now(),
        user: "testuser".to_string(),
        hostname: "testhostname".to_string(),
    };

    let finish_data = CommandFinished {
        session_id,
        time_stamp: Utc::now(),
        result: 0,
    };

    client
        .client
        .send(&Message::CommandStart(start_data.clone()))
        .unwrap();

    client
        .client
        .send(&Message::CommandFinished(finish_data.clone()))
        .unwrap();

    client.client.send(&Message::Stop).unwrap();

    let data_dir = client.data_dir.clone();
    drop(client);

    let entries = store::new(data_dir.clone())
        .get_entries(&Filter::default())
        .unwrap();

    std::fs::remove_dir_all(data_dir).unwrap();

    dbg!(&entries);

    assert_eq!(entries.len(), 0);
}

#[test]
fn write_newline_command() {
    let client = create_client_and_server(true);

    let session_id = Uuid::new_v4();

    let commands = vec![
        "\n".to_string(),
        "\r\n".to_string(),
        "\n\n".to_string(),
        "\n\n\n".to_string(),
        "\n".to_string(),
        '\n'.to_string(),
        '\r'.to_string(),
    ];

    for command in commands {
        let start_data = CommandStart {
            command,
            pwd: PathBuf::from("/tmp"),
            session_id,
            time_stamp: Utc::now(),
            user: "testuser".to_string(),
            hostname: "testhostname".to_string(),
        };

        let finish_data = CommandFinished {
            session_id,
            time_stamp: Utc::now(),
            result: 0,
        };

        client
            .client
            .send(&Message::CommandStart(start_data.clone()))
            .unwrap();

        client
            .client
            .send(&Message::CommandFinished(finish_data.clone()))
            .unwrap();
    }

    client.client.send(&Message::Stop).unwrap();

    let data_dir = client.data_dir.clone();
    drop(client);

    let entries = store::new(data_dir.clone())
        .get_entries(&Filter::default())
        .unwrap();

    std::fs::remove_dir_all(data_dir).unwrap();

    dbg!(&entries);

    assert_eq!(entries.len(), 0);
}
