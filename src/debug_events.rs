//! Lock-free logging interface, contingent on the hidden `__log_events` feature
//!
//! Only enable `__log_events` when debugging.
//! Be certain that the mechanism that reads the log events doesn't use USB,
//! as this will create an infinite loop!

/// Log events
#[cfg(feature = "__log_events")]
#[derive(Debug)]
pub enum DebugEvent {
    EpIn {
        len: usize,
    },
    EpOut {
        index: usize,
    },
    Ep0OutSetup,
    EpError {
        index: usize,
        direction: usb_device::UsbDirection,
        status: usb_device::UsbError,
    },
    Reset,
    Configure,
    AllocEp(
        usize,
        usb_device::UsbDirection,
        usb_device::endpoint::EndpointType,
    ),
    SetAddress(u8),
    Poll(u32),
    PollSLI,
    PollURI,
    PollPCI,
    PollUI {
        ep_out: u16,
        ep_in_complete: u16,
        ep_setup: u16,
    },
    PollNone,
}

#[cfg(not(feature = "__log_events"))]
#[derive(Debug)]
pub enum DebugEvent {}

#[cfg(feature = "__log_events")]
pub(crate) static EVENTS: heapless::mpmc::Q64<DebugEvent> = heapless::mpmc::MpMcQueue::new();

pub fn next() -> Option<DebugEvent> {
    None
}

macro_rules! debug_event {
    ($val: expr) => {
        #[cfg(feature = "__log_events")]
        {
            use $crate::debug_events::DebugEvent::*;
            $crate::debug_events::EVENTS.enqueue($val).unwrap();
        }
    };
}
