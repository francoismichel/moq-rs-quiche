use std::{sync::{Arc, Mutex}, collections::{HashMap}, net::SocketAddr};

use anyhow::{anyhow, Context};
use async_webtransport_handler::{regex::Regex, AsyncWebTransportServer, ServerRef};
use moq_transport::{SetupClient, Message, SetupServer};
use tokio::sync::mpsc::{Sender, Receiver};
use webtransport_quiche::quiche;

use crate::{Control, Objects};


struct ConnectionState {
                        // session_id, receiver_for_bidi, receiver_for_uni  
    new_session_tx: Sender<u64>,

}

pub struct Server {
	// The QUIC server, yielding new connections and sessions.
	pub server: async_webtransport_handler::ServerRef,
	pub socket: Arc<tokio::net::UdpSocket>,
    pub new_connection_rx: Receiver<(Vec<u8>, Receiver<u64>)>,
    // uri_regexes: Vec<Regex>,
    // new_webtransport_sessions_per_client: HashMap<u64, tokio::sync::mpsc::Sender<u64>>,
    join_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl Server {
    pub fn new(cert: &str, key: &str, bind_addr: SocketAddr) -> anyhow::Result<Self> {

		let mut quic_config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();

		quic_config.load_cert_chain_from_pem_file(cert).unwrap();
		quic_config.load_priv_key_from_pem_file(key).unwrap();
		quic_config
			.set_application_protos(quiche::h3::APPLICATION_PROTOCOL)
			.unwrap();
		
		quic_config.set_cc_algorithm_name("cubic").unwrap();
		quic_config.set_max_idle_timeout(10000);
		quic_config.set_max_recv_udp_payload_size(1200);
		quic_config.set_max_send_udp_payload_size(1200);
		quic_config.set_initial_max_data(1_000_000_000);
		quic_config.set_initial_max_stream_data_bidi_local(100_000_000);
		quic_config.set_initial_max_stream_data_bidi_remote(100_000_000);
		quic_config.set_initial_max_stream_data_uni(100_000_000);
		quic_config.set_initial_max_streams_bidi(1_000_000);
		quic_config.set_initial_max_streams_uni(1_000_000);
		quic_config.set_disable_active_migration(true);
		quic_config.enable_early_data();
		quic_config.grease(false);
		// quic_config.set_fec_scheduler_algorithm(quiche::FECSchedulerAlgorithm::BurstsOnly);
		// quic_config.send_fec(args.get_bool("--send-fec"));
		// quic_config.receive_fec(args.get_bool("--receive-fec"));
		// quic_config.set_real_time(args.get_bool("--real-time-cc"));
		let h3_config = quiche::h3::Config::new().unwrap();
		
		let keylog = if let Some(keylog_path) = std::env::var_os("SSLKEYLOGFILE") {
			let file = std::fs::OpenOptions::new()
				.create(true)
				.append(true)
				.open(keylog_path)
				.unwrap();
	
			Some(file)
		} else {
			None
		};
	
		let (server, socket) = AsyncWebTransportServer::with_configs(bind_addr,
			quic_config, h3_config, keylog)?;
		let uri_root = "/";

        let (new_connection_tx, new_connection_rx) = tokio::sync::mpsc::channel(10);
        // let (new_stream_for_session_tx, new_stream_for_session_rx) = tokio::sync::mpsc::channel(10);

        let server_ref = Arc::new(Mutex::new(server));

        let task_server_ref = server_ref.clone();
		let regexes = [Regex::new(format!("{}", uri_root).as_str()).unwrap()];
		let task_regexes = regexes.clone();
        let socket = Arc::new(socket);
        let task_socket = socket.clone();
        let join_handle = tokio::task::spawn(async move {
            let mut connection_states: HashMap<Vec<u8>, ConnectionState> = HashMap::new();
            let mut buf = vec![0; 65000];
            'mainloop: loop {
                let cid = {
                    // let mut server = endpoint.quiche_server.lock().await;
                    match async_webtransport_handler::AsyncWebTransportServer::listen_ref(task_server_ref.clone(), task_socket.clone(), &mut buf).await? {
                        Some(cid) => cid,
                        None => continue 'mainloop,
                    }
                };

                if !connection_states.contains_key(&cid) {
                    let (new_session_tx, new_session_rx) = tokio::sync::mpsc::channel(10);
                    let connection_state = ConnectionState {
                        new_session_tx,
                    };
                    connection_states.insert(cid.clone(), connection_state);
                    new_connection_tx.send((cid.clone(), new_session_rx)).await?;
                }
                let connection_state = connection_states.get_mut(&cid).ok_or(anyhow::Error::msg("connection not found"))?;

                loop {
                    let polled = task_server_ref.lock().unwrap().poll(&cid, &task_regexes);
                    match polled {
                        Ok(async_webtransport_handler::Event::NewSession(_path, session_id, _regex_index)) => {
                            connection_state.new_session_tx.send(session_id).await?;
                        },
                        Ok(async_webtransport_handler::Event::StreamData(_session_id, _stream_id)) => {
                            // TODO: remove this at some point, the async server will not need to trigger events etc.
							log::debug!("Received stream data")
                        },
                        Ok(async_webtransport_handler::Event::Done) => {
                            log::debug!("H3 Done");
                            break;
                        },
                        Ok(async_webtransport_handler::Event::GoAway) => {
                            log::debug!("GOAWAY");
                            break;
                        },

                        Err(_) => todo!(),
                    }
                }
            }
        });

		Ok(Self {
			server: server_ref,
            socket,
            new_connection_rx,
            join_handle,
		})
    }


	// Accept new QUIC connection.
	pub async fn accept(&mut self) -> anyhow::Result<Connect> {
        let (cid, new_session_rx) = self.new_connection_rx.recv().await.ok_or(anyhow::Error::msg("new_connection channel closed"))?;
            
        Ok(Connect{server: self.server.clone(), cid, new_session_rx})
    }
}

pub struct Connect {
    server: ServerRef,
    cid: Vec<u8>,
    new_session_rx: Receiver<u64>,
}

impl Connect {
    pub async fn accept(mut self) -> anyhow::Result<Setup> {
        let session_id = self.new_session_rx.recv().await.ok_or(anyhow!("could not recv on channel"))?;

		let stream_id = async_webtransport_handler::AsyncWebTransportServer::accept_bidi_stream_ref(
			self.server.clone(),
			&self.cid,
			session_id)
			.await
			.unwrap();

		let stream = async_webtransport_handler::ServerBidiStream::new(
			self.server.clone(),
			self.cid.clone(),
			session_id,
			stream_id
		);

		let session = WebTransportSession{
			server: self.server.clone(),
			cid: self.cid.clone(),
			session_id,
		};
		let objects = Objects::new(Arc::new(session));

		let mut control = Control::new(stream);
		let setup = match control.recv().await.context("failed to read SETUP")? {
			Message::SetupClient(setup) => setup,
			_ => anyhow::bail!("expected CLIENT SETUP"),
		};

		// Let the application decide if we accept this MoQ session.
		Ok(Setup {
			setup,
			control,
			objects,
		})
    }
}

pub struct WebTransportSession {
	pub server: ServerRef,
	pub session_id: u64,
	pub cid: Vec<u8>,
}

pub struct Setup {
	setup: SetupClient,
	control: Control,
	objects: Objects,
}


impl Setup {
	// Return the setup message we received.
	pub fn setup(&self) -> &SetupClient {
		&self.setup
	}

	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup: SetupServer) -> anyhow::Result<Session> {
		self.control.send(setup).await?;
		Ok(Session {
			control: self.control,
			objects: self.objects,
		})
	}

	pub async fn reject(self) -> anyhow::Result<()> {
		// TODO Close the QUIC connection with an error code.
		Ok(())
	}
}

pub struct Session {
	pub control: Control,
	pub objects: Objects,
}

impl Session {
	pub fn split(self) -> (Control, Objects) {
		(self.control, self.objects)
	}
}
