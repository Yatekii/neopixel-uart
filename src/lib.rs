#![no_std]

mod read_write_buffer;
mod brg;
mod channel_config;
mod producer;
mod consumer;

pub use crate::channel_config::{ ChannelConfig, DisplayType };


#[cfg(test)]
mod tests {

}