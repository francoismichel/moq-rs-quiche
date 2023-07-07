use moq_transport::{Decode, DecodeError, Encode, Message};

use bytes::{Buf, BytesMut};

use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct Control {
	sender: ControlSend,
	recver: ControlRecv,
}

impl Control {
	pub(crate) fn new(stream: async_webtransport_handler::ServerBidiStream) -> Self {
		let (sender, recver) = stream.split();
		let sender = ControlSend::new(sender);
		let recver = ControlRecv::new(recver);

		Self { sender, recver }
	}

	pub fn split(self) -> (ControlSend, ControlRecv) {
		(self.sender, self.recver)
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		self.sender.send(msg).await
	}

	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		self.recver.recv().await
	}
}

pub struct ControlSend {
	stream: async_webtransport_handler::ServerSendStream,
	buf: BytesMut, // reuse a buffer to encode messages.
}

impl ControlSend {
	pub fn new(inner: async_webtransport_handler::ServerSendStream) -> Self {
		Self {
			buf: BytesMut::new(),
			stream: inner,
		}
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);

		self.buf.clear();
		msg.encode(&mut self.buf)?;

		// TODO make this work with select!
		self.stream.write_all(&self.buf).await?;

		Ok(())
	}

	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> ControlShared {
		ControlShared {
			stream: Arc::new(Mutex::new(self)),
		}
	}
}

// Helper that allows multiple threads to send control messages.
// There's no equivalent for receiving since only one thread should be receiving at a time.
#[derive(Clone)]
pub struct ControlShared {
	stream: Arc<Mutex<ControlSend>>,
}

impl ControlShared {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let mut stream = self.stream.lock().await;
		stream.send(msg).await
	}
}

pub struct ControlRecv {
	stream: async_webtransport_handler::ServerRecvStream,
	buf: BytesMut, // data we've read but haven't fully decoded yet
}

impl ControlRecv {
	pub fn new(inner: async_webtransport_handler::ServerRecvStream) -> Self {
		Self {
			buf: BytesMut::new(),
			stream: inner,
		}
	}

	// Read the next full message from the stream.
	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		loop {
			// Read the contents of the buffer
			let mut peek = Cursor::new(&self.buf);

			match Message::decode(&mut peek) {
				Ok(msg) => {
					// We've successfully decoded a message, so we can advance the buffer.
					self.buf.advance(peek.position() as usize);

					log::info!("received message: {:?}", msg);
					return Ok(msg);
				}
				Err(DecodeError::UnexpectedEnd) => {
					// The decode failed, so we need to append more data.
					self.stream.read_buf(&mut self.buf).await?;
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
