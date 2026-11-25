use std::os::fd::AsFd;

use super::Identity;

#[derive(Clone, Debug)]
pub enum Process {}

impl Process {
    pub fn peer(_: &impl AsFd, _: Option<i32>) -> Option<Self> {
        None
    }

    pub fn open(_: u32) -> Option<Self> {
        None
    }

    pub fn identity(&self) -> Identity {
        match *self {}
    }

    pub fn live(&self) -> bool {
        match *self {}
    }

    pub fn parent(&self) -> Option<Self> {
        match *self {}
    }
}
