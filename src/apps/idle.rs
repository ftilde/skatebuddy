use crate::{App, Context};

pub async fn idle(ctx: &mut Context) -> App {
    ctx.lcd.clear().await;
    ctx.lcd.off();
    ctx.backlight.off();

    ctx.button.wait_for_press().await;

    App::Menu
}
