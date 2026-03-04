use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use reference_tool_lib::bib_parser::parse_bib_entries;
use reference_tool_lib::state::AppState;
use reference_tool_lib::storage::Storage;

const SAMPLE_BIB: &str = r#"
@ARTICLE{10495806,
  author={Huang, Yong and Li, Ming and Yu, F. Richard and Si, Peng and Zhang, Hu and Qiao, Jia},
  journal={IEEE Transactions on Cognitive Communications and Networking},
  title={Resources Scheduling for Ambient Backscatter Communication-Based Intelligent IIoT: A Collective Deep Reinforcement Learning Method},
  year={2024},
  volume={10},
  number={2},
  pages={634-648},
  doi={10.1109/TCCN.2024.1234567}
}

@ARTICLE{10648348,
  author={Wang, Xin and Liu, Li and Tang, Tao and Sun, Wei},
  journal={IEEE Transactions on Intelligent Transportation Systems},
  title={Enhancing Communication-Based Train Control Systems Through Train-to-Train Communications},
  year={2019},
  volume={20},
  number={4},
  pages={1544-1561}
}
"#;

const SINGLE_ENTRY_BIB: &str = r#"
@ARTICLE{9750059,
  author={Liu, Xin and Yu, Yingfeng and Li, Feng and Durrani, Tariq S.},
  journal={IEEE Transactions on Intelligent Transportation Systems},
  title={Throughput Maximization for RIS-UAV Relaying Communications},
  year={2022},
  volume={23},
  number={10},
  pages={19569-19574}
}
"#;

fn unique_state_path(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after UNIX_EPOCH")
        .as_nanos();

    env::temp_dir().join(format!(
        "reference-tool-integration-{}-{}-{}.json",
        test_name,
        process::id(),
        nanos
    ))
}

fn cleanup_if_exists(path: &Path) {
    if path.exists() {
        std::fs::remove_file(path).expect("cleanup temporary integration test state file");
    }
}

#[test]
fn import_and_cite_workflow_persists_between_sessions() {
    let path = unique_state_path("persisted-workflow");
    let storage = Storage::new(path.clone());

    let mut first_session = AppState::from_storage(storage.clone())
        .expect("initial app state should be created from empty storage");

    let imported_entries = parse_bib_entries(SAMPLE_BIB).expect("sample bib should parse");
    let import_result = first_session
        .import_entries(imported_entries)
        .expect("import should succeed");

    assert_eq!(import_result.total, 2);
    assert_eq!(import_result.new_count, 2);
    assert_eq!(import_result.overwritten_count, 0);

    let first_cite = first_session
        .cite_keys("10648348,10495806")
        .expect("cite should resolve imported keys");

    assert_eq!(first_cite.citation_text, "[1]-[2]");
    assert_eq!(first_cite.newly_added_count, 2);
    assert!(first_cite
        .cited_references_text
        .contains("[1]  Wang X, Liu L"));
    assert!(first_cite
        .cited_references_text
        .contains("[2]  Huang Y, Li M"));

    drop(first_session);

    let mut second_session =
        AppState::from_storage(storage).expect("reloading app state should succeed");

    let snapshot = second_session.snapshot();
    assert_eq!(snapshot.total_entries, 2);
    assert_eq!(snapshot.citation_order, vec!["10648348", "10495806"]);

    let second_cite = second_session
        .cite_keys("10495806")
        .expect("existing citation key should resolve");

    assert_eq!(second_cite.citation_text, "[2]");
    assert_eq!(second_cite.newly_added_count, 0);

    cleanup_if_exists(&path);
}

#[test]
fn cite_transaction_rolls_back_on_missing_key() {
    let path = unique_state_path("transaction-rollback");
    let storage = Storage::new(path.clone());

    let mut app_state =
        AppState::from_storage(storage.clone()).expect("app state initialization should succeed");

    let imported_entries = parse_bib_entries(SINGLE_ENTRY_BIB).expect("single bib should parse");
    app_state
        .import_entries(imported_entries)
        .expect("single entry import should succeed");

    let before_snapshot = app_state.snapshot();
    assert!(before_snapshot.citation_order.is_empty());

    let error = app_state
        .cite_keys("9750059,missing-key")
        .expect_err("missing key should cause transaction failure");

    assert!(error.contains("Missing citation key(s): missing-key"));

    let after_snapshot = app_state.snapshot();
    assert_eq!(
        after_snapshot.citation_order,
        before_snapshot.citation_order
    );

    drop(app_state);

    let reloaded = AppState::from_storage(storage).expect("reloading app state should succeed");
    assert!(reloaded.snapshot().citation_order.is_empty());

    cleanup_if_exists(&path);
}
