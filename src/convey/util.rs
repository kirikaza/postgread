use crate::msg::body::*;
use crate::convey::{Message, BackendMsg, FrontendMsg};

#[derive(Debug, PartialEq)]
pub enum MessageClone {
    Backend(BackendMsgClone),
    Frontend(FrontendMsgClone),
}

#[derive(Debug, PartialEq)]
pub enum BackendMsgClone {
    Authentication(Authentication),
    BackendKeyData(BackendKeyData),
    BindComplete(BindComplete),
    CommandComplete(CommandComplete),
    DataRow(DataRow),
    EmptyQueryResponse(EmptyQueryResponse),
    ErrorResponse(ErrorResponse),
    NegotiateProtocolVersion(NegotiateProtocolVersion),
    NoticeResponse(NoticeResponse),
    ParameterStatus(ParameterStatus),
    ParseComplete(ParseComplete),
    PortalSuspended(PortalSuspended),
    ReadyForQuery(ReadyForQuery),
    RowDescription(RowDescription),
}

#[derive(Debug, PartialEq)]
pub enum FrontendMsgClone {
    Bind(Bind),
    Execute(Execute),
    GssResponse(GssResponse),
    Initial(Initial),
    Parse(Parse),
    Password(Password),
    Query(Query),
    SaslInitialResponse(SaslInitialResponse),
    SaslResponse(SaslResponse),
    Sync(Sync),
    Terminate(Terminate),
}

impl MessageClone {
    pub fn make(refer: Message) -> Self {
        use Message as Ref;
        use MessageClone::*;
        match refer {
            Ref::Backend(refer) => Backend(BackendMsgClone::make(refer)),
            Ref::Frontend(refer) => Frontend(FrontendMsgClone::make(refer)),
        }
    }
}

impl BackendMsgClone {
    fn make(refer: BackendMsg) -> Self {
        use BackendMsg as Ref;
        use BackendMsgClone::*;
        match refer {
            Ref::Authentication(refer) => Authentication((*refer).clone()),
            Ref::BackendKeyData(refer) => BackendKeyData((*refer).clone()),
            Ref::BindComplete(refer) => BindComplete((*refer).clone()),
            Ref::CommandComplete(refer) => CommandComplete((*refer).clone()),
            Ref::DataRow(refer) => DataRow((*refer).clone()),
            Ref::EmptyQueryResponse(refer) => EmptyQueryResponse((*refer).clone()),
            Ref::ErrorResponse(refer) => ErrorResponse((*refer).clone()),
            Ref::NegotiateProtocolVersion(refer) => NegotiateProtocolVersion((*refer).clone()),
            Ref::NoticeResponse(refer) => NoticeResponse((*refer).clone()),
            Ref::ParameterStatus(refer) => ParameterStatus((*refer).clone()),
            Ref::ParseComplete(refer) => ParseComplete((*refer).clone()),
            Ref::PortalSuspended(refer) => PortalSuspended((*refer).clone()),
            Ref::ReadyForQuery(refer) => ReadyForQuery((*refer).clone()),
            Ref::RowDescription(refer) => RowDescription((*refer).clone()),
        }
    }
}

impl FrontendMsgClone {
    fn make(refer: FrontendMsg) -> Self {
        use FrontendMsg as Ref;
        use FrontendMsgClone::*;
        match refer {
            Ref::Bind(refer) => Bind((*refer).clone()),
            Ref::Execute(refer) => Execute((*refer).clone()),
            Ref::GssResponse(refer) => GssResponse((*refer).clone()),
            Ref::Initial(refer) => Initial((*refer).clone()),
            Ref::Parse(refer) => Parse((*refer).clone()),
            Ref::Password(refer) => Password((*refer).clone()),
            Ref::Query(refer) => Query((*refer).clone()),
            Ref::SaslInitialResponse(refer) => SaslInitialResponse((*refer).clone()),
            Ref::SaslResponse(refer) => SaslResponse((*refer).clone()),
            Ref::Sync(refer) => Sync((*refer).clone()),
            Ref::Terminate(refer) => Terminate((*refer).clone()),
        }        
    }
}