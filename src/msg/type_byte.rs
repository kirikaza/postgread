use num_enum::{IntoPrimitive, TryFromPrimitive};

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum TypeByte {
    Authentication = b'R',
    BackendKeyData = b'K',
    Bind = b'B',
    BindComplete = b'2',
    CommandComplete = b'C',
    DataRow = b'D',
    EmptyQueryResponse = b'I',
    Execute_or_ErrorResponse = b'E',
    NoticeResponse = b'N',
    GssResponse_or_Password_or_SaslResponses = b'p',
    NegotiateProtocolVersion = b'v',
    ParameterStatus_or_Sync = b'S',
    Parse = b'P',
    ParseComplete = b'1',
    PortalSuspended = b's',
    Query = b'Q',
    ReadyForQuery = b'Z',
    RowDescription = b'T',
    Terminate = b'X',
}
