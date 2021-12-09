use std::{
    collections::HashMap,
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use parking_lot::{Mutex, RwLock};
use tokio::{net::TcpListener, sync::broadcast, task::JoinHandle};

use crate::{
    data_handle::DataHandle, message::Message, synchronizable::Synchronizable,
    ws_handler::websocket_handler,
};

const CHANNEL_SIZE: usize = 32;

pub struct DataElement {
    pub data: Arc<RwLock<Box<dyn Synchronizable>>>,
    pub on_change: Arc<RwLock<Vec<Box<dyn Fn() + Send + Sync>>>>,
}
pub type Store = Arc<Mutex<HashMap<String, DataElement>>>;

pub type BroadcastSender = broadcast::Sender<Message>;
pub type BroadcastReceiver = broadcast::Receiver<Message>;

pub struct Tero {
    state: ServerState,
    addr: SocketAddr,
    server_handle: Option<JoinHandle<()>>,
    handler_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    store: Store,
    broadcast: (BroadcastSender, BroadcastReceiver),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Up,
    Down,
}

impl Tero {
    pub fn data<T: Synchronizable>(&'static self, key: &str, data: T) -> DataHandle<T> {
        let guard = self.store.lock();
        if guard.contains_key(key) {
            panic!("Key {} already exists", key);
        }
        let data = DataElement {
            data: Arc::new(RwLock::new(data.clone_synchronizable())),
            on_change: Arc::new(RwLock::new(Vec::new())),
        };
        let sender = self.broadcast.0.clone();
        DataHandle {
            key: key.to_string(),
            sender,
            data_type: PhantomData::<T>,
            data_element: data,
            on_change: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn new(addr: impl ToSocketAddrs) -> Tero {
        let channel = broadcast::channel(CHANNEL_SIZE);
        Tero {
            state: ServerState::Down,
            addr: addr.to_socket_addrs().unwrap().next().unwrap(),
            server_handle: None,
            handler_handles: Arc::new(Mutex::new(Vec::new())),
            store: Arc::new(Mutex::new(HashMap::new())),
            broadcast: channel,
        }
    }

    pub fn get_state(&self) -> ServerState {
        self.state
    }

    pub async fn start(&mut self) {
        let socket = TcpListener::bind(self.addr).await;
        let listener = socket.expect("Failed to bind addr.");
        let store = self.store.clone();
        let handler_handles = self.handler_handles.clone();
        let broadcast_sender = self.broadcast.0.clone();
        let server_handle = tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                let store_clone = store.clone();
                let broadcast_receiver = broadcast_sender.subscribe();
                let new_handler = tokio::spawn(websocket_handler(
                    stream,
                    addr,
                    store_clone,
                    broadcast_receiver,
                ));
                handler_handles.lock().push(new_handler);
            }
        });
        self.server_handle = Some(server_handle);
        self.state = ServerState::Up;
    }

    pub fn stop(&mut self) {
        if self.state == ServerState::Up {
            for each in &(*(self.handler_handles.lock())) {
                each.abort();
            }
            self.handler_handles = Arc::new(Mutex::new(Vec::new()));
            self.server_handle.take().unwrap().abort();
            self.state = ServerState::Down;
        }
    }
}

impl Drop for Tero {
    fn drop(&mut self) {
        self.stop();
    }
}