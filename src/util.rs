//pub fn exit() -> ! {
//    loop {
//        cortex_m::asm::bkpt();
//    }
//}

// TODO: Not sure why this is not working better
pub fn delay_micros(micros: u32) {
    //const FREQ_MHZ: u32 = nrf52840_hal::clocks::HFCLK_FREQ / 1_000_000;
    const FREQ_MHZ: u32 = 43;
    const SETUP_OVERHEAD_ESTIMATE: u32 = 10; //TODO actually think about this
    let cycles = micros * FREQ_MHZ - SETUP_OVERHEAD_ESTIMATE;
    cortex_m::asm::delay(cycles);
}
