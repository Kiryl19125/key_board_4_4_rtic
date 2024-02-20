#![no_std]
#![no_main]

use panic_halt as _;

#[rtic::app(device = stm32f1xx_hal::pac, dispatchers = [UART5, UART4])]
mod app {

    use rtic::Monotonic;
    use rtt_target::{rprintln, rtt_init_print};
    use stm32f1xx_hal::{gpio::*, prelude::*};
    use systick_monotonic::{fugit::ExtU32, *};

    // A monotonic timer to enable scheduling in RTIC
    #[monotonic(binds = SysTick, default = true)]
    type MyMono = Systick<100>; // 100 Hz / 10 ms granularity

    #[shared]
    struct Shared {
        led_red: ErasedPin<Output>,
        led_blue: ErasedPin<Output>,
    }

    #[local]
    struct Local {
        counter: u32,
        emergency_button: ErasedPin<Input<PullUp>>,
        led_green: ErasedPin<Output>,

        key_columns: [ErasedPin<Output>; 4],
        key_rows: [ErasedPin<Input<PullDown>>; 4],
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        rtt_init_print!();

        let systick = ctx.core.SYST;
        let mono = Systick::new(systick, 16_000_000);

        let rcc = ctx.device.RCC.constrain();
        let mut flash = ctx.device.FLASH.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(16.MHz())
            .freeze(&mut flash.acr);

        // leds initialization
        let mut gpio_b = ctx.device.GPIOB.split();
        let led_red = gpio_b
            .pb12
            .into_push_pull_output_with_state(&mut gpio_b.crh, PinState::Low);

        let led_blue = gpio_b
            .pb14
            .into_push_pull_output_with_state(&mut gpio_b.crh, PinState::High);

        let led_green = gpio_b
            .pb13
            .into_push_pull_output_with_state(&mut gpio_b.crh, PinState::Low);

        // configuring button interrupt
        let mut afio = ctx.device.AFIO.constrain();
        let mut emergency_button = gpio_b.pb0.into_pull_up_input(&mut gpio_b.crl);
        emergency_button.make_interrupt_source(&mut afio);
        emergency_button.enable_interrupt(&mut ctx.device.EXTI);
        emergency_button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Falling);

        // key board initializations
        let mut gpio_a = ctx.device.GPIOA.split();

        let columns = [
            gpio_a.pa0.into_push_pull_output(&mut gpio_a.crl).erase(),
            gpio_a.pa1.into_push_pull_output(&mut gpio_a.crl).erase(),
            gpio_a.pa2.into_push_pull_output(&mut gpio_a.crl).erase(),
            gpio_a.pa3.into_push_pull_output(&mut gpio_a.crl).erase(),
        ];

        let rows = [
            gpio_a.pa4.into_pull_down_input(&mut gpio_a.crl).erase(),
            gpio_a.pa5.into_pull_down_input(&mut gpio_a.crl).erase(),
            gpio_a.pa6.into_pull_down_input(&mut gpio_a.crl).erase(),
            gpio_a.pa7.into_pull_down_input(&mut gpio_a.crl).erase(),
        ];

        // let delay = &systick.delay(&clocks);

        rprintln!("init");
        rprintln!("System closk: {}", clocks.sysclk());

        foo::spawn().unwrap();
        key_listener::spawn().unwrap();

        return (
            Shared {
                led_red: led_red.erase(),
                led_blue: led_blue.erase(),
            },
            Local {
                counter: 0,
                emergency_button: emergency_button.erase(),
                led_green: led_green.erase(),
                key_columns: columns,
                key_rows: rows,
            },
            init::Monotonics(mono),
        );
    }

    #[idle()]
    fn idle(_ctx: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }

    #[task(shared=[led_red, led_blue], local=[counter], priority = 3)]
    fn foo(mut ctx: foo::Context) {
        rprintln!("foo");

        ctx.shared.led_red.lock(|led| led.toggle());
        ctx.shared.led_blue.lock(|led| led.toggle());

        *ctx.local.counter += 1;

        bar::spawn_after(ExtU32::secs(1).into(), *ctx.local.counter).unwrap();
    }

    #[task(shared=[led_red, led_blue], priority = 3)]
    fn bar(mut ctx: bar::Context, counter: u32) {
        rprintln!("bar, number of led_red blink: {}", counter);

        ctx.shared.led_red.lock(|led| led.toggle());
        ctx.shared.led_blue.lock(|led| led.toggle());

        foo::spawn_after(ExtU32::secs(1).into()).unwrap();
    }

    #[task(priority=1, local=[key_columns, key_rows])]
    fn key_listener(ctx: key_listener::Context) {
        loop {
            for i in 0..ctx.local.key_columns.len() {
                ctx.local.key_columns[i].set_high();
                for j in 0..ctx.local.key_rows.len() {
                    if ctx.local.key_rows[j].is_high() {
                        rprintln!("column: {}, row: {}", i, j);
                    }
                }
                ctx.local.key_columns[i].set_low();
            }
        }
    }

    #[task(binds=EXTI0, local=[led_green, emergency_button], shared = [led_red, led_blue], priority = 6)]
    fn emergency_stop(mut ctx: emergency_stop::Context) {
        ctx.local.led_green.toggle();
        rprintln!("Emergency STOP!");

        ctx.shared.led_blue.lock(|led| led.set_low());
        ctx.shared.led_red.lock(|led| led.set_low());

        ctx.local.emergency_button.clear_interrupt_pending_bit();
        loop {
            rtic::export::nop();
        }
    }
}
