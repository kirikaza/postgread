use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct Terminate {}

impl Terminate {
    pub const TYPE_BYTE: Option<u8> = Some(b'X');

    pub async fn read<R>(_stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::Terminate;
    use crate::msg::FrontendMessage;
    use crate::msg::test_util::*;

    #[test]
    fn simple() {
        let mut bytes: &[u8] = &[
            b'X',
            0, 0, 0, 4, // len
        ];
        assert_eq!(
            ok_some(Terminate {}),
            force_read_frontend(&mut bytes, false),
        );
    }

    fn ok_some(body: Terminate) -> Result<Option<FrontendMessage>, String> {
        ok_some_msg(body, FrontendMessage::Terminate)
    }
}