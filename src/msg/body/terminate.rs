use crate::msg::util::read::*;
use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct Terminate {}

impl Terminate {
    pub const TYPE_BYTE: u8 = b'X';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream, |_, _| Ok(Self {})).await
    }
}

#[cfg(test)]
mod tests {
    use super::Terminate;
    use crate::msg::FrontendMessage;
    use crate::msg::util::test::*;

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
