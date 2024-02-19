use crate::Context;

pub async fn idle(ctx: &mut Context) {
    ctx.lcd.clear().await;
    ctx.lcd.off().await;
    ctx.backlight.set_off().await;

    ctx.button.wait_for_press().await;
}
