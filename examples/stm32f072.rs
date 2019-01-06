#![no_main]
#![no_std]

#[allow(unused)]
use cortex_m::interrupt::Mutex;
use core::cell::RefCell;
use panic_halt;

use stm32f0xx_hal as hal;

use crate::hal::stm32;
use crate::hal::prelude::*;
use crate::hal::serial::Serial;
use crate::hal::time::*;
use crate::hal::timers::*;

use cortex_m_rt::{ entry, interrupt };
use nb::block;

// Make external interrupt registers globally available
static INT: Mutex<RefCell<Option<stm32::TIM2>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    if let Some(p) = stm32::Peripherals::take() {
        let tim2 = p.TIM2;

        /* Constrain clocking registers */
        let rcc = p.RCC.constrain();

        /* Configure clock to 8 MHz (i.e. the default) and freeze it */
        let clocks = rcc.cfgr.sysclk(8.mhz()).freeze();

        let mut timer = Timer::tim1(p.TIM1, 1.hz(), clocks);

        loop {
            led.toggle();
            block!(timer.wait()).ok();
        }
    }

    loop {
        continue;
    }
}

#[interrupt]
fn EXTI0_1() {
    // Enter critical section
    cortex_m::interrupt::free(|cs| {
        let exti = INT.borrow(cs).borrow_mut().deref_mut().unwrap();

        // Clear interrupt
        exti.pr.write(|w| w.pif1().set_bit());
    });
}

// https://github.com/stm32-rs/stm32f0xx-hal/blob/master/examples/led_hal_button_irq.rs
// https://github.com/japaric/stm32f103xx-hal/blob/master/examples/serial-dma-tx.rs
// https://github.com/stm32-rs/stm32f0xx-hal/blob/master/examples/blinky_timer.rs#L7