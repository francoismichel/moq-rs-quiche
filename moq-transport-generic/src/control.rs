use moq_transport::Message;


pub enum Control {
	Quinn(moq_transport_quinn::Control),
	Quiche(moq_transport_quiche::Control),
}

impl Control {

	pub fn split(self) -> (ControlSend, ControlRecv) {
		match self {
			Self::Quinn(control) => {
				let (send, recv) = control.split();
				(ControlSend::Quinn(send), ControlRecv::Quinn(recv))
			}
			Self::Quiche(control) => {
				let (send, recv) = control.split();
				(ControlSend::Quiche(send), ControlRecv::Quiche(recv))
			}
		}
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		match self {
			Self::Quinn(send) => send.send(msg).await,
			Self::Quiche(send) => send.send(msg).await,
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		match self {
			Self::Quinn(recv) => recv.recv().await,
			Self::Quiche(recv) => recv.recv().await,
		}
	}
}

pub enum ControlSend {
	Quinn(moq_transport_quinn::ControlSend),
	Quiche(moq_transport_quiche::ControlSend),
}

impl ControlSend {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		match self {
			Self::Quinn(control_send) => control_send.send(msg).await,
			Self::Quiche(control_send) => control_send.send(msg).await,
		}
	}

	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> ControlShared {
		match self {
			Self::Quinn(control_send) => ControlShared::Quinn(control_send.share()),
			Self::Quiche(control_send) => ControlShared::Quiche(control_send.share()),
		}
	}
}

// Helper that allows multiple threads to send control messages.
// There's no equivalent for receiving since only one thread should be receiving at a time.
#[derive(Clone)]
pub enum ControlShared {
	Quinn(moq_transport_quinn::ControlShared),
	Quiche(moq_transport_quiche::ControlShared),
}

impl ControlShared {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		match self {
			Self::Quinn(control_shared) => control_shared.send(msg).await,
			Self::Quiche(control_shared) => control_shared.send(msg).await,
		}
	}
}

pub enum ControlRecv {
	Quinn(moq_transport_quinn::ControlRecv),
	Quiche(moq_transport_quiche::ControlRecv),
}

impl ControlRecv {

	// Read the next full message from the stream.
	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		match self {
			Self::Quinn(control_recv) => control_recv.recv().await,
			Self::Quiche(control_recv) => control_recv.recv().await,
		}
	}
}
