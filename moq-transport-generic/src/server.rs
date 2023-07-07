use std::{time, sync, net::SocketAddr};

use moq_transport::{SetupServer, SetupClient};

use crate::{Control, Objects};

pub enum GenericServer {
    Quinn(moq_transport_quinn::Server),
    Quiche(moq_transport_quiche::Server),
}

impl GenericServer {
    pub fn new_quinn(tls_config: rustls::ServerConfig, bind_addr: SocketAddr) -> anyhow::Result<Self> {
		let mut server_config = quinn::ServerConfig::with_crypto(sync::Arc::new(tls_config));

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(2)));
		transport_config.congestion_controller_factory(sync::Arc::new(quinn::congestion::BbrConfig::default()));

		server_config.transport = sync::Arc::new(transport_config);
		let server = quinn::Endpoint::server(server_config, bind_addr)?;

		Ok(GenericServer::Quinn(moq_transport_quinn::Server::new(server)))
    }

    pub fn new_quiche(cert: &str, key: &str, bind_addr: SocketAddr) -> anyhow::Result<Self> {
        Ok(GenericServer::Quiche(moq_transport_quiche::Server::new(cert, key, bind_addr)?))
    }

    pub async fn accept(&mut self) -> anyhow::Result<Connect> {
        match self {
            Self::Quinn(server) => Ok(Connect::Quinn(server.accept().await?)),
            Self::Quiche(server) => Ok(Connect::Quiche(server.accept().await?)),
        }
    }
}

pub enum Connect {
    Quinn(moq_transport_quinn::Connect),
    Quiche(moq_transport_quiche::Connect),
}

impl Connect {
    pub async fn accept(self) -> anyhow::Result<Setup> {
        match self {
            Self::Quinn(server) => Ok(Setup::Quinn(server.accept().await?)),
            Self::Quiche(server) => Ok(Setup::Quiche(server.accept().await?)),
        }
    }
}

pub enum Setup {
    Quinn(moq_transport_quinn::Setup),
    Quiche(moq_transport_quiche::Setup),
}

impl Setup {
	// Return the setup message we received.
	pub fn setup(&self) -> &SetupClient {
        match self {
            Self::Quinn(setup) => setup.setup(),
            Self::Quiche(setup) => setup.setup(),
        }
	}

	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup: SetupServer) -> anyhow::Result<Session> {
        match self {
            Self::Quinn(s) => Ok(Session::Quinn(s.accept(setup).await?)),
            Self::Quiche(s) => Ok(Session::Quiche(s.accept(setup).await?)),
        }
	}

	pub async fn reject(self) -> anyhow::Result<()> {
        match self {
            Self::Quinn(setup) => setup.reject().await,
            Self::Quiche(setup) => setup.reject().await,
        }
	}

}

pub enum Session {
    Quinn(moq_transport_quinn::Session),
    Quiche(moq_transport_quiche::Session),
}

impl Session {
    pub fn split(self) -> (Control, Objects) {
        match self {
            Self::Quinn(session) => {
                let (control, objects) = session.split();
                (Control::Quinn(control), Objects::Quinn(objects))
            }
            Self::Quiche(session) => {
                let (control, objects) = session.split();
                (Control::Quiche(control), Objects::Quiche(objects))
            }
        }
    }
}