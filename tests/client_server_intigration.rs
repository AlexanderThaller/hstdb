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
    cache_dir: PathBuf,
    data_dir: PathBuf,

    keep_datadir: bool,
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.barrier_stop.wait();

        std::fs::remove_dir_all(&self.cache_dir).unwrap();

        if !self.keep_datadir {
            std::fs::remove_dir_all(&self.data_dir).unwrap();
        }
    }
}

fn create_client_and_server(keep_datadir: bool) -> TestClient {
    let cache_dir = tempfile::tempdir().unwrap().into_path();
    let data_dir = tempfile::tempdir().unwrap().into_path();
    let socket = tempfile::NamedTempFile::new()
        .unwrap()
        .into_temp_path()
        .to_path_buf();

    let barrier_start = Arc::new(Barrier::new(2));
    let barrier_stop = Arc::new(Barrier::new(2));

    {
        let barrier_start = Arc::clone(&barrier_start);
        let barrier_stop = Arc::clone(&barrier_stop);

        let cache_dir = cache_dir.clone();
        let data_dir = data_dir.clone();
        let socket = socket.clone();

        let server = server::builder(cache_dir, data_dir, socket, false)
            .build()
            .unwrap();

        thread::spawn(move || {
            barrier_start.wait();
            server.run().unwrap();
            barrier_stop.wait();
        });
    }

    barrier_start.wait();

    let client = client::new(socket);

    TestClient {
        client,
        barrier_stop,
        cache_dir,
        data_dir,
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
        session_id: session_id.clone(),
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
        command: r#"Test\nTest\nTest      "#.to_string(),
        pwd: PathBuf::from("/tmp"),
        session_id: session_id.clone(),
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
        command: r#"Test\nTest\nTest"#.to_string(),
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
        command: "".to_string(),
        pwd: PathBuf::from("/tmp"),
        session_id: session_id.clone(),
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
        r#"\n"#.to_string(),
        '\n'.to_string(),
        '\r'.to_string(),
    ];

    for command in commands {
        let start_data = CommandStart {
            command,
            pwd: PathBuf::from("/tmp"),
            session_id: session_id.clone(),
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

#[test]
fn existing_empty_file() {
    let hostname = "testhostname".to_string();
    let data_dir = tempfile::tempdir().unwrap().into_path();
    std::fs::File::create(data_dir.join(format!("{}.csv", hostname))).unwrap();

    dbg!(&data_dir);

    let store = store::new(data_dir.clone());
    let entries = store.get_entries(&Filter::default()).unwrap();

    dbg!(&entries);

    assert_eq!(entries.len(), 0);

    std::fs::remove_dir_all(&data_dir).unwrap();
}
