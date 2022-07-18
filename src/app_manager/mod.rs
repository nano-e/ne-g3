use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::thread::current;
use std::time::SystemTime;

use flume;
use flume::SendError;

use crate::adp;

use crate::adp::TExtendedAddress;
use crate::app;
use crate::app_config;
use crate::app_manager::ready::Ready;
use crate::app_manager::set_params::SetParams;
use crate::app_manager::stack_initialize::StackInitialize;
use crate::lbp;
use crate::lbp_manager;
use crate::request::AdpMacSetRequest;
use crate::request::AdpSetRequest;
use crate::usi;

use self::get_params::GetParams;
use self::idle::Idle;
use self::join_network::JoinNetwork;
use self::start_network::StartNetwork;

mod stack_initialize;
mod ready;
mod set_params;
mod join_network;
mod idle;
mod get_params;
mod start_network;

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum State {
    Idle,
    StackInitialize,
    SetParams,
    GetParams,
    JoinNetwork,
    StartNetwork,
    Ready,
}
#[derive(Debug)]
pub enum Message<'a> {
    Adp(&'a adp::Message),
    HeartBeat(SystemTime),
    Startup,
}
pub trait CommandSender<C> {
    fn send_cmd(&self, cmd: C) -> bool;
}
pub trait Stateful<S: Hash + PartialEq + Eq + Clone, C, CS: CommandSender<C>, CTX> {
    fn on_enter(&mut self, cs: &CS, context: &mut CTX) -> Response<S>;
    fn on_event(&mut self, cs: &CS, event: &Message, context: &mut CTX) -> Response<S>;
    fn on_exit(&mut self, context: &mut CTX);
}

pub enum Response<S> {
    Handled,
    Transition(S),
}
pub enum Error<S> {
    Handled,
    Transition(S),
}
// pub trait ResultExt<T, S> {
//     fn or_transition(self, state: S) -> core::result::Result<T, Error<S>>;

//     fn or_handle(self) -> core::result::Result<T, Error<S>>;
// }

// impl<T, E, S> ResultExt<T, S> for core::result::Result<T, E> {
//     fn or_transition(self, state: S) -> core::result::Result<T, Error<S>> {
//         self.map_err(|_| Error::Transition(state))
//     }

//     fn or_handle(self) -> core::result::Result<T, Error<S>> {
//         self.map_err(|_| Error::Handled)
//     }
// }
// impl<T, S> ResultExt<T, S> for core::option::Option<T> {
//     fn or_transition(self, state: S) -> core::result::Result<T, Error<S>> {
//         self.ok_or(Error::Transition(state))
//     }

//     fn or_handle(self) -> core::result::Result<T, Error<S>> {
//         self.ok_or(Error::Handled)
//     }
// }

pub struct StateMachine<S: Hash + PartialEq + Eq + Clone, C, CS: CommandSender<C>, CTX> {
    states: HashMap<S, Box<dyn Stateful<S, C, CS, CTX>>>,
    current_state: S,
    command_sender: CS,
    context: CTX
}
impl<S: Hash + PartialEq + Eq + Clone, C, CS, CTX> StateMachine<S, C, CS, CTX>
where
    CS: CommandSender<C>, S: Debug, CTX: Sized
{
    pub fn new(initial_state: S, command_sender: CS, context: CTX) -> Self {
        let mut states = HashMap::<S, Box<dyn Stateful<S, C, CS, CTX>>>::new();
        Self {
            states: states,
            current_state: initial_state,
            command_sender: command_sender,
            context: context
        }
    }
    pub fn add_state(&mut self, s: S, state: Box<dyn Stateful<S, C, CS, CTX>>) {
        self.states.insert(s, state);
    }

    fn process_event(&mut self, event: &Message) {
        let state = self.states.get_mut(&self.current_state);

        if let Some(st) = state {
            match st.on_event(&self.command_sender, event, &mut self.context) {
                Response::Handled => {}
                Response::Transition(s) => {
                    if s != self.current_state {
                        st.on_exit(&mut &mut self.context);
                        self.current_state = s;
                        loop {
                            log::info!("StateMachine : {:?} - {:?}", self.current_state, event);
                            if let Some(s) = self.states.get_mut(&self.current_state) {
                                match s.on_enter(&self.command_sender, &mut self.context) {
                                    Response::Handled => {
                                        break;
                                    }
                                    Response::Transition(s) => {
                                        if s == self.current_state {
                                            break;
                                        } else {
                                            self.current_state = s;
                                        }
                                    }
                                }
                            }
                            else{
                                log::warn!("Failed to find state : {:?}", self.current_state);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    // pub fn on_enter(&mut self) {
    //     if let Some(state) = self.states.get_mut(&self.current_state){
    //         state.on_enter();
    //     }
    // }
    // pub fn on_exit(&mut self) {
    //     if let Some(state) = self.states.get_mut(&self.current_state){
    //         state.on_exit();
    //     }
    // }
}


#[derive(Debug)]
pub struct Context {
    is_coordinator: bool,
    extended_addr: Option<TExtendedAddress>
}

pub struct AppManager {
    usi_tx: flume::Sender<usi::Message>,
    net_tx: flume::Sender<adp::Message>,
}

impl AppManager {
    pub fn new(
        usi_tx: flume::Sender<usi::Message>,
        net_tx: flume::Sender<adp::Message>,
        
    ) -> Self {
        AppManager {
            usi_tx,
            net_tx,
        }
    }
    
    fn init_states( state_machine: &mut StateMachine::<State, usi::Message, flume::Sender<usi::Message>, Context>) {
        state_machine.add_state(State::Idle, Box::new(Idle {}));
        state_machine.add_state(State::StackInitialize, Box::new(StackInitialize::new()));
        state_machine.add_state(State::SetParams, Box::new(SetParams::new()));
        state_machine.add_state(State::GetParams, Box::new(GetParams::new()));
        state_machine.add_state(State::JoinNetwork, Box::new(JoinNetwork::new()));
        state_machine.add_state(State::StartNetwork, Box::new(StartNetwork::new()));
        state_machine.add_state(State::Ready, Box::new(Ready::new()));
    }
    pub fn start(self, usi_receiver: flume::Receiver<usi::Message>, is_coordinator: bool) {
        log::info!("App Manager started ...");
        thread::spawn(move || {
            let mut state_machine =
                StateMachine::<State, usi::Message, flume::Sender<usi::Message>, Context>::new(
                    State::Idle,
                    self.usi_tx.clone(),
                    Context { is_coordinator: is_coordinator, extended_addr: None }
                );
            let mut lbp_manager = lbp_manager::LbpManager::new();
            Self::init_states(&mut state_machine);
            
           
            loop {
                match usi_receiver.recv() {
                    Ok(event) => {
                        log::info!("app_manager - {:?} received msg : {:?}", state_machine.current_state, event);
                        match event {
                            usi::Message::UsiIn(usi_msg) => {
                                if let Some(adp_msg) = adp::usi_message_to_message(&usi_msg){
                                    //TODO optimize, split event those needed by the state machine and those needed by network manager
                                    state_machine.process_event(&Message::Adp(&adp_msg));
                                    match adp_msg {
                                        adp::Message::AdpG3LbpEvent(lbp_event) => {
                                            if let Some(lbp_message) = lbp::adp_message_to_lbp_message(&lbp_event) {
                                                log::info!("Received lbp_event {:?}", lbp_message);
                                                if let Some(result) = lbp_manager.process_msg(&lbp_message) {
                                                    // (result.into());
                                                    self.usi_tx.send_cmd(usi::Message::UsiOut(result.into()));
                                                }
                                            }
                                        }
                                        adp::Message::AdpG3LbpReponse(lbp_response) => {
                                            lbp_manager.process_response (&lbp_response);
                                        }
                                        _ => {
                                            if let Err(e) = self.net_tx.send(adp_msg) {
                                                log::warn!("Failed to send adp message to network manager {}", e);
                                            }
                                        }
                                    }                                    
                                }
                               
                            }

                            usi::Message::HeartBeat(time) => {
                                state_machine.process_event(&Message::HeartBeat(time));
                            }
                            usi::Message::SystemStartup => {
                                state_machine.process_event(&Message::Startup);
                            } 
                            _ => {}
                        }
                        
                    }
                    Err(e) => {
                        log::warn!("app_manager : failed to receive message {}", e)
                    }
                }
            }
        });
    }
}

impl CommandSender<usi::Message> for flume::Sender<usi::Message> {
    fn send_cmd(&self, cmd: usi::Message) -> bool {
        self.send(cmd);
        true
    }
}

