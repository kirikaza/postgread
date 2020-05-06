macro_rules! export_wrapper {
    ($enum:ident::$variant:ident) => {
        pub use crate::convey::$enum::$variant as $enum;
    };
}

pub mod authentication {
    use crate::msg::body::authentication::*;
    export_wrapper!(BackendMsg::Authentication);

    pub fn ok(_: ()) -> Authentication {
        Authentication::Ok
    }

    pub fn cleartext_password(_: ()) -> Authentication {
        Authentication::CleartextPassword
    }

    pub fn kerberos_v5(_: ()) -> Authentication {
        Authentication::KerberosV5
    }
}

pub mod backend_key_data {
    use crate::msg::body::backend_key_data::*;
    export_wrapper!(BackendMsg::BackendKeyData);

    pub fn new(process_id: u32, secret_key: u32) -> BackendKeyData {
        BackendKeyData {
            process_id,
            secret_key
        }
    }
}

pub mod command_complete {
    use crate::msg::body::command_complete::*;
    export_wrapper!(BackendMsg::CommandComplete);

    pub fn new(tag: &'static str) -> CommandComplete {
        CommandComplete {
            tag: tag.into(),
        }
    }
}

pub mod data_row {
    use crate::msg::body::data_row::*;
    export_wrapper!(BackendMsg::DataRow);

    pub fn columns(values: &[Option<&'static str>]) -> DataRow {
        use Column::*;
        let columns = values.iter().map(|opt| {
            opt.map_or(Null, |str| Value(str.into()))
        }).collect();
        DataRow { columns }
    }
}

pub mod empty_query_response {
    use crate::msg::body::empty_query_response::*;
    export_wrapper!(BackendMsg::EmptyQueryResponse);

    pub fn new(_: ()) -> EmptyQueryResponse {
        EmptyQueryResponse {}
    }
}

pub mod error_response {
    use crate::msg::body::error_and_notice_responses::*;
    export_wrapper!(BackendMsg::ErrorResponse);

    pub fn new(message: &'static str) -> ErrorResponse {
        ErrorResponse(ErrorOrNoticeFields {
            severity: Some("ERROR".into()),
            message: Some(message.into()),
            ..Default::default()
        })
    }
}

pub mod notice_response {
    use crate::msg::body::error_and_notice_responses::*;
    export_wrapper!(BackendMsg::NoticeResponse);

    pub fn new(message: &'static str) -> NoticeResponse {
        NoticeResponse(ErrorOrNoticeFields {
            severity: Some("NOTICE".into()),
            message: Some(message.into()),
            ..Default::default()
        })
    }
}


pub mod initial {
    use crate::msg::body::initial::*;
    use ::std::collections::HashMap;
    export_wrapper!(FrontendMsg::Initial);

    pub fn startup(
        major: u16,
        minor: u16,
        params: HashMap<&'static str, &'static str>
    ) -> Initial {
        let to_startup_param = |(name, value): (&&'static str, &&'static str)| {
            StartupParam {
                name: (*name).into(),
                value: (*value).into(),
            }
        };
        Initial::Startup(Startup {
            version: Version { major, minor },
            params: params.iter().map(to_startup_param).collect(),
        })
    }

    pub fn cancel(process_id: u32, secret_key: u32) -> Initial {
        Initial::Cancel(Cancel {
            process_id,
            secret_key,
        })
    }

    pub fn tls(_: ()) -> Initial {
        Initial::TLS
    }
}

pub mod negotiate_protocol_version {
    use crate::msg::body::negotiate_protocol_version::*;
    export_wrapper!(BackendMsg::NegotiateProtocolVersion);

    pub fn new(newest_backend_minor: u32, unrecognized_options: &[&'static str]) -> NegotiateProtocolVersion {
        let unrecognized_options = unrecognized_options.iter().map(|str| (*str).into()).collect();
        NegotiateProtocolVersion {
            newest_backend_minor,
            unrecognized_options,
        }
    }
}

pub mod parameter_status {
    use crate::msg::body::parameter_status::*;
    export_wrapper!(BackendMsg::ParameterStatus);

    pub fn new(name: &'static str, value: &'static str) -> ParameterStatus {
        ParameterStatus {
            name: name.into(),
            value: value.into(),
        }
    }
}

pub mod password {
    use crate::msg::body::password::*;
    export_wrapper!(FrontendMsg::Password);

    pub fn new(password: &'static str) -> Password {
        Password(password.into())
    }
}

pub mod query {
    use crate::msg::body::query::*;
    export_wrapper!(FrontendMsg::Query);

    pub fn new(sql: &'static str) -> Query {
       Query(sql.into())
    }
}

pub mod ready_for_query {
    use crate::msg::body::ready_for_query::*;
    export_wrapper!(BackendMsg::ReadyForQuery);

    pub fn idle(_: ()) -> ReadyForQuery {
        ReadyForQuery { status: Status::Idle }
    }
}

pub mod row_description {
    use crate::msg::body::row_description::*;
    export_wrapper!(BackendMsg::RowDescription);

    pub fn fields(names: &[&'static str]) -> RowDescription {
        let fields = names.iter().enumerate().map(|(i, name)| Field {
            name: (*name).into(),
            column_attr_num: i as u16,
            type_size: 10 * i as i16,
            column_oid: 100 * i as u32,
            type_oid: 1000 * i as u32,
            type_modifier: 10000 * i as i32,
            format: Format::Text,
        }).collect();
        RowDescription { fields }
    }
}

pub mod terminate {
    use crate::msg::body::terminate::*;
    export_wrapper!(FrontendMsg::Terminate);

    pub fn new(_: ()) -> Terminate {
        Terminate {}
    }
}