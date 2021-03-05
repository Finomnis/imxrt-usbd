//! usb-device test
//!
//! This example turns the Teensy 4 into a USB device that can be tested
//! from the usb-device host-side test framework. See the usb-device
//! documentation for more information.
//!
//! Once we're configured and ready for the test, the LED should blink.

#![no_std]
#![no_main]

use imxrt_hal as hal;
use teensy4_pins as pins;

use embedded_hal::timer::CountDown;

const UART_BAUD: u32 = 115_200;
const GPT_OCR: hal::gpt::OutputCompareRegister = hal::gpt::OutputCompareRegister::One;
const TESTING_BLINK_PERIOD: core::time::Duration = core::time::Duration::from_millis(200);
const USB_PERIOD: core::time::Duration = core::time::Duration::from_micros(250);

#[cortex_m_rt::entry]
fn main() -> ! {
    let hal::Peripherals {
        iomuxc,
        mut ccm,
        dma,
        uart,
        mut dcdc,
        gpt1,
        pit,
        ..
    } = hal::Peripherals::take().unwrap();
    let pins = pins::t40::into_pins(iomuxc);
    let mut led = support::configure_led(pins.p13);

    // Timer for blinking
    let (_, ipg_hz) =
        ccm.pll1
            .set_arm_clock(imxrt_hal::ccm::PLL1::ARM_HZ, &mut ccm.handle, &mut dcdc);

    let mut cfg = ccm.perclk.configure(
        &mut ccm.handle,
        hal::ccm::perclk::PODF::DIVIDE_3,
        hal::ccm::perclk::CLKSEL::IPG(ipg_hz),
    );
    let (mut usb_timer, _, _, _) = pit.clock(&mut cfg);

    let mut gpt1 = gpt1.clock(&mut cfg);

    gpt1.set_wait_mode_enable(true);
    gpt1.set_mode(imxrt_hal::gpt::Mode::Reset);

    // DMA initialization (for logging)
    let mut dma_channels = dma.clock(&mut ccm.handle);
    let mut channel = dma_channels[7].take().unwrap();
    channel.set_interrupt_on_completion(false); // We'll poll the logger ourselves...

    //
    // UART initialization (for logging)
    //
    let uarts = uart.clock(
        &mut ccm.handle,
        hal::ccm::uart::ClockSelect::OSC,
        hal::ccm::uart::PrescalarSelect::DIVIDE_1,
    );
    let uart = uarts.uart2.init(pins.p14, pins.p15, UART_BAUD).unwrap();

    let (tx, _) = uart.split();
    imxrt_uart_log::dma::init(tx, channel, Default::default()).unwrap();

    let (ccm, _) = ccm.handle.raw();
    hal::ral::modify_reg!(hal::ral::ccm, ccm, CCGR6, CG1: 0b11, CG0: 0b11);

    let bus_adapter = support::new_bus_adapter();
    let bus = usb_device::bus::UsbBusAllocator::new(bus_adapter);

    let mut test_class = usb_device::test_class::TestClass::new(&bus);
    let mut device = test_class
        .make_device_builder(&bus)
        .max_packet_size_0(64)
        .build();

    gpt1.set_enable(true);
    gpt1.set_output_compare_duration(GPT_OCR, TESTING_BLINK_PERIOD);

    'reset: loop {
        led.clear();
        imxrt_uart_log::dma::poll();
        if !device.poll(&mut [&mut test_class]) {
            continue 'reset;
        }

        if device.state() != usb_device::device::UsbDeviceState::Configured {
            continue 'reset;
        }

        device.bus().configure();
        led.set();

        'configured: loop {
            usb_timer.start(USB_PERIOD);
            nb::block!(usb_timer.wait()).unwrap();
            time_elapse(&mut gpt1, || led.toggle());
            imxrt_uart_log::dma::poll();
            if device.poll(&mut [&mut test_class]) {
                test_class.poll();
            }
            if device.state() != usb_device::device::UsbDeviceState::Configured {
                break 'configured;
            }
        }
    }
}

fn time_elapse(gpt: &mut hal::gpt::GPT, func: impl FnOnce()) {
    let mut status = gpt.output_compare_status(GPT_OCR);
    if status.is_set() {
        status.clear();
        func();
    }
}
