use assert_cmd::Command;
use assert_fs::fixture::TempDir;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test1() {
    let journal_dir = TempDir::new().unwrap();

    let monero_mock = MockServer::start().await;
    let shelly_mock = MockServer::start().await;

    mount_shelly(shelly_mock);

    let mut cmd = Command::cargo_bin("cipo").unwrap();
    let assert = cmd
        .arg("--journal")
        .arg(journal_dir.as_os_str())
        .arg("--config")
        .arg("/home/jonny/projects/cipo/docs/example-config.toml")
        .timeout(std::time::Duration::from_secs(22))
        .assert();

    assert.interrupted();
}

fn mount_shelly(mock: MockServer) {
    // On and off
    Mock::given(method("GET"))
        .and(path("/rpc/Switch.Set"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock);

    Mock::given(method("GET"))
        .and(path("/rpc/Switch.GetStatus"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&mock);
}
