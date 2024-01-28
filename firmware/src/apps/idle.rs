use crate::Context;

pub async fn idle(ctx: &mut Context) {
    ctx.lcd.clear().await;
    ctx.lcd.off();
    ctx.backlight.off();

    ctx.button.wait_for_press().await;
}
