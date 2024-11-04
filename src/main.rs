// This example shows how to use a global allocator for dynamic memory allocation
// when using no_std

#![no_std]
#![no_main]

use core::u8;
use alloc::vec::Vec;
use core::ptr::addr_of_mut;
// Linked-List First Fit Heap allocator (feature = "llff")
use embedded_alloc::LlffHeap as Heap;
// Two-Level Segregated Fit Heap allocator (feature = "tlsf")
// use embedded_alloc::TlsfHeap as Heap;

use defmt::*;
use embassy_executor::Executor;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

extern crate alloc;

#[global_allocator]
static HEAP: Heap = Heap::empty();

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

static CHANNEL: Channel<CriticalSectionRawMutex, DisplayMessage, 1> = Channel::new();

enum DisplayMessage {
    LedOn,
    LedOff,
}


#[cortex_m_rt::entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1280;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }

    let mut x: Vec<u64> = Vec::new();
    for i in 0..10 {
        info!("free={}", HEAP.free());
        x.push(i + 10);
    }
    info!("x.len()={}", x.len());

    let p = embassy_rp::init(Default::default());
    let led = Output::new(p.PIN_26, Level::Low);

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| unwrap!(spawner.spawn(core1_task(led))));
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());

    executor0.run(|spawner: embassy_executor::Spawner| unwrap!(spawner.spawn(core0_task())));
}

#[embassy_executor::task]
async fn core0_task() {
    info!("Hello from core 0");

    let mut x: Vec<u64> = Vec::new();
    info!("Heap core 0 free = {}", HEAP.free());
    for i in 0..10 {
        x.push(100 + i);
    }
    info!("Heap core 0 free = {}", HEAP.free());

    loop {
        CHANNEL.send(DisplayMessage::LedOn).await;
        CHANNEL.send(DisplayMessage::LedOff).await;
        Timer::after_millis(100).await;
        info!("Heap core 0 free = {}", HEAP.free());
    }
}

// This task on core 1 does all I/O

#[embassy_executor::task]
async fn core1_task(mut led: Output<'static>) {
    info!("Hello from core 1");

    let mut x: Vec<u64> = Vec::new();
    info!("Heap core 1 free = {}", HEAP.free());
    for i in 0..10 {
        x.push(100 + i);
    }
    info!("Heap core 1 free = {}", HEAP.free());

    loop {
        match CHANNEL.receive().await {
            DisplayMessage::LedOn => {
                led.set_high();
                info!("Heap Core 1 free = {}", HEAP.free());
                {
                    let mut y: Vec<u64> = Vec::new();
                    for i in 1..10 {
                        y.push(i);
                    }
                    info!("y.len() = {}", y.len());
                    let a: Vec<u64> = y.into_iter().filter(|&x| x & 1 == 1).collect();
                    info!("a.len() = {}", a.len());
                }
                x.push(999);
            }
            DisplayMessage::LedOff => led.set_low(),
        }
    }
}
