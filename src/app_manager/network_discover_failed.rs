use crate::{usi, request::AdpSetRequest, adp::{EAdpPibAttribute, self}};

use super::{Stateful, State, Context, Response, Message};


pub struct NetworkDiscoverFailed {}
impl Stateful<State, usi::Message, flume::Sender<usi::Message>, Context> for NetworkDiscoverFailed {
    fn on_enter(&mut self, cs: &flume::Sender<usi::Message>, context: &mut Context) -> Response<State> {

        Response::Handled
    }

    fn on_event(&mut self, cs: &flume::Sender<usi::Message>, event: &Message, context: &mut Context) -> Response<State> {
        log::info!("on event {:?}", event );
        match event {            
            Message::HeartBeat(time) => Response::Transition(State::DiscoverNetwork),
            _ =>{
                Response::Handled
            },
        }
    }

    fn on_exit(&mut self, context: &mut Context) {}
}