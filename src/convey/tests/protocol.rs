use super::fake_stream::{TwoFakeStreams};
use super::fake_tls::*;
use super::new_msg::*;

use crate::convey::{ConveyError::*, ConveyResult, Conveyor, Message};

use ::async_std::task;
use ::std::iter::Iterator;

macro_rules! backend {
    (
        $module:ident::$func:ident( $( $arg:expr ),* ),
        $conveyed:ident,
        $fake_streams:ident
    ) => {
        $fake_streams.push_backend($module::$func( $( $arg, )* ));
        let msg_holder_ = $module::$func( $( $arg, )* );
        $conveyed.push(Message::Backend($module::BackendMsg(&msg_holder_)));
    }
}

macro_rules! frontend {
    (
        $module:ident::$func:ident( $( $arg:expr ),* ),
        $conveyed:ident,
        $fake_streams:ident
    ) => {
        $fake_streams.push_frontend($module::$func( $( $arg, )* ));
        let msg_holder_ = $module::$func( $( $arg, )* );
        $conveyed.push(Message::Frontend($module::FrontendMsg(&msg_holder_)));
    }
}

#[test]
fn cancel() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::cancel(11, 12), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn cancel_when_backend_accepts_tls() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::tls(()), conveyed, streams);
    streams.backend_accepts_tls();
    streams.frontend_starts_tls();
    frontend!(initial::cancel(11, 12), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn cancel_when_backend_rejects_tls() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::tls(()), conveyed, streams);
    streams.backend_rejects_tls();
    streams.frontend_starts_tls();
    frontend!(initial::cancel(11, 12), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn backend_does_not_know_tls() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::tls(()), conveyed, streams);
    backend!(error_response::new("too old backend"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn error_after_startup() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn negotiate_and_error_after_startup() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(negotiate_protocol_version::new(21, &["_pq_x", "_pq_y"]), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn error_after_startup_when_backend_accepts_tls() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::tls(()), conveyed, streams);
    streams.backend_accepts_tls();
    streams.frontend_starts_tls();
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn error_after_startup_when_backend_rejects_tls() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::tls(()), conveyed, streams);
    streams.backend_rejects_tls();
    streams.frontend_starts_tls();
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn auth_ok_and_error() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn auth_cleartext_with_correct_password() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::cleartext_password(()), conveyed, streams);
    frontend!(password::new("correct"), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(error_response::new("shorten test"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn auth_cleartext_with_wrong_password() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::cleartext_password(()), conveyed, streams);
    frontend!(password::new("wrong"), conveyed, streams);
    backend!(error_response::new("wrong password"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn auth_md5_with_correct_password() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::md5_password(b"salt"), conveyed, streams);
    frontend!(password::new("md5correct"), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(error_response::new("shorten test"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn auth_md5_with_wrong_password() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::md5_password(b"salt"), conveyed, streams);
    frontend!(password::new("md5wrong"), conveyed, streams);
    backend!(error_response::new("wrong password"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn auth_kerberos_unsupported() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::kerberos_v5(()), conveyed, streams);
    assert_matches!(test_convey(conveyed, streams), Err(Unsupported(_)));
}

#[test]
fn auth_scm_credential_unsupported() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::scm_credential(()), conveyed, streams);
    assert_matches!(test_convey(conveyed, streams), Err(Unsupported(_)));
}

#[test]
fn negotiate_and_error_after_auth_ok() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(negotiate_protocol_version::new(21, &["_pq_x", "_pq_y"]), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn negotiate_before_and_after_auth_ok() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(negotiate_protocol_version::new(21, &["_pq_x", "_pq_y"]), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(negotiate_protocol_version::new(22, &["_pq_z", "_pq_t"]), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn error_after_param_status() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(parameter_status::new("param1", "value A"), conveyed, streams);
    backend!(parameter_status::new("param2", "value B"), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn error_after_backend_key() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(parameter_status::new("param1", "value A"), conveyed, streams);
    backend!(parameter_status::new("param2", "value B"), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn error_after_ready() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(parameter_status::new("param1", "value A"), conveyed, streams);
    backend!(parameter_status::new("param2", "value B"), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    backend!(error_response::new("something goes wrong"), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn ready_and_terminate() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(parameter_status::new("param1", "value A"), conveyed, streams);
    backend!(parameter_status::new("param2", "value B"), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn ready_and_terminate_with_notices() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(notice_response::new("first"), conveyed, streams);
    backend!(parameter_status::new("param1", "value A"), conveyed, streams);
    backend!(notice_response::new("second"), conveyed, streams);
    backend!(parameter_status::new("param2", "value B"), conveyed, streams);
    backend!(notice_response::new("third"), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
     backend!(notice_response::new("forth"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    backend!(notice_response::new("fifth"), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn empty_query() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new(""), conveyed, streams);
    backend!(empty_query_response::new(()), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn simple_select_query() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("select 2+3 as sum, null as nil"), conveyed, streams);
    backend!(notice_response::new("first"), conveyed, streams);
    backend!(row_description::fields(&["sum", "nil"]), conveyed, streams);
    backend!(notice_response::new("second"), conveyed, streams);
    backend!(data_row::columns(&[Some("5"), None]), conveyed, streams);
    backend!(notice_response::new("third"), conveyed, streams);
    backend!(command_complete::new("SELECT 1"), conveyed, streams);
    backend!(notice_response::new("forth"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    backend!(notice_response::new("fifth"), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn simple_select_query_with_notices() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("select 2+3 as sum, null as nil"), conveyed, streams);
    backend!(row_description::fields(&["sum", "nil"]), conveyed, streams);
    backend!(data_row::columns(&[Some("5"), None]), conveyed, streams);
    backend!(command_complete::new("SELECT 1"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn single_changing_query() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("insert values('new') into t1"), conveyed, streams);
    backend!(command_complete::new("INSERT 1"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn multiple_statements_in_query() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("select 2+3 as sum; update t1 set key=0; delete from t1; select null as nil"), conveyed, streams);
    backend!(row_description::fields(&["sum"]), conveyed, streams);
    backend!(data_row::columns(&[Some("5"), None]), conveyed, streams);
    backend!(command_complete::new("SELECT 1"), conveyed, streams);
    backend!(command_complete::new("UPDATE 9000"), conveyed, streams);
    backend!(command_complete::new("DELETE 9000"), conveyed, streams);
    backend!(row_description::fields(&["nil"]), conveyed, streams);
    backend!(data_row::columns(&[Some("null"), None]), conveyed, streams);
    backend!(command_complete::new("SELECT 1"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn multiple_statements_in_query_with_error_between() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("select 2+3 as sum; update t1 set key=0; delete from t1; select null as nil"), conveyed, streams);
    backend!(row_description::fields(&["sum"]), conveyed, streams);
    backend!(data_row::columns(&[Some("5"), None]), conveyed, streams);
    backend!(command_complete::new("SELECT 1"), conveyed, streams);
    backend!(error_response::new("relation \"t1\" does not exist"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

#[test]
fn multiple_queries_with_error_between() {
    let mut streams = TwoFakeStreams::new();
    let mut conveyed = vec![];
    frontend!(initial::startup(11, 12, hashmap!{}), conveyed, streams);
    backend!(authentication::ok(()), conveyed, streams);
    backend!(backend_key_data::new(21, 22), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("update t1 set key=0"), conveyed, streams);
    backend!(error_response::new("relation \"t1\" does not exist"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(query::new("select 2+3 as sum"), conveyed, streams);
    backend!(row_description::fields(&["sum"]), conveyed, streams);
    backend!(data_row::columns(&[Some("5")]), conveyed, streams);
    backend!(command_complete::new("SELECT 1"), conveyed, streams);
    backend!(ready_for_query::idle(()), conveyed, streams);
    frontend!(terminate::new(()), conveyed, streams);
    assert_ok!(test_convey(conveyed, streams));
}

fn test_convey(
    expected_conveyed: Vec<Message>,
    mut fake_streams: TwoFakeStreams,
) -> ConveyResult<()> {
    let mut expected_conveyed = expected_conveyed.iter();
    let mut conveyor = Conveyor::new(
        fake_streams.frontend_stream(),
        fake_streams.backend_stream(),
        FakeTlsServer(),
        FakeTlsClient(),
        |msg| { assert_eq!(expected_conveyed.next(), Some(&msg)) },
    );
    let convey_result = task::block_on(conveyor.go());
    assert!(expected_conveyed.len() == 0,
        "expected but not conveyed {:?}", expected_conveyed.collect::<Vec<_>>()
    );
    let unread = fake_streams.untaken();
    assert!(unread.is_empty(), "untaken messages {:?}", unread);
    convey_result
}
