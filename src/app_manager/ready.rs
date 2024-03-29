use crate::{app_config, usi, request::{AdpSetRequest, AdpInitializeRequest, self}, app_manager::Idle, adp};

use super::{Stateful, Response, State, Message, Context};

pub struct Ready {

}

impl Ready {
    pub fn new() -> Self {
        
        Ready {
            
        }
    }
    
}

impl Stateful<State, usi::Message, flume::Sender<usi::Message>, Context> for Ready {
    fn on_enter(&mut self, cs: &flume::Sender<usi::Message>, context: &mut Context) -> Response<State> {
        log::info!("State : Ready - onEnter");
        Response::Handled
    }

    fn on_event(&mut self, cs: &flume::Sender<usi::Message>, event: &Message, context: &mut Context) -> Response<State> {
        log::trace!("Ready : {:?}", event);
        match event {
            Message::Adp(adp) => {
                match adp {
                    adp::Message::AdpG3MsgStatusResponse(status_response) => {
                      Response::Handled
                    },                
                    _ => {
                        Response::Handled
                    }
                }
            },
            Message::HeartBeat(time) => {
                context.is_coordinator = true;
                Response::Transition(State::StackInitialize)
            }
            _ => {
                Response::Handled
            }
        }        
    }

    fn on_exit(&mut self, context: &mut Context) {}
}
