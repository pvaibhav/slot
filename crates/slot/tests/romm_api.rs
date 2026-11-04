use slot::romm::api::{
    content_path, device_name, parse_installs, parse_pending, parse_poll, parse_rom, parse_version, report_body,
    status_error,
};
use slot::romm::{http_api, version_ok, ApiError, Outcome, Poll, RomFile, RommConf};

#[test]
fn version_table() {
    for (v, want) in [
        ("5.4.0", true),
        ("5.10.0", true),
        ("6.0.0", true),
        ("5.3.1", false),
        ("5.4.0-beta.1", true),
        ("development", true),
        ("garbage", true),
        ("5.x.0", false),
    ] {
        assert_eq!(version_ok(v), want, "{v}");
    }
}

#[test]
fn pairing_names_the_device_by_model() {
    assert_eq!(device_name(Some("rg35xxsp")), "slot on rg35xxsp");
    assert_eq!(device_name(None), "slot");
}

#[test]
fn rom_files_are_parsed() {
    let rom = parse_rom(
        r#"{"platform_slug":"gba","fs_name":"Folder","files":[{"id":7,"file_name":"Game.gba"}]}"#,
    )
    .unwrap();
    assert_eq!(
        rom.files,
        vec![RomFile {
            id: 7,
            file_name: "Game.gba".into()
        }]
    );
}

#[test]
fn poll_maps_every_pending_detail() {
    for (detail, want) in [
        ("authorization_pending", Poll::Waiting),
        ("slow_down", Poll::SlowDown),
        ("access_denied", Poll::Denied),
        ("expired_token", Poll::Expired),
    ] {
        let body = format!(r#"{{"detail":"{detail}"}}"#);
        assert_eq!(parse_poll(400, &body), Ok(want), "{detail}");
    }
}

#[test]
fn poll_success_carries_token_and_device() {
    let body = r#"{"access_token":"rmm_x","device_id":"d1","scopes":[],"expires_at":null}"#;
    assert_eq!(
        parse_poll(200, body),
        Ok(Poll::Paired {
            token: "rmm_x".into(),
            device_id: "d1".into()
        })
    );
}

#[test]
fn status_mapping() {
    assert_eq!(status_error(401), ApiError::Unauthorized);
    assert_eq!(status_error(404), ApiError::NotFound);
    assert_eq!(status_error(409), ApiError::Client(409));
    assert_eq!(status_error(503), ApiError::Server(503));
}

#[test]
fn connection_failure_is_offline() {
    let conf = RommConf {
        protocol: slot::romm::Protocol::Http,
        server: "127.0.0.1".into(),
        port: Some(1),
        device_id: Some("d".into()),
        token: Some("t".into()),
        ..RommConf::default()
    };
    assert_eq!(http_api(&conf).version(), Err(ApiError::Offline));
}

#[test]
fn parses_the_response_fixtures() {
    assert_eq!(
        parse_version(r#"{"SYSTEM":{"VERSION":"5.4.0"}}"#),
        Ok("5.4.0".into())
    );
    let pending = parse_pending(
        r#"{"device_code":"dc","user_code":"ABCD-EFGH","verification_path":"/pair/device","verification_path_complete":"/pair/device?user_code=ABCD-EFGH","expires_in":600,"interval":5}"#,
    )
    .unwrap();
    assert_eq!(pending.path, "/pair/device?user_code=ABCD-EFGH");
    assert_eq!(pending.interval, 5);
    let installs = parse_installs(
        r#"[{"id":"i1","user_id":1,"device_id":"d","rom_id":7,"file_ids":[3,4],"status":"taken"}]"#,
    )
    .unwrap();
    assert_eq!(installs[0].rom_id, 7);
    assert_eq!(installs[0].file_ids, vec![3, 4]);
    let rom = parse_rom(r#"{"platform_slug":"gba","fs_name":"Game (USA).zip"}"#).unwrap();
    assert_eq!(rom.platform_slug, "gba");
}

#[test]
fn builds_requests() {
    assert_eq!(
        content_path(7, "Game (USA).zip", &[1, 2]),
        "/api/roms/7/content/Game%20%28USA%29.zip?file_ids=1,2"
    );
    assert_eq!(report_body(&Outcome::Done), r#"{"status":"done"}"#);
    assert_eq!(
        report_body(&Outcome::Failed("no".into())),
        r#"{"reason":"no","status":"failed"}"#
    );
}

mod loopback {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use slot::romm::{http_api, ApiError, Cancel, Protocol, RommConf};

    fn serve(response: &'static [u8], hold: bool) -> (RommConf, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = sock.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..n]);
            }
            let _ = tx.send(String::from_utf8_lossy(&seen).to_string());
            sock.write_all(response).unwrap();
            if hold {
                thread::sleep(std::time::Duration::from_millis(300));
            }
        });
        let conf = RommConf {
            protocol: Protocol::Http,
            server: "127.0.0.1".into(),
            port: Some(port),
            device_id: Some("d".into()),
            token: Some("secret-token".into()),
            ..RommConf::default()
        };
        (conf, rx)
    }

    #[test]
    fn pairing_requests_send_no_authorization() {
        let body = r#"{"device_code":"dc","user_code":"U","verification_path_complete":"/p","expires_in":1,"interval":1}"#;
        let response: &'static [u8] = Box::leak(
            format!(
                "HTTP/1.1 201 Created\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .into_boxed_str()
            .into_boxed_bytes(),
        );
        let (conf, rx) = serve(response, false);
        http_api(&conf).pair_start(&conf).unwrap();
        let request = rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.contains("/api/auth/device/init"));
        assert!(!request.contains("authorization"));
    }

    #[test]
    fn authed_requests_send_the_bearer() {
        let (conf, rx) = serve(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]",
            false,
        );
        assert_eq!(http_api(&conf).claim(), Ok(vec![]));
        let request = rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.contains("authorization: bearer secret-token"));
    }

    #[test]
    fn short_body_is_truncated() {
        let (conf, _rx) = serve(
            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nabc",
            false,
        );
        let dir = tempfile::tempdir().unwrap();
        let result = http_api(&conf).download(
            1,
            "a.gba",
            &[],
            &dir.path().join("a.gba"),
            &Cancel::default(),
        );
        assert_eq!(result, Err(ApiError::Truncated));
    }

    #[test]
    fn cancel_before_the_request_never_connects() {
        let (conf, _rx) = serve(b"", false);
        let dir = tempfile::tempdir().unwrap();
        let cancel = Cancel::default();
        cancel.set();
        let result =
            http_api(&conf).download(1, "a.gba", &[], &dir.path().join("a.gba"), &cancel);
        assert_eq!(result, Err(ApiError::Cancelled));
        assert!(!dir.path().join("a.gba").exists());
    }

    #[test]
    fn unwritable_target_is_a_write_error() {
        let (conf, _rx) = serve(
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc",
            false,
        );
        let dir = tempfile::tempdir().unwrap();
        let result = http_api(&conf).download(
            1,
            "a.gba",
            &[],
            &dir.path().join("missing").join("a.gba"),
            &Cancel::default(),
        );
        assert!(matches!(result, Err(ApiError::Write(_))));
    }
}
