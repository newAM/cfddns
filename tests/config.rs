use assert_cmd::Command;
use std::io::Write;
use tempfile::NamedTempFile;

fn main_bin() -> Command {
    Command::cargo_bin(assert_cmd::crate_name!()).unwrap()
}

#[test]
fn no_config_file() {
    main_bin().assert().stderr(
        predicates::str::is_match("usage: \\S+cfddns \\[config-file\\.json\\]\n")
            .unwrap()
            .count(1),
    );
}

#[test]
fn bad_config_file() {
    let mut config_file: NamedTempFile = NamedTempFile::new().unwrap();
    config_file.write_all(&[0xFF]).unwrap();
    config_file.flush().unwrap();

    main_bin().args([config_file.path()]).assert().stderr(
        r#"Error: Failed to deserialize config file

Caused by:
    expected value at line 1 column 1
"#,
    );
}
#[test]
fn deny_unknown_fields() {
    const MOCK_CONFIG: &str = r#"{
        "a_interface": "bond-wan",
        "aaaa_interface": "br-lan",
        "zones": [],
        "history_path": "",
        "log_level": "off",
        "some_extra_field": 1
    }"#;

    let mut config_file: NamedTempFile = NamedTempFile::new().unwrap();
    config_file.write_all(MOCK_CONFIG.as_bytes()).unwrap();
    config_file.flush().unwrap();

    main_bin()
        .args([config_file.path()])
        .assert()
        .stderr(predicates::str::starts_with(
            r#"Error: Failed to deserialize config file

Caused by:
    unknown field `some_extra_field`, expected one of"#,
        ));
}

#[test]
fn no_client_secret() {
    const MOCK_CONFIG: &str = r#"{
        "a_interface": "bond-wan",
        "aaaa_interface": "br-lan",
        "zones": [],
        "history_path": "",
        "log_level": "off"
    }"#;

    let mut config_file: NamedTempFile = NamedTempFile::new().unwrap();
    config_file.write_all(MOCK_CONFIG.as_bytes()).unwrap();
    config_file.flush().unwrap();

    main_bin().args([config_file.path()]).assert().stderr(
        r#"Error: Failed to read cloudflare API token from environment variable 'CLOUDFLARE_TOKEN'

Caused by:
    environment variable not found
"#,
    );
}

#[test]
fn no_zones_early_return() {
    const MOCK_CONFIG: &str = r#"{
        "a_interface": "bond-wan",
        "aaaa_interface": "br-lan",
        "zones": [],
        "history_path": "/tmp/rmme",
        "log_level": "off"
    }"#;

    let mut config_file: NamedTempFile = NamedTempFile::new().unwrap();
    config_file.write_all(MOCK_CONFIG.as_bytes()).unwrap();
    config_file.flush().unwrap();

    main_bin()
        .args([config_file.path()])
        .env("CLOUDFLARE_TOKEN", "AAA")
        .assert()
        .code(0);
}

#[test]
fn no_ipv4_or_ipv6_early_return() {
    const MOCK_CONFIG: &str = r#"{
        "a_interface": null,
        "aaaa_interface": null,
        "zones": [
            {
                "name": "myzone",
                "records": []
            }
        ],
        "history_path": "/tmp/rmme",
        "log_level": "off"
    }"#;

    let mut config_file: NamedTempFile = NamedTempFile::new().unwrap();
    config_file.write_all(MOCK_CONFIG.as_bytes()).unwrap();
    config_file.flush().unwrap();

    main_bin()
        .args([config_file.path()])
        .env("CLOUDFLARE_TOKEN", "AAA")
        .assert()
        .code(0);
}

#[test]
fn missing_history() {
    const MOCK_CONFIG: &str = r#"{
        "a_interface": "bond-wan",
        "aaaa_interface": "br-lan",
        "zones": [
            {
                "name": "myzone",
                "records": []
            }
        ],
        "history_path": "",
        "log_level": "off"
    }"#;

    let mut config_file: NamedTempFile = NamedTempFile::new().unwrap();
    config_file.write_all(MOCK_CONFIG.as_bytes()).unwrap();
    config_file.flush().unwrap();

    main_bin()
        .args([config_file.path()])
        .env("CLOUDFLARE_TOKEN", "AAA")
        .assert()
        .stderr(
            r#"Error: Failed to create initial history file at ''

Caused by:
    0: Failed to open history file at '' for writing
    1: No such file or directory (os error 2)
"#,
        );
}
