//! A device's new screen size: "Deactivate All", then a new capability exchange (MS-RDPBCGR 1.3.1.3).

use ironrdp_connector::Sequence as _;
use ironrdp_connector::connection_activation::ConnectionActivationState;
use ironrdp_core::WriteBuf;
use ironrdp_graphics::image_processing::PixelFormat;
use ironrdp_pdu::Action;
use ironrdp_session::image::DecodedImage;

use super::device::{Device, Event, Malformed};
use crate::screen::Screen;

/// The most steps one message may move the exchange on: it has fewer than this in all.
const MOST_STEPS: usize = 32;

pub(super) fn go_on(
    device: &mut Device,
    mut message: Option<(Action, &[u8])>,
    screen: &mut Screen,
) -> Result<Vec<Event>, Malformed> {
    let mut events = Vec::new();
    let mut output = WriteBuf::new();
    for _ in 0..MOST_STEPS {
        let Some(exchange) = &mut device.reactivating else {
            break;
        };
        let written = if exchange.next_pdu_hint().is_some() {
            match message.take() {
                // Drawing that was on its way when the device changed its screen.
                Some((Action::FastPath, _)) | None => break,
                Some((Action::X224, message)) => exchange.step(message, None, &mut output),
            }
        } else {
            exchange.step_no_input(&mut output)
        }
        .map_err(|_| Malformed)?;
        if let Some(length) = written.size() {
            events.push(Event::Send(output[..length].to_vec()));
        }
        output.clear();
        if finish(device, screen)? {
            events.push(Event::Resized);
        }
    }
    Ok(events)
}

fn finish(device: &mut Device, screen: &mut Screen) -> Result<bool, Malformed> {
    let Some(exchange) = &device.reactivating else {
        return Ok(false);
    };
    let ConnectionActivationState::Finalized {
        desktop_size,
        share_id,
        static_channel_chunk_size,
        enable_server_pointer,
        pointer_software_rendering,
        ..
    } = exchange.connection_activation_state()
    else {
        return Ok(false);
    };
    let (io_channel, user_channel) = (exchange.io_channel_id(), exchange.user_channel_id());
    let reactivated = device.stage.reactivate(
        io_channel,
        user_channel,
        share_id,
        enable_server_pointer,
        pointer_software_rendering,
        static_channel_chunk_size,
    );
    if !reactivated {
        return Err(Malformed);
    }
    let (width, height) = (desktop_size.width, desktop_size.height);
    screen
        .resize(usize::from(width), usize::from(height))
        .map_err(|_| Malformed)?;
    device.image = DecodedImage::new(PixelFormat::RgbA32, width, height);
    device.reactivating = None;
    Ok(true)
}
