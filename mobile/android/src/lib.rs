//! Honda Sim for Android: makes the window and OpenGL ES surface, points the app's files at storage the user
//! can reach, gives it the clipboard, and runs the phone app (mobile/shared).
#![cfg(target_os = "android")]

use honda_sim_mobile::winit::event_loop::EventLoopBuilder;
use honda_sim_mobile::winit::platform::android::activity::AndroidApp;
use honda_sim_mobile::winit::platform::android::EventLoopBuilderExtAndroid;
use honda_sim_mobile::winit::window::WindowBuilder;
use honda_sim_mobile::{Platform, WindowedSurface};

#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("HondaSim"));
    std::panic::set_hook(Box::new(|p| log::error!("{p}")));

    // Tunes, logs and controls.json go in Android/data/com.hondasim.app/files/HondaSim (the desktop app's
    // store.rs puts them under %APPDATA%\HondaSim).
    let dir = app.external_data_path().or_else(|| app.internal_data_path());
    if let Some(d) = &dir {
        std::env::set_var("APPDATA", d);
    }
    let hint = dir.map(|d| format!("Files are in {}/HondaSim (Android/data/com.hondasim.app on your phone's storage).", d.display())).unwrap_or_default();
    let clip_app = app.clone();
    honda_sim_mobile::set_platform(Platform { clipboard: Some(Box::new(move || clipboard(&clip_app))), files_hint: hint });

    let event_loop = EventLoopBuilder::new().with_android_app(app).build();
    honda_sim_mobile::run(event_loop, WindowBuilder::new().with_title("Honda Sim"), |w| Box::new(WindowedSurface::new(w)));
}

/// The clipboard's text (for pasting tables copied from FlashPro or a spreadsheet), through Android's ClipboardManager.
fn clipboard(app: &AndroidApp) -> Option<String> {
    use jni::objects::{JObject, JString};
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let result = (|| -> jni::errors::Result<Option<String>> {
        let name = env.new_string("clipboard")?;
        let cm = env.call_method(&activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[(&name).into()])?.l()?;
        if cm.is_null() { return Ok(None); }
        let clip = env.call_method(&cm, "getPrimaryClip", "()Landroid/content/ClipData;", &[])?.l()?;
        if clip.is_null() { return Ok(None); }
        let item = env.call_method(&clip, "getItemAt", "(I)Landroid/content/ClipData$Item;", &[0.into()])?.l()?;
        let text = env.call_method(&item, "coerceToText", "(Landroid/content/Context;)Ljava/lang/CharSequence;", &[(&activity).into()])?.l()?;
        if text.is_null() { return Ok(None); }
        let s = env.call_method(&text, "toString", "()Ljava/lang/String;", &[])?.l()?;
        Ok(Some(env.get_string(&JString::from(s))?.into()))
    })();
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    // The activity reference belongs to Android; don't let JObject's drop touch it.
    std::mem::forget(activity);
    result.ok().flatten()
}
