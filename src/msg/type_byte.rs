use num_enum::{IntoPrimitive, TryFromPrimitive};

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum TypeByte {
    Authentication = b'R',
    BackendKeyData = b'K',
    CommandComplete = b'C',
    DataRow = b'D',
    EmptyQueryResponse = b'I',
    ErrorResponse = b'E',
    NoticeResponse = b'N',
    GssResponse_Or_Password = b'p',
    NegotiateProtocolVersion = b'v',
    ParameterStatus = b'S',
    Query = b'Q',
    ReadyForQuery = b'Z',
    RowDescription = b'T',
    Terminate = b'X',
}
