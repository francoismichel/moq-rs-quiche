use moq_transport::{Object};
use tokio::io::{AsyncRead, AsyncWrite};

// TODO support clients

pub enum SendStream {
	Quinn(moq_transport_quinn::SendStream),
	Quiche(moq_transport_quiche::SendStream),
}

impl AsyncWrite for SendStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
		let stream = self.get_mut();
		match stream {
			Self::Quinn(stream) => {
				tokio::pin!(stream);
				stream.poll_write(cx, buf)
			},
			Self::Quiche(stream) => {
				tokio::pin!(stream);
				stream.poll_write(cx, buf)
			},
		}
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
		let stream = self.get_mut();
		match stream {
			Self::Quinn(stream) => {
				tokio::pin!(stream);
				stream.poll_flush(cx)
			},
			Self::Quiche(stream) => {
				tokio::pin!(stream);
				stream.poll_flush(cx)
			},
		}
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
		let stream = self.get_mut();
		match stream {
			Self::Quinn(stream) => {
				tokio::pin!(stream);
				stream.poll_shutdown(cx)
			},
			Self::Quiche(stream) => {
				tokio::pin!(stream);
				stream.poll_shutdown(cx)
			},
		}
    }
}
pub enum RecvStream {
	Quinn(moq_transport_quinn::RecvStream),
	Quiche(moq_transport_quiche::RecvStream),
}

impl AsyncRead for RecvStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
		let stream = self.get_mut();
		match stream {
			Self::Quinn(stream) => {
				tokio::pin!(stream);
				stream.poll_read(cx, buf)
			},
			Self::Quiche(stream) => {
				tokio::pin!(stream);
				stream.poll_read(cx, buf)
			},
		}
    }
}


pub enum Objects {
	Quinn(moq_transport_quinn::Objects),
	Quiche(moq_transport_quiche::Objects),
}

impl Objects {

	pub fn split(self) -> (SendObjects, RecvObjects) {
		match self {
			Self::Quinn(objects) => {
				let (send, recv) = objects.split();
				(SendObjects::Quinn(send), RecvObjects::Quinn(recv))
			}
			Self::Quiche(objects) => {
				let (send, recv) = objects.split();
				(SendObjects::Quiche(send), RecvObjects::Quiche(recv))
			}
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, RecvStream)> {
		match self {
			Self::Quinn(recv_object) => {
				let (object, recv_stream) = recv_object.recv().await?;
				Ok((object, RecvStream::Quinn(recv_stream)))
			}
			Self::Quiche(recv_object) => {
				let (object, recv_stream) = recv_object.recv().await?;
				Ok((object, RecvStream::Quiche(recv_stream)))
			}
		}
	}

	pub async fn send(&mut self, header: Object) -> anyhow::Result<SendStream> {
		match self {
			Self::Quinn(send) => Ok(SendStream::Quinn(send.send(header).await?)),
			Self::Quiche(send) => Ok(SendStream::Quiche(send.send(header).await?)),
		}
	}
}

pub enum SendObjects {
	Quinn(moq_transport_quinn::SendObjects),
	Quiche(moq_transport_quiche::SendObjects),
}

impl SendObjects {

	pub async fn send(&mut self, header: Object) -> anyhow::Result<SendStream> {
		match self {
			Self::Quinn(send_object) => Ok(SendStream::Quinn(send_object.send(header).await?)),
			Self::Quiche(send_object) => Ok(SendStream::Quiche(send_object.send(header).await?)),
		}
	}
}

impl Clone for SendObjects {
	fn clone(&self) -> Self {
		match self {
			Self::Quinn(send_object) => Self::Quinn(send_object.clone()),
			Self::Quiche(send_object) => Self::Quiche(send_object.clone()),
		}
	}
}

// Not clone, so we don't accidentally have two listners.
pub enum RecvObjects {
	Quinn(moq_transport_quinn::RecvObjects),
	Quiche(moq_transport_quiche::RecvObjects),
}

impl RecvObjects {

	pub async fn recv(&mut self) -> anyhow::Result<(Object, RecvStream)> {
		match self {
			Self::Quinn(recv_object) => {
				let (object, recv_stream) = recv_object.recv().await?;
				Ok((object, RecvStream::Quinn(recv_stream)))
			}
			Self::Quiche(recv_object) => {
				let (object, recv_stream) = recv_object.recv().await?;
				Ok((object, RecvStream::Quiche(recv_stream)))
			}
		}
	}
}
